use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::SendError,
    },
    time::Duration,
};

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
    sync::{
        Mutex, Notify,
        mpsc::{Sender, channel},
    },
    task::{JoinHandle, spawn_blocking},
};
use webrtc::{
    api::{
        APIBuilder,
        interceptor_registry::register_default_interceptors,
        media_engine::{
            MIME_TYPE_AV1, MIME_TYPE_H264, MIME_TYPE_HEVC, MIME_TYPE_OPUS, MediaEngine,
        },
    },
    ice_transport::{
        ice_candidate::RTCIceCandidateInit, ice_connection_state::RTCIceConnectionState,
        ice_server::RTCIceServer,
    },
    interceptor::registry::Registry,
    peer_connection::{
        configuration::RTCConfiguration,
        peer_connection_state::RTCPeerConnectionState,
        sdp::{sdp_type::RTCSdpType, session_description::RTCSessionDescription},
        signaling_state::RTCSignalingState,
    },
    rtp_transceiver::rtp_codec::RTCRtpCodecCapability,
    track::track_local::track_local_static_sample::TrackLocalStaticSample,
};

use crate::{
    Config,
    api::stream::{audio::OpusTrackSampleAudioDecoder, video::H264TrackSampleVideoDecoder},
    api_bindings::{
        App, RtcIceCandidate, RtcSdpType, RtcSessionDescription, StreamClientMessage,
        StreamServerMessage, StreamSignalingMessage,
    },
    data::RuntimeApiData,
};

mod audio;
mod connection;
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

        let message = match serde_json::from_str::<StreamClientMessage>(&message) {
            Ok(value) => value,
            Err(_) => {
                return;
            }
        };

        let StreamClientMessage::AuthenticateAndInit {
            credentials,
            host_id,
            app_id,
        } = message
        else {
            let _ = session.close(None).await;
            return;
        };

        if credentials != config.credentials {
            return;
        }

        if let Err(err) = start(
            data,
            host_id as usize,
            app_id as usize,
            session.clone(),
            stream,
        )
        .await
        {
            warn!("[Stream]: stream error: {err:?}");

            let _ = session.close(None).await;
        }
    });

    Ok(response)
}

struct StreamStage {
    name: &'static str,
    notify: Notify,
    state: AtomicBool,
}

impl StreamStage {
    pub fn new(name: &'static str) -> Self {
        Self {
            name,
            notify: Notify::new(),
            state: AtomicBool::new(false),
        }
    }

    pub fn is_reached(&self) -> bool {
        self.state.load(Ordering::Acquire)
    }
    pub async fn when_reached(&self) {
        let future = self.notify.notified();
        if self.is_reached() {
            return;
        }

        future.await;
    }

    pub fn set_reached(&self) {
        info!("[Stream]: signal \"{}\" called", self.name);
        self.state.store(true, Ordering::Release);
        self.notify.notify_waiters();
    }
}

struct StreamState {
    pub connected: StreamStage,
    pub stop: StreamStage,
}

