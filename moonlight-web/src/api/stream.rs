use std::{sync::Arc, time::Duration};

use actix_web::{
    Error, HttpRequest, HttpResponse, get, rt as actix_rt,
    web::{Data, Payload},
};
use actix_ws::{Closed, Message, MessageStream, Session};
use anyhow::anyhow;
use log::{debug, info, warn};
use moonlight_common::{
    debug::{DebugHandler, NullHandler},
    network::ApiError,
    stream::{ColorRange, Colorspace, MoonlightStream},
    video::SupportedVideoFormats,
};
use tokio::{
    spawn,
    sync::{Mutex, Notify, mpsc::Sender},
    task::{JoinHandle, spawn_blocking},
};
use webrtc::{
    api::{
        APIBuilder,
        interceptor_registry::register_default_interceptors,
        media_engine::{MIME_TYPE_AV1, MIME_TYPE_H264, MIME_TYPE_HEVC, MediaEngine},
    },
    ice_transport::{ice_connection_state::RTCIceConnectionState, ice_server::RTCIceServer},
    interceptor::registry::Registry,
    peer_connection::{
        configuration::RTCConfiguration, peer_connection_state::RTCPeerConnectionState,
        sdp::session_description::RTCSessionDescription,
    },
    rtp_transceiver::rtp_codec::RTCRtpCodecCapability,
    track::track_local::track_local_static_sample::TrackLocalStaticSample,
};

use crate::{
    Config,
    api::stream::video::H264TrackSampleVideoDecoder,
    api_bindings::{App, RtcIceCandidate, StreamClientMessage, StreamServerMessage},
    data::RuntimeApiData,
};

mod video;

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

        let video_mime_type = MIME_TYPE_H264;
        let video_formats = supported_formats_from_mime(video_mime_type);

        let moonlight = actix_rt::spawn({
            let hosts = data.clone();

            start_moonlight(hosts, host_id as usize, app_id as usize, video_formats)
        });

        if let Err(err) = start_webrtc(
            data,
            moonlight,
            session.clone(),
            stream,
            offer_description,
            video_mime_type.to_owned(),
        )
        .await
        {
            warn!("[Stream]: stream error: {err:?}");

            let _ = session.close(None).await;
        }
    });

    Ok(response)
}

struct MlJoinData {
    app: App,
    stream: MoonlightStream,
    set_video_track: Sender<Arc<TrackLocalStaticSample>>,
}

async fn start_moonlight(
    data: Data<RuntimeApiData>,
    host_id: usize,
    app_id: usize,
    supported_video_formats: SupportedVideoFormats,
) -> Result<Option<MlJoinData>, ApiError> {
    let hosts = data.hosts.read().await;
    let Some(host) = hosts.get(host_id) else {
        return Ok(None);
    };
    let mut host = host.lock().await;

    let Some(result) = host.moonlight.app_list().await else {
        return Ok(None);
    };
    let app_list = result?;

    let Some(app) = app_list.into_iter().find(|app| app.id as usize == app_id) else {
        return Ok(None);
    };

    // Start stream
    let video_decoder = H264TrackSampleVideoDecoder::new(None);
    let set_video_decoder = video_decoder.video_track_setter();

    let stream = match host
        .moonlight
        .start_stream(
            &data.instance,
            &data.crypto,
            app_id as u32,
            1920,
            1080,
            60,
            Colorspace::Rec709,
            ColorRange::Limited,
            10000,
            4096,
            DebugHandler,
            video_decoder,
            NullHandler,
        )
        .await
    {
        Some(Ok(value)) => value,
        Some(Err(err)) => {
            warn!("[Stream]: failed to start moonlight stream: {err:?}");
            return Ok(None);
        }
        None => return Ok(None),
    };

    Ok(Some(MlJoinData {
        app: app.into(),
        stream,
        set_video_track: set_video_decoder,
    }))
}

