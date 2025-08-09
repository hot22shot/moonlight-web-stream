use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use actix_web::{
    Error, HttpRequest, HttpResponse, get, rt as actix_rt,
    web::{Data, Payload},
};
use actix_ws::{Closed, Message, MessageStream, Session};
use log::{debug, info, warn};
use moonlight_common::{
    debug::DebugHandler,
    high::StreamError,
    stream::{ColorRange, Colorspace, MoonlightStream},
    video::SupportedVideoFormats,
};
use tokio::{
    io::Take,
    runtime::Handle,
    spawn,
    sync::{Mutex, Notify, RwLock},
    task::{spawn_blocking, spawn_local},
    time::sleep,
};
use webrtc::{
    api::{
        API, APIBuilder,
        interceptor_registry::register_default_interceptors,
        media_engine::{
            MIME_TYPE_AV1, MIME_TYPE_H264, MIME_TYPE_HEVC, MIME_TYPE_OPUS, MediaEngine,
        },
    },
    data_channel::RTCDataChannel,
    ice_transport::{
        ice_candidate::{RTCIceCandidate, RTCIceCandidateInit},
        ice_connection_state::RTCIceConnectionState,
        ice_server::RTCIceServer,
    },
    interceptor::registry::Registry,
    peer_connection::{
        RTCPeerConnection,
        configuration::RTCConfiguration,
        peer_connection_state::RTCPeerConnectionState,
        sdp::{sdp_type::RTCSdpType, session_description::RTCSessionDescription},
        signaling_state::RTCSignalingState,
    },
    rtp_transceiver::{rtp_codec::RTCRtpCodecCapability, rtp_sender::RTCRtpSender},
    track::track_local::track_local_static_sample::TrackLocalStaticSample,
};

use crate::{
    Config,
    api::stream::{
        audio::OpusTrackSampleAudioDecoder, input::StreamInput, video::H264TrackSampleVideoDecoder,
    },
    api_bindings::{
        RtcIceCandidate, RtcSdpType, RtcSessionDescription, StreamClientMessage,
        StreamServerMessage, StreamSignalingMessage,
    },
    data::RuntimeApiData,
};

mod audio;
mod buffer;
mod connection;
mod input;
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

struct StreamStages {
    pub connected: StreamStage,
    pub stop: StreamStage,
}
struct StreamInfo {
    pub host_id: usize,
    pub app_id: usize,
}

struct StreamConnection {
    pub info: StreamInfo,
    pub runtime: Handle,
    pub stages: Arc<StreamStages>,
    pub data: Data<RuntimeApiData>,
    pub peer: Arc<RTCPeerConnection>,
    pub ws_sender: Session,
    // Signaling
    pub making_offer: AtomicBool,
    pub setting_remote_answer_pending: AtomicBool,
    // Video
    pub video_track: Arc<TrackLocalStaticSample>,
    // Audio
    pub audio_track: Arc<TrackLocalStaticSample>,
    // Input
    pub input: StreamInput,
    // Stream
    pub stream: RwLock<Option<MoonlightStream>>,
}

const POLITE: bool = false;

