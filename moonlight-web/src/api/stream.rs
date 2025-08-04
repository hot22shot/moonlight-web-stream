use std::{fs::File, io::BufReader, sync::Arc, time::Duration};

use actix_web::{
    Error, HttpRequest, HttpResponse, get, rt as actix_rt,
    web::{Data, Payload},
};
use actix_ws::{Closed, Message, MessageStream, Session};
use anyhow::anyhow;
use log::warn;
use moonlight_common::network::ApiError;
use tokio::{spawn, task::JoinHandle, time::sleep};
use webrtc::{
    api::{
        APIBuilder,
        interceptor_registry::register_default_interceptors,
        media_engine::{MIME_TYPE_H264, MediaEngine},
    },
    ice_transport::ice_server::RTCIceServer,
    interceptor::registry::Registry,
    media::{Sample, io::h264_reader::H264Reader},
    peer_connection::{
        configuration::RTCConfiguration, sdp::session_description::RTCSessionDescription,
    },
    rtp_transceiver::{rtp_codec::RTCRtpCodecCapability, rtp_sender::RTCRtpSender},
    track::track_local::track_local_static_sample::TrackLocalStaticSample,
};

use crate::{
    Config,
    api_bindings::{App, RtcIceCandidate, StreamClientMessage, StreamServerMessage},
    data::RuntimeApiData,
};

/// The stream handler WILL authenticate the client because it is a websocket
#[get("/stream")]
pub async fn start_stream(
    data: Data<RuntimeApiData>,
    config: Data<Config>,
    request: HttpRequest,
    payload: Payload,
) -> Result<HttpResponse, Error> {
    let (response, session, mut stream) = actix_ws::handle(&request, payload)?;

    actix_rt::spawn(async move {
        let message;
        loop {
            message = match stream.recv().await {
                Some(Ok(Message::Text(text))) => text,
                Some(Ok(Message::Binary(_))) => {
                    return;
                }
                Some(Ok(_)) => continue,
                Some(Err(_)) => {
                    return;
                }
                None => {
                    return;
                }
            };
            break;
        }

        let message: StreamClientMessage = match serde_json::from_str(&message) {
            Ok(value) => value,
            Err(_) => {
                return;
            }
        };

        let StreamClientMessage::Offer {
            credentials,
            host_id,
            app_id,
            offer_sdp,
        } = message
        else {
            return;
        };

        if credentials != config.credentials {
            return;
        }

        let offer_description = match RTCSessionDescription::offer(offer_sdp) {
            Ok(value) => value,
            Err(err) => {
                warn!("failed to create session description from offer: {err}");
                return;
            }
        };

        let app = spawn({
            let data = data.clone();

            async move {
                let hosts = data.hosts.read().await;
                let Some(host) = hosts.get(host_id as usize) else {
                    return Ok(None);
                };
                let mut host = host.lock().await;

                let Some(result) = host.moonlight.app_list().await else {
                    return Ok(None);
                };
                let app_list = result?;

                let Some(app) = app_list.into_iter().find(|app| app.id == app_id) else {
                    return Ok(None);
                };

                Ok(Some(app.into()))
            }
        });

        if let Err(err) =
            start_connection(data, app, session.clone(), stream, offer_description).await
        {
            warn!("stream error: {err:?}");

            let _ = session.close(None).await;
        }
    });

    Ok(response)
}

