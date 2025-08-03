use std::{sync::Arc, time::Duration};

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
        media_engine::{MIME_TYPE_HEVC, MediaEngine},
    },
    ice_transport::ice_server::RTCIceServer,
    interceptor::registry::Registry,
    peer_connection::{
        configuration::RTCConfiguration, sdp::session_description::RTCSessionDescription,
    },
    rtp_transceiver::rtp_codec::RTCRtpCodecCapability,
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

    // Create test data channel
    let channel = peer.create_data_channel("test", None).await?;

    // Create and Add a video track
    let video_track = Arc::new(TrackLocalStaticSample::new(
        RTCRtpCodecCapability {
            mime_type: MIME_TYPE_HEVC.to_owned(),
            ..Default::default()
        },
        "video".to_owned(),
        "webrtc-video".to_owned(),
    ));
    let rtp_sender = peer.add_track(Arc::clone(&video_track) as Arc<_>).await?;

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

    Ok(())
}

async fn send_ws_message(sender: &mut Session, message: StreamServerMessage) -> Result<(), Closed> {
    let Ok(json) = serde_json::to_string(&message) else {
        warn!("stream failed to serialize to json");
        return Ok(());
    };

    sender.text(json).await
}