impl StreamConnection {
    pub async fn new(
        info: StreamInfo,
        data: Data<RuntimeApiData>,
        ws_sender: Session,
        mut ws_receiver: MessageStream,
        api: &API,
        config: RTCConfiguration,
    ) -> Result<Arc<Self>, anyhow::Error> {
        let peer = Arc::new(api.new_peer_connection(config).await?);

        // -- Create and Add a video track
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
        let video_sender = peer.add_track(Arc::clone(&video_track) as Arc<_>).await?;

        // Read incoming RTCP packets
        // Before these packets are returned they are processed by interceptors. For things
        // like NACK this needs to be called.
        spawn(async move {
            let mut rtcp_buf = vec![0u8; 1500];
            while let Ok((_, _)) = video_sender.read(&mut rtcp_buf).await {}
        });

        // -- Create and Add a audio track
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
        // let audio_sender = peer.add_track(Arc::clone(&audio_track) as Arc<_>).await?;

        // // Read incoming RTCP packets
        // // Before these packets are returned they are processed by interceptors. For things
        // // like NACK this needs to be called.
        // spawn(async move {
        //     let mut rtcp_buf = vec![0u8; 1500];
        //     while let Ok((_, _)) = audio_sender.read(&mut rtcp_buf).await {}
        // });

        // -- Input
        let input = StreamInput::new();

        let this = Arc::new(Self {
            info,
            runtime: Handle::current(),
            data,
            stages: Arc::new(StreamStages {
                connected: StreamStage::new("connected"),
                stop: StreamStage::new("stop"),
            }),
            peer: peer.clone(),
            ws_sender,
            making_offer: AtomicBool::new(false),
            setting_remote_answer_pending: AtomicBool::new(false),
            video_track,
            audio_track,
            input,
            stream: Default::default(),
        });

        // -- Connection state
        peer.on_ice_connection_state_change({
            let this = this.clone();
            Box::new(move |state| {
                let this = this.clone();
                Box::pin(async move {
                    this.on_ice_connection_state_change(state).await;
                })
            })
        });
        peer.on_peer_connection_state_change({
            let this = this.clone();
            Box::new(move |state| {
                let this = this.clone();
                Box::pin(async move {
                    this.on_peer_connection_state_change(state).await;
                })
            })
        });

        // -- Signaling
        peer.on_negotiation_needed({
            let this = this.clone();
            Box::new(move || {
                let this = this.clone();
                Box::pin(async move {
                    this.on_negotiation_needed().await;
                })
            })
        });
        peer.on_ice_candidate({
            let this = this.clone();
            Box::new(move |candidate| {
                let this = this.clone();
                Box::pin(async move {
                    this.on_ice_candidate(candidate).await;
                })
            })
        });
        actix_rt::spawn({
            let this = this.clone();
            async move {
                while let Some(Ok(Message::Text(text))) = ws_receiver.recv().await {
                    let this = this.clone();

                    this.on_ws_message(&text).await;
                }
            }
        });

        // -- Data Channels
        peer.on_data_channel({
            let this = this.clone();
            Box::new(move |channel| {
                let this = this.clone();
                Box::pin(async move {
                    this.on_data_channel(channel).await;
                })
            })
        });

        let none = peer.create_data_channel("none", None).await?;
        none.on_open({
            let none = none.clone();
            Box::new(move || {
                let none = none.clone();

                Box::pin(async move {
                    none.send_text("Test").await.unwrap();
                })
            })
        });

        Ok(this)
    }

    // -- Handle Connection State
    async fn on_ice_connection_state_change(self: &Arc<Self>, state: RTCIceConnectionState) {
        if matches!(state, RTCIceConnectionState::Connected) {
            self.stages.connected.set_reached();

            let this = self.clone();

            // spawn(async move {
            //     if let Err(err) = this.start_stream().await {
            //         warn!("[Stream]: failed to start stream: {err}");
            //         this.stop().await;
            //     }
            // });
        }
    }
    async fn on_peer_connection_state_change(&self, state: RTCPeerConnectionState) {
        if matches!(
            state,
            RTCPeerConnectionState::Failed
                | RTCPeerConnectionState::Disconnected
                | RTCPeerConnectionState::Closed
        ) {
            self.stop().await;
        }
    }

