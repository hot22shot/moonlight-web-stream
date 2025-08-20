use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use actix_web::{
    Error, HttpRequest, HttpResponse, get, rt as actix_rt,
    web::{Data, Payload},
};
use actix_ws::{Closed, Message, MessageStream, Session};
use log::{debug, info, warn};
use moonlight_common::{
    MoonlightError,
    high::HostError,
    moonlight::{
        connection::{ConnectionListener, ConnectionStatus, Stage},
        stream::{ColorRange, Colorspace, MoonlightStream},
        video::SupportedVideoFormats,
    },
};
use serde::Serialize;
use tokio::{
    runtime::Handle,
    spawn,
    sync::{Notify, RwLock},
    task::spawn_blocking,
};
use webrtc::{
    api::{
        API, APIBuilder,
        interceptor_registry::register_default_interceptors,
        media_engine::{MIME_TYPE_AV1, MIME_TYPE_H264, MIME_TYPE_HEVC, MediaEngine},
        setting_engine::SettingEngine,
    },
    data_channel::RTCDataChannel,
    ice::udp_network::{EphemeralUDP, UDPNetwork},
    ice_transport::{
        ice_candidate::{RTCIceCandidate, RTCIceCandidateInit},
        ice_candidate_type::RTCIceCandidateType,
        ice_connection_state::RTCIceConnectionState,
    },
    interceptor::registry::Registry,
    peer_connection::{
        RTCPeerConnection,
        configuration::RTCConfiguration,
        peer_connection_state::RTCPeerConnectionState,
        sdp::{sdp_type::RTCSdpType, session_description::RTCSessionDescription},
    },
};

use crate::{
    Config, PortRange,
    api::stream::{
        audio::OpusTrackSampleAudioDecoder, input::StreamInput, video::TrackSampleVideoDecoder,
    },
    api_bindings::{
        RtcIceCandidate, RtcSdpType, RtcSessionDescription, StreamClientMessage,
        StreamServerGeneralMessage, StreamServerMessage, StreamSignalingMessage,
    },
    data::RuntimeApiData,
};

mod audio;
mod buffer;
pub mod cancel;
mod decoder;
mod input;
mod video;

struct StreamSettings {
    bitrate: u32,
    packet_size: u32,
    fps: u32,
    width: u32,
    height: u32,
    video_sample_queue_size: u32,
    audio_sample_queue_size: u32,
    play_audio_local: bool,
    video_supported_formats: SupportedVideoFormats,
    video_color_range_full: bool,
}

/// The stream handler WILL authenticate the client because it is a websocket
/// The Authenticator will let this route through
#[get("/host/stream")]
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
            bitrate,
            packet_size,
            fps,
            width,
            height,
            video_sample_queue_size,
            play_audio_local,
            audio_sample_queue_size,
            video_supported_formats,
            video_color_range_full,
        } = message
        else {
            let _ = session.close(None).await;
            return;
        };

        if credentials != config.credentials {
            return;
        }

        let info = StreamInfo {
            host_id: host_id as usize,
            app_id: app_id as usize,
        };

        let stream_settings = StreamSettings {
            bitrate,
            packet_size,
            fps,
            width,
            height,
            video_sample_queue_size,
            audio_sample_queue_size,
            play_audio_local,
            video_supported_formats: SupportedVideoFormats::from_bits(video_supported_formats)
                .unwrap_or_else(|| {
                    warn!("[Stream]: Received invalid supported video formats");
                    SupportedVideoFormats::H264
                }),
            video_color_range_full,
        };

        if let Err(err) = start(config, data, info, stream_settings, session.clone(), stream).await
        {
            warn!("[Stream]: stream error: {err:?}");

            let _ = session.close(None).await;
        }
    });

    Ok(response)
}