async fn start_connection(
    data: Data<RuntimeApiData>,
    app: JoinHandle<Result<Option<App>, ApiError>>,
    mut sender: Session,
    receiver: MessageStream,
    offer_description: RTCSessionDescription,
) -> Result<(), anyhow::Error> {
    let config = RTCConfiguration {
        // TODO: put this into config
        ice_servers: vec![RTCIceServer {
            urls: vec![
                "stun:stun.l.google.com:19302".to_owned(),
                "stun:stun2.l.google.com:19302".to_owned(),
            ],
            ..Default::default()
        }],
        ..Default::default()
    };

    let mut media = MediaEngine::default();
    // TODO: only register supported codecs
    media.register_default_codecs()?;

    let mut registry = Registry::new();

    // Use the default set of Interceptors
    registry = register_default_interceptors(registry, &mut media)?;

    let api = APIBuilder::new()
        .with_media_engine(media)
        .with_interceptor_registry(registry)
        .build();

    // Create Peer Connection
    let peer = api.new_peer_connection(config.clone()).await?;

    // Send ice candidates to client
    peer.on_ice_candidate({
        let sender = sender.clone();

        Box::new(move |candidate| {
            let mut sender = sender.clone();
            Box::pin(async move {
                let Some(candidate) = candidate else {
                    return;
                };

                let Ok(candidate_json) = candidate.to_json() else {
                    return;
                };

                let message = StreamServerMessage::AddIceCandidate {
                    candidate: RtcIceCandidate {
                        candidate: candidate_json.candidate,
                        sdp_mid: candidate_json.sdp_mid,
                        sdp_mline_index: candidate_json.sdp_mline_index,
                        username_fragment: candidate_json.username_fragment,
                    },
                };

                let _ = send_ws_message(&mut sender, message).await;
            })
        })
    });

    // Create and Add a video track
    let video_track = Arc::new(TrackLocalStaticSample::new(
        RTCRtpCodecCapability {
            mime_type: MIME_TYPE_H264.to_owned(),
            ..Default::default()
        },
        "video".to_owned(),
        "webrtc-rs".to_owned(),
    ));
    let rtp_sender = peer.add_track(Arc::clone(&video_track) as Arc<_>).await?;
    test_video(video_track, rtp_sender);

    // Listen test Channel
    peer.on_data_channel(Box::new(|channel| {
        let label = channel.label().to_owned();
        channel.on_message(Box::new(move |message| {
            warn!("RECEIVED: {}, {:?}", label, str::from_utf8(&message.data));

            Box::pin(async move {})
        }));

        Box::pin(async move {})
    }));

    // Test Channel
    let test_channel = peer.create_data_channel("test2", None).await?;

    // Set Offer as Remote
    peer.set_remote_description(offer_description).await?;

    // Create and Send Answer
    let answer = peer.create_answer(None).await?;
    peer.set_local_description(answer.clone()).await?;

    let app = match app.await {
        Ok(Ok(Some(value))) => value,
        Ok(Ok(None)) => {
            send_ws_message(&mut sender, StreamServerMessage::HostOrAppNotFound).await?;

            let _ = peer.close().await;
            return Err(anyhow!("app not found"));
        }
        Ok(Err(err)) => {
            let _ = peer.close().await;
            return Err(anyhow!("error whilst getting app: {err:?}"));
        }
        Err(err) => {
            let _ = peer.close().await;
            return Err(anyhow!("error whilst getting app: {err:?}"));
        }
    };

    send_ws_message(
        &mut sender,
        StreamServerMessage::Answer {
            answer_sdp: answer.sdp,
            app,
        },
    )
    .await?;

    sleep(Duration::from_secs(4)).await;

    test_channel.send_text("Hello").await?;

    sleep(Duration::from_secs(100)).await;

    Ok(())
}

async fn send_ws_message(sender: &mut Session, message: StreamServerMessage) -> Result<(), Closed> {
    let Ok(json) = serde_json::to_string(&message) else {
        warn!("stream failed to serialize to json");
        return Ok(());
    };

    sender.text(json).await
}

fn test_video(video_track: Arc<TrackLocalStaticSample>, rtp_sender: Arc<RTCRtpSender>) {
    let video_file_name = "server/output.h264";

    // Read incoming RTCP packets
    // Before these packets are returned they are processed by interceptors. For things
    // like NACK this needs to be called.
    tokio::spawn(async move {
        let mut rtcp_buf = vec![0u8; 1500];
        while let Ok((_, _)) = rtp_sender.read(&mut rtcp_buf).await {}
    });

    spawn(async move {
        // Open a H264 file and start reading using our H264Reader
        let file = File::open(video_file_name).unwrap();
        let reader = BufReader::new(file);
        let mut h264 = H264Reader::new(reader, 1_048_576);

        // Wait for connection established
        sleep(Duration::from_secs(4)).await;

        println!("play video from disk file {video_file_name}");

        // It is important to use a time.Ticker instead of time.Sleep because
        // * avoids accumulating skew, just calling time.Sleep didn't compensate for the time spent parsing the data
        // * works around latency issues with Sleep
        let mut ticker = tokio::time::interval(Duration::from_millis(33));
        loop {
            let nal = match h264.next_nal() {
                Ok(nal) => nal,
                Err(err) => {
                    println!("All video frames parsed and sent: {err}");
                    break;
                }
            };

            /*println!(
                "PictureOrderCount={}, ForbiddenZeroBit={}, RefIdc={}, UnitType={}, data={}",
                nal.picture_order_count,
                nal.forbidden_zero_bit,
                nal.ref_idc,
                nal.unit_type,
                nal.data.len()
            );*/

            video_track
                .write_sample(&Sample {
                    data: nal.data.freeze(),
                    duration: Duration::from_secs(1),
                    ..Default::default()
                })
                .await
                .unwrap();

            let _ = ticker.tick().await;
        }
    });
}