    // -- Handle Signaling
    async fn make_answer(&self) -> Option<RTCSessionDescription> {
        let local_description = match self.peer.create_answer(None).await {
            Err(err) => {
                warn!("[Signaling]: failed to create answer: {err:?}");
                return None;
            }
            Ok(value) => value,
        };

        if let Err(err) = self
            .peer
            .set_local_description(local_description.clone())
            .await
        {
            warn!("[Signaling]: failed to set local description: {err:?}");
            return None;
        }

        Some(local_description)
    }
    async fn make_offer(&self) -> Option<RTCSessionDescription> {
        let local_description = match self.peer.create_offer(None).await {
            Err(err) => {
                warn!("[Signaling]: failed to create offer: {err:?}");
                return None;
            }
            Ok(value) => value,
        };

        if let Err(err) = self
            .peer
            .set_local_description(local_description.clone())
            .await
        {
            warn!("[Signaling]: failed to set local description: {err:?}");
            return None;
        }

        Some(local_description)
    }
    async fn set_local_description(&self) -> Option<RTCSessionDescription> {
        match self.peer.signaling_state() {
            RTCSignalingState::HaveRemoteOffer => self.make_answer().await,
            RTCSignalingState::Stable => self.make_offer().await,
            _ => {
                warn!("[Signaling]: Not in a valid state to set local description");
                None
            }
        }
    }

    async fn on_negotiation_needed(&self) {
        debug!("[Signaling] Negotiation Needed");

        self.making_offer.store(true, Ordering::Release);

        let Some(local_description) = self.set_local_description().await else {
            return;
        };

        debug!(
            "[Signaling] Sending Local Description: {:?}",
            local_description.sdp_type
        );

        let _ = Self::send_ws_message(
            &mut self.ws_sender.clone(),
            StreamServerMessage::Signaling(StreamSignalingMessage::Description(
                RtcSessionDescription {
                    ty: local_description.sdp_type.into(),
                    sdp: local_description.sdp,
                },
            )),
        )
        .await;

        self.making_offer.store(false, Ordering::Release);
    }

    async fn on_ws_message(&self, text: &str) {
        let Ok(message) = serde_json::from_str::<StreamClientMessage>(text) else {
            warn!("[Stream]: failed to deserialize from json");
            return;
        };

        match message {
            StreamClientMessage::Signaling(StreamSignalingMessage::Description(description)) => {
                debug!(
                    "[Signaling] Received Remote Description: {:?}",
                    description.ty
                );

                let making_offer = self.making_offer.load(Ordering::Acquire);
                let setting_remote_answer_pending =
                    self.setting_remote_answer_pending.load(Ordering::Acquire);

                let ready_for_offer = !making_offer
                    && (self.peer.signaling_state() == RTCSignalingState::Stable
                        || setting_remote_answer_pending);

                let offer_collision = description.ty == RtcSdpType::Offer && !ready_for_offer;
                let ignore_offer = !POLITE && offer_collision;

                // Ignore Offer if we're impolite
                if ignore_offer {
                    return;
                }

                let description = match &description.ty {
                    RtcSdpType::Offer => RTCSessionDescription::offer(description.sdp),
                    RtcSdpType::Answer => RTCSessionDescription::answer(description.sdp),
                    RtcSdpType::Pranswer => RTCSessionDescription::pranswer(description.sdp),
                    _ => {
                        warn!(
                            "[Signaling]: failed to handle RTCSdpType {:?}",
                            description.ty
                        );
                        return;
                    }
                };

                let Ok(description) = description else {
                    warn!("[Signaling]: Received invalid RTCSessionDescription");
                    return;
                };

                // Set the remote description
                self.setting_remote_answer_pending.store(
                    description.sdp_type == RTCSdpType::Answer,
                    Ordering::Release,
                );

                let remote_ty = description.sdp_type;
                if let Err(err) = self.peer.set_remote_description(description).await {
                    self.setting_remote_answer_pending
                        .store(false, Ordering::Release);

                    warn!("[Signaling]: failed to set remote description: {err:?}");
                    return;
                }

                self.setting_remote_answer_pending
                    .store(false, Ordering::Release);

                // Send an answer (local description) if we got an offer
                if remote_ty == RTCSdpType::Offer {
                    let Some(local_description) = self.set_local_description().await else {
                        return;
                    };

                    debug!(
                        "[Signaling] Sending Local Description: {:?}",
                        local_description.sdp_type
                    );

                    let _ = Self::send_ws_message(
                        &mut self.ws_sender.clone(),
                        StreamServerMessage::Signaling(StreamSignalingMessage::Description(
                            RtcSessionDescription {
                                ty: local_description.sdp_type.into(),
                                sdp: local_description.sdp,
                            },
                        )),
                    )
                    .await;
                }
            }
            StreamClientMessage::Signaling(StreamSignalingMessage::AddIceCandidate(
                description,
            )) => {
                debug!("[Signaling] Received Ice Candidate");

                if let Err(err) = self
                    .peer
                    .add_ice_candidate(RTCIceCandidateInit {
                        candidate: description.candidate,
                        sdp_mid: description.sdp_mid,
                        sdp_mline_index: description.sdp_mline_index,
                        username_fragment: description.username_fragment,
                    })
                    .await
                {
                    warn!("[Signaling]: failed to add ice candidate: {err:?}");
                }
            }
            // This should already be done
            StreamClientMessage::AuthenticateAndInit { .. } => {}
        }
    }