async fn start_webrtc(
    data: Data<RuntimeApiData>,
    app: JoinHandle<Result<Option<MlJoinData>, ApiError>>,
    mut sender: Session,
    receiver: MessageStream,
    offer_description: RTCSessionDescription,
    video_mime_type: String,
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
    let peer = Arc::new(api.new_peer_connection(config.clone()).await?);

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

    // - Create and Add a video track
    let video_track = Arc::new(TrackLocalStaticSample::new(
        RTCRtpCodecCapability {
            mime_type: video_mime_type,
            clock_rate: 90000,
            sdp_fmtp_line: "packetization-mode=0;profile-level-id=42e01f".to_owned(), // important
            ..Default::default()
        },
        "video".to_owned(),
        "moonlight".to_owned(),
    ));
    let rtp_sender = peer.add_track(Arc::clone(&video_track) as Arc<_>).await?;

    // Read incoming RTCP packets
    // Before these packets are returned they are processed by interceptors. For things
    // like NACK this needs to be called.
    spawn(async move {
        let mut rtcp_buf = vec![0u8; 1500];
        while let Ok((_, _)) = rtp_sender.read(&mut rtcp_buf).await {}
    });

    // - Listen test Channel
    peer.on_data_channel(Box::new(|channel| {
        let label = channel.label().to_owned();
        channel.on_message(Box::new(move |message| {
            debug!("RECEIVED: {}, {:?}", label, str::from_utf8(&message.data));

            Box::pin(async move {})
        }));

        Box::pin(async move {})
    }));

    // - Test Channel
    let test_channel_notify = Arc::new(Notify::new());
    let test_channel = peer.create_data_channel("test2", None).await?;
    test_channel.on_open({
        let test_channel_notify = test_channel_notify.clone();
        Box::new(move || {
            test_channel_notify.notify_waiters();

            Box::pin(async move {})
        })
    });
    let test_channel_notify = spawn(async move { test_channel_notify.notified().await });

    // When connected
    let connected_notify = Arc::new(Notify::new());
    peer.on_ice_connection_state_change({
        let connected_notify = connected_notify.clone();

        Box::new(move |state| {
            if matches!(state, RTCIceConnectionState::Connected) {
                // Connection established: allow audio, video
                connected_notify.notify_waiters();
            }

            Box::pin(async move {})
        })
    });

    let connected_notify = spawn(async move { connected_notify.notified().await });

    // Set Offer as Remote
    peer.set_remote_description(offer_description).await?;

    // Create and Send Answer
    let answer = peer.create_answer(None).await?;
    peer.set_local_description(answer.clone()).await?;

    let MlJoinData {
        app,
        stream,
        set_video_track,
    } = match app.await {
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

    // Connection state change
    peer.on_peer_connection_state_change({
        let stream = Arc::new(Mutex::new(Some(stream)));

        Box::new(move |state| {
            if matches!(
                state,
                RTCPeerConnectionState::Disconnected
                    | RTCPeerConnectionState::Failed
                    | RTCPeerConnectionState::Closed
            ) {
                let stream = stream.clone();

                return Box::pin(async move {
                    let mut stream = stream.lock().await;

                    if let Some(stream) = stream.take() {
                        let _ = spawn_blocking(move || {
                            stream.stop();
                            info!("[Stream]: Moonlight Stream Stopped (State: {state})");
                        })
                        .await;
                    }
                });
            }

            Box::pin(async move {})
        })
    });

    send_ws_message(
        &mut sender,
        StreamServerMessage::Answer {
            answer_sdp: answer.sdp,
            app,
        },
    )
    .await?;

    connected_notify.await?;

    debug!("[Stream]: SETTINGS VIDEO TRACK1");
    // Send test messages
    test_channel_notify.await?;
    test_channel.send_text("Hello").await?;

    // Set video decoder
    set_video_track
        .send_timeout(video_track, Duration::from_secs(2))
        .await?;

    info!("[Stream]: Stopping Stream");

    Ok(())
}

async fn send_ws_message(sender: &mut Session, message: StreamServerMessage) -> Result<(), Closed> {
    let Ok(json) = serde_json::to_string(&message) else {
        warn!("stream failed to serialize to json");
        return Ok(());
    };

    sender.text(json).await
}

fn supported_formats_from_mime(mime_type: &str) -> SupportedVideoFormats {
    if mime_type.eq_ignore_ascii_case(MIME_TYPE_H264) {
        return SupportedVideoFormats::MASK_H264;
    } else if mime_type.eq_ignore_ascii_case(MIME_TYPE_AV1) {
        return SupportedVideoFormats::MASK_AV1;
    } else if mime_type.eq_ignore_ascii_case(MIME_TYPE_HEVC) {
        return SupportedVideoFormats::MASK_H265;
    }

    SupportedVideoFormats::empty()
}