async fn start(
    config: Data<Config>,
    data: Data<RuntimeApiData>,
    info: StreamInfo,
    settings: StreamSettings,
    ws_sender: Session,
    ws_receiver: MessageStream,
) -> Result<Arc<StreamConnection>, anyhow::Error> {
    // TODO: send webrtc ice servers and other config values required for the rtc peer to the web client
    // send_ws_message(sender, message)

    // -- Configure WebRTC
    let rtc_config = RTCConfiguration {
        ice_servers: config.webrtc_ice_servers.clone(),
        ..Default::default()
    };

    let mut api_media = MediaEngine::default();
    // TODO: only register supported codecs
    api_media.register_default_codecs()?;

    let mut api_registry = Registry::new();

    // Use the default set of Interceptors
    api_registry = register_default_interceptors(api_registry, &mut api_media)?;

    let mut api_settings = SettingEngine::default();
    if let Some(PortRange { min, max }) = config.webrtc_port_range {
        match EphemeralUDP::new(min, max) {
            Ok(udp) => {
                api_settings.set_udp_network(UDPNetwork::Ephemeral(udp));
            }
            Err(err) => {
                warn!("[Stream]: Invalid port range in config: {err:?}");
            }
        }
    }
    api_settings.set_nat_1to1_ips(
        config.webrtc_nat_1to1_ips.clone(),
        RTCIceCandidateType::Host,
    );

    let api = APIBuilder::new()
        .with_media_engine(api_media)
        .with_interceptor_registry(api_registry)
        .with_setting_engine(api_settings)
        .build();

    // -- Create and Configure Peer
    let connection = StreamConnection::new(
        info,
        settings,
        data,
        ws_sender,
        ws_receiver,
        &api,
        rtc_config,
    )
    .await?;

    Ok(connection)
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
    pub runtime: Handle,
    pub info: StreamInfo,
    pub settings: StreamSettings,
    pub stages: Arc<StreamStages>,
    pub data: Data<RuntimeApiData>,
    pub peer: Arc<RTCPeerConnection>,
    pub ws_sender: Session,
    pub general_channel: Arc<RTCDataChannel>,
    // Input
    pub input: StreamInput,
    // Stream
    pub stream: RwLock<Option<MoonlightStream>>,
}