    async fn on_ice_candidate(&self, candidate: Option<RTCIceCandidate>) {
        debug!(
            "[Signaling] Sending Ice Candidate, is last: {}",
            candidate.is_none()
        );

        let Some(candidate) = candidate else {
            return;
        };

        let Ok(candidate_json) = candidate.to_json() else {
            return;
        };

        let message = StreamServerMessage::Signaling(StreamSignalingMessage::AddIceCandidate(
            RtcIceCandidate {
                candidate: candidate_json.candidate,
                sdp_mid: candidate_json.sdp_mid,
                sdp_mline_index: candidate_json.sdp_mline_index,
                username_fragment: candidate_json.username_fragment,
            },
        ));

        let _ = Self::send_ws_message(&mut self.ws_sender.clone(), message).await;
    }

    // -- Data Channels
    async fn on_data_channel(&self, channel: Arc<RTCDataChannel>) {
        self.input.on_data_channel(channel);
    }

    // Start Moonlight Stream
    async fn start_stream(&self) -> Result<(), anyhow::Error> {
        let hosts = self.data.hosts.read().await;
        let Some(host) = hosts.get(self.info.host_id) else {
            let _ = Self::send_ws_message(
                &mut self.ws_sender.clone(),
                StreamServerMessage::HostNotFound,
            )
            .await;

            todo!()
        };
        let mut host = host.lock().await;

        let Some(result) = host.moonlight.app_list().await else {
            let _ = Self::send_ws_message(
                &mut self.ws_sender.clone(),
                StreamServerMessage::AppNotFound,
            )
            .await;

            todo!()
        };
        let app_list = result?;

        let Some(app) = app_list
            .into_iter()
            .find(|app| app.id as usize == self.info.app_id)
        else {
            let _ = Self::send_ws_message(
                &mut self.ws_sender.clone(),
                StreamServerMessage::InternalServerError,
            )
            .await;

            todo!()
        };

        // Send App Update
        spawn({
            let mut sender = self.ws_sender.clone();
            async move {
                let _ = Self::send_ws_message(
                    &mut sender,
                    StreamServerMessage::UpdateApp { app: app.into() },
                )
                .await;
            }
        });

        let video_decoder =
            H264TrackSampleVideoDecoder::new(self.video_track.clone(), self.stages.clone());
        let audio_decoder =
            OpusTrackSampleAudioDecoder::new(self.audio_track.clone(), self.stages.clone());

        let stream = match host
            .moonlight
            .start_stream(
                &self.data.instance,
                &self.data.crypto,
                self.info.app_id as u32,
                2560,
                1440,
                60,
                Colorspace::Rec709,
                ColorRange::Limited,
                10000,
                2048,
                DebugHandler,
                video_decoder,
                audio_decoder,
            )
            .await
        {
            Some(Ok(value)) => value,
            Some(Err(err)) => {
                warn!("[Stream]: failed to start moonlight stream: {err:?}");

                #[allow(clippy::single_match)]
                match err {
                    StreamError::Moonlight(moonlight_common::Error::ConnectionAlreadyExists) => {
                        let _ = Self::send_ws_message(
                            &mut self.ws_sender.clone(),
                            StreamServerMessage::AlreadyStreaming,
                        )
                        .await;
                    }
                    _ => {}
                }

                return Err(err.into());
            }
            None => todo!(),
        };

        let mut stream_guard = self.stream.write().await;
        stream_guard.replace(stream);

        Ok(())
    }