async fn start(
    data: Data<RuntimeApiData>,
    host_id: usize,
    app_id: usize,
    ws_sender: Session,
    mut ws_receiver: MessageStream,
) -> Result<(), anyhow::Error> {
    let state = Arc::new(StreamState {
        connected: StreamStage::new("connected"),
        stop: StreamStage::new("stop"),
    });

    let hosts = data.hosts.read().await;
    let Some(host) = hosts.get(host_id) else {
        todo!()
    };
    let mut host = host.lock().await;

    let Some(result) = host.moonlight.app_list().await else {
        todo!()
    };
    let app_list = result?;

    let Some(app) = app_list.into_iter().find(|app| app.id as usize == app_id) else {
        todo!()
    };

    // Send App Update
    spawn({
        let mut sender = ws_sender.clone();
        async move {
            let _ = send_ws_message(
                &mut sender,
                StreamServerMessage::UpdateApp { app: app.into() },
            )
            .await;
        }
    });

    // Create video stream
    let video_track = Arc::new(TrackLocalStaticSample::new(
        RTCRtpCodecCapability {
            mime_type: MIME_TYPE_H264.to_string(),
            clock_rate: 90000,
            sdp_fmtp_line: "packetization-mode=0;profile-level-id=42e01f".to_owned(), // important
            ..Default::default()
        },
        "video".to_owned(),
        "moonlight".to_owned(),
    ));
    let video_decoder = H264TrackSampleVideoDecoder::new(video_track.clone(), state.clone());

    // Create audio stream
    let audio_track = Arc::new(TrackLocalStaticSample::new(
        RTCRtpCodecCapability {
            mime_type: MIME_TYPE_OPUS.to_string(),
            clock_rate: 48000,
            channels: 2,
            ..Default::default()
        },
        "audio".to_owned(),
        "moonlight".to_owned(),
    ));
    let audio_decoder = OpusTrackSampleAudioDecoder::new(audio_track.clone(), state.clone());

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
            audio_decoder,
        )
        .await
    {
        Some(Ok(value)) => value,
        Some(Err(err)) => {
            warn!("[Stream]: failed to start moonlight stream: {err:?}");
            todo!()
        }
        None => todo!(),
    };

    // -- Configure WebRTC
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

    // -- Create and Configure Peer
    let peer = Arc::new(api.new_peer_connection(config.clone()).await?);

    // -- Handle Signaling: We're the impolite peer
    {
        let making_offer = Arc::new(AtomicBool::new(false));

        actix_rt::spawn({
            let mut ws_sender = ws_sender.clone();
            let peer = peer.clone();
            let making_offer = making_offer.clone();

            async move {
                while let Some(Ok(Message::Text(text))) = ws_receiver.recv().await {
                    let Ok(message) = serde_json::from_str::<StreamClientMessage>(&text) else {
                        warn!("[Stream]: failed to deserialize from json");
                        continue;
                    };

                    match message {
                        StreamClientMessage::Signaling(StreamSignalingMessage::Description(
                            description,
                        )) => {
                            let making_offer = making_offer.load(Ordering::Acquire);

                            let ready_for_offer = !making_offer
                                && peer.signaling_state() == RTCSignalingState::Stable;

                            let offer_collision =
                                description.ty == RtcSdpType::Offer && !ready_for_offer;

                            if offer_collision {
                                continue;
                            }

                            let description = match &description.ty {
                                RtcSdpType::Offer => RTCSessionDescription::offer(description.sdp),
                                RtcSdpType::Answer => {
                                    RTCSessionDescription::answer(description.sdp)
                                }
                                RtcSdpType::Pranswer => {
                                    RTCSessionDescription::pranswer(description.sdp)
                                }
                                _ => {
                                    warn!(
                                        "[Stream]: failed to handle RTCSdpType {:?}",
                                        description.ty
                                    );
                                    continue;
                                }
                            };
                            let Ok(description) = description else {
                                warn!("[Stream]: Received invalid RTCSessionDescription");
                                continue;
                            };

                            if let Err(err) = peer.set_remote_description(description).await {
                                warn!("[Stream]: failed to set remote description: {err:?}");
                                continue;
                            }

                            let local_description = match peer.create_answer(None).await {
                                Err(err) => {
                                    warn!("[Stream]: failed to create answer: {err:?}");
                                    continue;
                                }
                                Ok(value) => value,
                            };
                            if let Err(err) =
                                peer.set_local_description(local_description.clone()).await
                            {
                                warn!("[Stream]: failed to set local description: {err:?}");
                                continue;
                            }

                            let _ = send_ws_message(
                                &mut ws_sender,
                                StreamServerMessage::Signaling(
                                    StreamSignalingMessage::Description(RtcSessionDescription {
                                        ty: local_description.sdp_type.into(),
                                        sdp: local_description.sdp,
                                    }),
                                ),
                            )
                            .await;
                        }
                        StreamClientMessage::Signaling(
                            StreamSignalingMessage::AddIceCandidate(description),
                        ) => {
                            if let Err(err) = peer
                                .add_ice_candidate(RTCIceCandidateInit {
                                    candidate: description.candidate,
                                    sdp_mid: description.sdp_mid,
                                    sdp_mline_index: description.sdp_mline_index,
                                    username_fragment: description.username_fragment,
                                })
                                .await
                            {
                                warn!("[Stream]: failed to add ice candidate: {err:?}");
                                continue;
                            }
                        }
                        // This should already be done
                        StreamClientMessage::AuthenticateAndInit { .. } => {}
                    }
                }
            }
        });

        peer.on_negotiation_needed({
            let peer = peer.clone();
            let making_offer = making_offer.clone();

            Box::new(move || {
                let peer = peer.clone();
                let making_offer = making_offer.clone();

                Box::pin(async move {
                    making_offer.store(true, Ordering::Release);

                    let local_description = match peer.create_offer(None).await {
                        Err(err) => {
                            making_offer.store(false, Ordering::Release);

                            warn!("[Stream]: failed to create offer: {err:?}");
                            return;
                        }
                        Ok(value) => value,
                    };
                    if let Err(err) = peer.set_local_description(local_description).await {
                        making_offer.store(false, Ordering::Release);

                        warn!("[Stream]: failed to set local description: {err:?}");
                        return;
                    }

                    making_offer.store(false, Ordering::Release);
                })
            })
        });

        peer.on_ice_candidate({
            let sender = ws_sender.clone();

            Box::new(move |candidate| {
                let mut sender = sender.clone();
                Box::pin(async move {
                    let Some(candidate) = candidate else {
                        return;
                    };

                    let Ok(candidate_json) = candidate.to_json() else {
                        return;
                    };

                    let message = StreamServerMessage::Signaling(
                        StreamSignalingMessage::AddIceCandidate(RtcIceCandidate {
                            candidate: candidate_json.candidate,
                            sdp_mid: candidate_json.sdp_mid,
                            sdp_mline_index: candidate_json.sdp_mline_index,
                            username_fragment: candidate_json.username_fragment,
                        }),
                    );

                    let _ = send_ws_message(&mut sender, message).await;
                })
            })
        });
    };

    // -- Add a video track
    let video_sender = peer.add_track(Arc::clone(&video_track) as Arc<_>).await?;

    // Read incoming RTCP packets
    // Before these packets are returned they are processed by interceptors. For things
    // like NACK this needs to be called.
    spawn(async move {
        let mut rtcp_buf = vec![0u8; 1500];
        while let Ok((_, _)) = video_sender.read(&mut rtcp_buf).await {}
    });

    // -- Add a audio track
    let audio_sender = peer.add_track(Arc::clone(&audio_track) as Arc<_>).await?;

    // Read incoming RTCP packets
    // Before these packets are returned they are processed by interceptors. For things
    // like NACK this needs to be called.
    spawn(async move {
        let mut rtcp_buf = vec![0u8; 1500];
        while let Ok((_, _)) = audio_sender.read(&mut rtcp_buf).await {}
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

    // Connection state change
    peer.on_ice_connection_state_change({
        let state = state.clone();

        Box::new(move |peer_state| {
            if matches!(peer_state, RTCIceConnectionState::Connected) {
                state.connected.set_reached();
            }

            Box::pin(async move {})
        })
    });
    peer.on_peer_connection_state_change({
        let state = state.clone();

        Box::new(move |peer_state| {
            if matches!(
                peer_state,
                RTCPeerConnectionState::Failed
                    | RTCPeerConnectionState::Disconnected
                    | RTCPeerConnectionState::Closed
            ) {
                // Sometimes we don't connect before failing
                state.connected.set_reached();

                state.stop.set_reached();
            }

            Box::pin(async move {})
        })
    });

    spawn({
        let state = state.clone();
        state.connected.when_reached().await;

        async move {
            // Send test messages
            let _ = test_channel_notify.await;
            let _ = test_channel.send_text("Hello").await;
        }
    });

    state.stop.when_reached().await;
    info!("[Stream]: Stream Stopped");
    if let Err(err) = peer.close().await {
        warn!("[Stream]: failed to close stream: {err:?}");
    }

    Ok(())
}

async fn send_ws_message(sender: &mut Session, message: StreamServerMessage) -> Result<(), Closed> {
    let Ok(json) = serde_json::to_string(&message) else {
        warn!("[Stream]: failed to serialize to json");
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