impl StreamConnection {
    pub async fn new(
        info: StreamInfo,
        settings: StreamSettings,
        data: Data<RuntimeApiData>,
        ws_sender: Session,
        mut ws_receiver: MessageStream,
        api: &API,
        config: RTCConfiguration,
    ) -> Result<Arc<Self>, anyhow::Error> {
        let peer = Arc::new(api.new_peer_connection(config).await?);

        let stages = Arc::new(StreamStages {
            connected: StreamStage::new("connected"),
            stop: StreamStage::new("stop"),
        });

        // -- Input
        let input = StreamInput::new();

        let general_channel = peer.create_data_channel("general", None).await?;

        let this = Arc::new(Self {
            runtime: Handle::current(),
            info,
            settings,
            data,
            stages,
            peer: peer.clone(),
            ws_sender,
            general_channel,
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

        Ok(this)
    }

    // -- Handle Connection State
    async fn on_ice_connection_state_change(self: &Arc<Self>, state: RTCIceConnectionState) {
        if matches!(state, RTCIceConnectionState::Connected) {
            self.stages.connected.set_reached();

            if let Err(err) = self.start_stream().await {
                warn!("[Stream]: failed to start stream: {err:?}");

                self.stop().await;
            }
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

    async fn on_negotiation_needed(&self) {
        // Empty
    }
    async fn renegotiate(&self) {
        // TODO: remove unwraps
        let local_description = self.peer.create_offer(None).await.unwrap();

        self.peer
            .set_local_description(local_description.clone())
            .await
            .unwrap();

        let _ = send_ws_message(
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

                let remote_ty = description.sdp_type;
                if let Err(err) = self.peer.set_remote_description(description).await {
                    warn!("[Signaling]: failed to set remote description: {err:?}");
                    return;
                }

                // Send an answer (local description) if we got an offer
                if remote_ty == RTCSdpType::Offer {
                    let Some(local_description) = self.make_answer().await else {
                        return;
                    };

                    debug!(
                        "[Signaling] Sending Local Description: {:?}",
                        local_description.sdp_type
                    );

                    let _ = send_ws_message(
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

        let _ = send_ws_message(&mut self.ws_sender.clone(), message).await;
    }

    // -- Data Channels
    async fn on_data_channel(self: &Arc<Self>, channel: Arc<RTCDataChannel>) {
        self.input.on_data_channel(self, channel).await;
    }

    // Start Moonlight Stream
    async fn start_stream(self: &Arc<Self>) -> Result<(), anyhow::Error> {
        let hosts = self.data.hosts.read().await;
        let Some(host) = hosts.get(self.info.host_id) else {
            let _ = send_ws_message(
                &mut self.ws_sender.clone(),
                StreamServerMessage::HostNotFound,
            )
            .await;

            todo!()
        };
        let mut host = host.lock().await;

        let app_list = host.moonlight.app_list().await?;

        let Some(app) = app_list
            .iter()
            .find(|app| app.id as usize == self.info.app_id)
        else {
            let _ = send_ws_message(
                &mut self.ws_sender.clone(),
                StreamServerMessage::InternalServerError,
            )
            .await;

            todo!()
        };

        // Send App Update
        let app = app.to_owned();
        spawn({
            let mut sender = self.ws_sender.clone();
            async move {
                let _ = send_ws_message(
                    &mut sender,
                    StreamServerMessage::UpdateApp { app: app.into() },
                )
                .await;
            }
        });

        let gamepads = self.input.active_gamepads.read().await;

        let video_decoder = TrackSampleVideoDecoder::new(
            self.clone(),
            self.settings.video_supported_formats,
            self.settings.video_sample_queue_size as usize,
        );

        let audio_decoder = OpusTrackSampleAudioDecoder::new(
            self.clone(),
            self.settings.audio_sample_queue_size as usize,
        );

        let connection_listener = StreamConnectionListener {
            stream: self.clone(),
        };

        let stream = match host
            .moonlight
            .start_stream(
                &self.data.instance,
                &self.data.crypto,
                self.info.app_id as u32,
                self.settings.width,
                self.settings.height,
                self.settings.fps,
                false,
                true,
                self.settings.play_audio_local,
                *gamepads,
                false,
                Colorspace::Rec709,
                if self.settings.video_color_range_full {
                    ColorRange::Full
                } else {
                    ColorRange::Limited
                },
                self.settings.bitrate,
                self.settings.packet_size,
                connection_listener,
                video_decoder,
                audio_decoder,
            )
            .await
        {
            Ok(value) => value,
            Err(err) => {
                warn!("[Stream]: failed to start moonlight stream: {err:?}");

                #[allow(clippy::single_match)]
                match err {
                    HostError::Moonlight(MoonlightError::ConnectionAlreadyExists) => {
                        let _ = send_ws_message(
                            &mut self.ws_sender.clone(),
                            StreamServerMessage::AlreadyStreaming,
                        )
                        .await;
                    }
                    _ => {}
                }

                return Err(err.into());
            }
        };

        self.input.on_stream_start(&stream).await;

        drop(gamepads);

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
            let _ = send_ws_message(&mut ws_sender, StreamServerMessage::PeerDisconnect).await;
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
}

// TODO: move this into connection.rs
struct StreamConnectionListener {
    stream: Arc<StreamConnection>,
}

impl ConnectionListener for StreamConnectionListener {
    fn stage_starting(&mut self, stage: Stage) {
        let mut ws_sender = self.stream.ws_sender.clone();

        self.stream.runtime.spawn(async move {
            let _ = send_ws_message(
                &mut ws_sender,
                StreamServerMessage::StageStarting {
                    stage: stage.name().to_string(),
                },
            )
            .await;
        });
    }

    fn stage_complete(&mut self, stage: Stage) {
        let mut ws_sender = self.stream.ws_sender.clone();

        self.stream.runtime.spawn(async move {
            let _ = send_ws_message(
                &mut ws_sender,
                StreamServerMessage::StageComplete {
                    stage: stage.name().to_string(),
                },
            )
            .await;
        });
    }

    fn stage_failed(&mut self, stage: Stage, error_code: i32) {
        let mut ws_sender = self.stream.ws_sender.clone();

        self.stream.runtime.spawn(async move {
            let _ = send_ws_message(
                &mut ws_sender,
                StreamServerMessage::StageFailed {
                    stage: stage.name().to_string(),
                    error_code,
                },
            )
            .await;
        });
    }

    fn connection_started(&mut self) {
        let mut ws_sender = self.stream.ws_sender.clone();

        self.stream.runtime.spawn(async move {
            let _ = send_ws_message(&mut ws_sender, StreamServerMessage::ConnectionComplete).await;
        });

        // Renegotate because we now have the audio and video streams
        let stream = self.stream.clone();
        self.stream.runtime.spawn(async move {
            stream.renegotiate().await;
        });
    }

    fn connection_terminated(&mut self, error_code: i32) {
        let mut ws_sender = self.stream.ws_sender.clone();

        self.stream.runtime.spawn(async move {
            let _ = send_ws_message(
                &mut ws_sender,
                StreamServerMessage::ConnectionTerminated { error_code },
            )
            .await;
        });

        let stream = self.stream.clone();
        self.stream.runtime.spawn(async move {
            if let Some(message) = serialize_json(&StreamServerGeneralMessage::ConnectionTerminated)
            {
                let _ = stream.general_channel.send_text(message).await;
            }
        });
    }

    fn log_message(&mut self, message: &str) {
        info!("[Moonlight Stream]: {}", message.trim());
    }

    fn connection_status_update(&mut self, status: ConnectionStatus) {
        let stream = self.stream.clone();
        self.stream.runtime.spawn(async move {
            if let Some(message) =
                serialize_json(&StreamServerGeneralMessage::ConnectionStatusUpdate {
                    status: status.into(),
                })
            {
                let _ = stream.general_channel.send_text(message).await;
            }
        });
    }

    fn set_hdr_mode(&mut self, _hdr_enabled: bool) {}

    fn controller_rumble(
        &mut self,
        controller_number: u16,
        low_frequency_motor: u16,
        high_frequency_motor: u16,
    ) {
        let stream = self.stream.clone();

        self.stream.runtime.spawn(async move {
            stream
                .input
                .send_controller_rumble(
                    controller_number as u8,
                    low_frequency_motor,
                    high_frequency_motor,
                )
                .await;
        });
    }

    fn controller_rumble_triggers(
        &mut self,
        controller_number: u16,
        left_trigger_motor: u16,
        right_trigger_motor: u16,
    ) {
        let stream = self.stream.clone();

        self.stream.runtime.spawn(async move {
            stream
                .input
                .send_controller_trigger_rumble(
                    controller_number as u8,
                    left_trigger_motor,
                    right_trigger_motor,
                )
                .await;
        });
    }

    fn controller_set_motion_event_state(
        &mut self,
        _controller_number: u16,
        _motion_type: u8,
        _report_rate_hz: u16,
    ) {
        // unsupported: https://github.com/w3c/gamepad/issues/211
    }

    fn controller_set_adaptive_triggers(
        &mut self,
        _controller_number: u16,
        _event_flags: u8,
        _type_left: u8,
        _type_right: u8,
        _left: &mut u8,
        _right: &mut u8,
    ) {
        // unsupported
    }

    fn controller_set_led(&mut self, _controller_number: u16, _r: u8, _g: u8, _b: u8) {
        // unsupported
    }
}

fn serialize_json<T>(message: &T) -> Option<String>
where
    T: Serialize,
{
    let Ok(json) = serde_json::to_string(&message) else {
        warn!("[Stream]: failed to serialize to json");
        return None;
    };

    Some(json)
}

async fn send_ws_message(sender: &mut Session, message: StreamServerMessage) -> Result<(), Closed> {
    let Some(json) = serialize_json(&message) else {
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