    async fn stop(&self) {
        self.stages.stop.set_reached();

        // Sometimes we don't connect before failing
        self.stages.connected.set_reached();

        let mut ws_sender = self.ws_sender.clone();
        spawn(async move {
            let _ =
                Self::send_ws_message(&mut ws_sender, StreamServerMessage::PeerDisconnect).await;
            let _ = ws_sender.close(None).await;
        });

        let stream = {
            let mut stream = self.stream.write().await;
            stream.take()
        };
        if let Err(err) = spawn_blocking(move || {
            drop(stream);
        })
        .await
        {
            warn!("[Stream]: failed to stop stream: {err}");
        };
    }

    async fn send_ws_message(
        sender: &mut Session,
        message: StreamServerMessage,
    ) -> Result<(), Closed> {
        let Ok(json) = serde_json::to_string(&message) else {
            warn!("[Stream]: failed to serialize to json");
            return Ok(());
        };

        sender.text(json).await
    }
}

async fn start(
    data: Data<RuntimeApiData>,
    host_id: usize,
    app_id: usize,
    ws_sender: Session,
    ws_receiver: MessageStream,
) -> Result<(), anyhow::Error> {
    // -- Configure WebRTC
    let config = RTCConfiguration {
        // TODO: put this into config
        // ice_servers: vec![
        //     RTCIceServer {
        //         urls: vec!["stun:stun.l.google.com:19302".to_owned()],
        //         ..Default::default()
        //     },
        //     RTCIceServer {
        //         urls: vec!["stun:stun.l.google.com:19302".to_owned()],
        //         ..Default::default()
        //     },
        //     RTCIceServer {
        //         urls: vec!["stun:stun.l.google.com:5349".to_owned()],
        //         ..Default::default()
        //     },
        //     RTCIceServer {
        //         urls: vec!["stun:stun1.l.google.com:3478".to_owned()],
        //         ..Default::default()
        //     },
        //     RTCIceServer {
        //         urls: vec!["stun:stun1.l.google.com:5349".to_owned()],
        //         ..Default::default()
        //     },
        //     RTCIceServer {
        //         urls: vec!["stun:stun2.l.google.com:19302".to_owned()],
        //         ..Default::default()
        //     },
        //     RTCIceServer {
        //         urls: vec!["stun:stun2.l.google.com:5349".to_owned()],
        //         ..Default::default()
        //     },
        //     RTCIceServer {
        //         urls: vec!["stun:stun3.l.google.com:3478".to_owned()],
        //         ..Default::default()
        //     },
        //     RTCIceServer {
        //         urls: vec!["stun:stun3.l.google.com:5349".to_owned()],
        //         ..Default::default()
        //     },
        //     RTCIceServer {
        //         urls: vec!["stun:stun4.l.google.com:19302".to_owned()],
        //         ..Default::default()
        //     },
        //     RTCIceServer {
        //         urls: vec!["stun:stun4.l.google.com:5349".to_owned()],
        //         ..Default::default()
        //     },
        // ],
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
    let connection = StreamConnection::new(
        StreamInfo { host_id, app_id },
        data,
        ws_sender,
        ws_receiver,
        &api,
        config,
    )
    .await?;

    connection.stages.stop.when_reached().await;

    Ok(())
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
