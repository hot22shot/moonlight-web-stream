#![feature(if_let_guard)]
#![feature(async_fn_traits)]

use std::{
    panic,
    process::exit,
    sync::{
        Arc, Weak,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

use common::{
    StreamSettings,
    ipc::{
        IpcReceiver, IpcSender, ServerIpcMessage, StreamerConfig, StreamerIpcMessage,
        create_process_ipc,
    },
};
use log::{LevelFilter, debug, info, warn};
use moonlight_common::{
    MoonlightError,
    high::{HostError, MoonlightHost},
    network::backend::reqwest::ReqwestClient,
    pair::ClientAuth,
    stream::{
        MoonlightInstance, MoonlightStream,
        audio::AudioDecoder,
        bindings::{
            ActiveGamepads, AudioConfig, Capabilities, ColorRange, ConnectionStatus, DecodeResult,
            EncryptionFlags, HostFeatures, OpusMultistreamConfig, Stage, SupportedVideoFormats,
            VideoDecodeUnit, VideoFormat,
        },
        connection::ConnectionListener,
        video::{VideoDecoder, VideoSetup},
    },
};
use simplelog::{ColorChoice, TermLogger, TerminalMode};
use tokio::{
    io::{stdin, stdout},
    runtime::Handle,
    spawn,
    sync::{Mutex, RwLock},
    time::sleep,
};

use common::api_bindings::{StreamCapabilities, StreamServerMessage};

use crate::transport::{
    InboundPacket, OutboundPacket, TransportEvent, TransportEvents, TransportSender, webrtc,
};

// TODO: deasyncify this, just remove most of the async code into sync code

pub type RequestClient = ReqwestClient;

mod buffer;
mod convert;
mod transport;

#[tokio::main]
async fn main() {
    let default_panic = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        default_panic(info);
        exit(0);
    }));

    // At this point we're authenticated
    let (mut ipc_sender, mut ipc_receiver) =
        create_process_ipc::<ServerIpcMessage, StreamerIpcMessage>(stdin(), stdout()).await;

    // Send stage
    ipc_sender
        .send(StreamerIpcMessage::WebSocket(
            StreamServerMessage::StageComplete {
                stage: "Launch Streamer".to_string(),
            },
        ))
        .await;

    let (
        config,
        stream_settings,
        host_address,
        host_http_port,
        client_unique_id,
        client_private_key,
        client_certificate,
        server_certificate,
        app_id,
    ) = loop {
        match ipc_receiver.recv().await {
            Some(ServerIpcMessage::Init {
                config,
                stream_settings,
                host_address,
                host_http_port,
                client_unique_id,
                client_private_key,
                client_certificate,
                server_certificate,
                app_id,
            }) => {
                debug!(
                    "Client supported codecs: {:?}",
                    stream_settings
                        .video_supported_formats
                        .iter_names()
                        .collect::<Vec<_>>()
                );

                break (
                    config,
                    stream_settings,
                    host_address,
                    host_http_port,
                    client_unique_id,
                    client_private_key,
                    client_certificate,
                    server_certificate,
                    app_id,
                );
            }
            _ => continue,
        }
    };

    TermLogger::init(
        config.log_level,
        simplelog::ConfigBuilder::new()
            .add_filter_ignore_str("webrtc_sctp")
            .set_time_level(LevelFilter::Off)
            .build(),
        TerminalMode::Stderr,
        ColorChoice::Never,
    )
    .expect("failed to init logger");

    // Send stage
    ipc_sender
        .send(StreamerIpcMessage::WebSocket(
            StreamServerMessage::StageStarting {
                stage: "Setup WebRTC Peer".to_string(),
            },
        ))
        .await;

    // -- Create the host and pair it
    let mut host = MoonlightHost::new(host_address, host_http_port, client_unique_id)
        .expect("failed to create host");

    host.set_pairing_info(
        &ClientAuth {
            certificate: client_certificate,
            private_key: client_private_key,
        },
        &server_certificate,
    )
    .expect("failed to set pairing info");

    // -- Configure moonlight
    let moonlight = MoonlightInstance::global().expect("failed to find moonlight");

    // -- Create and Configure Peer
    let connection = StreamConnection::new(
        moonlight,
        StreamInfo {
            host: Mutex::new(host),
            app_id,
        },
        stream_settings,
        ipc_sender.clone(),
        ipc_receiver,
        config,
    )
    .await
    .expect("failed to create connection");

    // Send stage
    ipc_sender
        .send(StreamerIpcMessage::WebSocket(
            StreamServerMessage::StageComplete {
                stage: "Setup WebRTC Peer".to_string(),
            },
        ))
        .await;

    // Send stage
    ipc_sender
        .send(StreamerIpcMessage::WebSocket(
            StreamServerMessage::StageStarting {
                stage: "WebRTC Peer Negotiation".to_string(),
            },
        ))
        .await;

    // Wait for termination
    // TODO
    // connection.terminate.notified().await;
    sleep(Duration::from_secs(100)).await;

    // Exit streamer
    exit(0);
}

struct StreamInfo {
    host: Mutex<MoonlightHost<RequestClient>>,
    app_id: u32,
}

struct StreamConnection {
    pub runtime: Handle,
    pub moonlight: MoonlightInstance,
    pub info: StreamInfo,
    pub settings: StreamSettings,
    pub ipc_sender: IpcSender<StreamerIpcMessage>,
    // Video
    pub stream_info: Mutex<Option<VideoSetup>>,
    // Stream
    pub stream: RwLock<Option<MoonlightStream>>,
    pub transport_sender: Mutex<Box<dyn TransportSender + Send + Sync>>,
    is_terminating: AtomicBool,
}

impl StreamConnection {
    pub async fn new(
        moonlight: MoonlightInstance,
        info: StreamInfo,
        settings: StreamSettings,
        ipc_sender: IpcSender<StreamerIpcMessage>,
        mut ipc_receiver: IpcReceiver<ServerIpcMessage>,
        config: StreamerConfig,
    ) -> Result<Arc<Self>, anyhow::Error> {
        let (sender, mut events) = webrtc::new(settings.clone(), &config.webrtc).await?;

        let this = Arc::new(Self {
            runtime: Handle::current(),
            moonlight,
            info,
            settings,
            ipc_sender,
            stream_info: Mutex::new(None),
            stream: RwLock::new(None),
            transport_sender: Mutex::new(Box::new(sender)),
            is_terminating: AtomicBool::new(false),
        });

        thread::spawn({
            let runtime = this.runtime.clone();
            let mut ipc_sender = this.ipc_sender.clone();
            let this = Arc::downgrade(&this);

            move || {
                // TODO: this is blocking
                loop {
                    match events.poll_event() {
                        Ok(value) => match value {
                            TransportEvent::SendIpc(message) => {
                                ipc_sender.blocking_send(message);
                            }
                            TransportEvent::StartStream { settings } => {
                                let this = this.upgrade().unwrap();

                                runtime.spawn(async move {
                                    this.start_stream().await.unwrap();
                                });
                            }
                            TransportEvent::RecvPacket(packet) => {
                                let this = this.upgrade().unwrap();

                                this.on_packet(packet);
                            }
                            TransportEvent::Closed => todo!(),
                        },
                        Err(err) => {
                            todo!();
                        }
                    }
                }
            }
        });

        spawn({
            let this = Arc::downgrade(&this);

            async move {
                while let Some(message) = ipc_receiver.recv().await {
                    let Some(this) = this.upgrade() else {
                        debug!("Received ipc message while the main type is already deallocated");
                        return;
                    };

                    // TODO: it should be blocking?
                    if let ServerIpcMessage::Stop = &message {
                        this.on_ipc_message(ServerIpcMessage::Stop).await;
                        return;
                    }

                    this.on_ipc_message(message).await;
                }
            }
        });

        Ok(this)
    }

    fn on_packet(&self, packet: InboundPacket) {
        let stream = self.stream.blocking_read();
        let stream = stream.as_ref().unwrap();

        match packet {
            InboundPacket::General { message } => todo!(),
            InboundPacket::MousePosition {
                x,
                y,
                reference_width,
                reference_height,
            } => {
                stream
                    .send_mouse_position(x, y, reference_width, reference_height)
                    .unwrap();
            }
            InboundPacket::MouseButton { action, button } => {
                stream.send_mouse_button(action, button).unwrap();
            }
            InboundPacket::MouseMove { delta_x, delta_y } => {
                stream.send_mouse_move(delta_x, delta_y).unwrap();
            }
            InboundPacket::HighResScroll { delta_x, delta_y } => {
                if delta_y != 0 {
                    stream.send_high_res_scroll(delta_y).unwrap();
                }
                if delta_x != 0 {
                    stream.send_high_res_horizontal_scroll(delta_x).unwrap();
                }
            }
            InboundPacket::Scroll { delta_x, delta_y } => {
                if delta_y != 0 {
                    stream.send_scroll(delta_y).unwrap();
                }
                if delta_x != 0 {
                    stream.send_horizontal_scroll(delta_x).unwrap();
                }
            }
            InboundPacket::Key {
                action,
                modifiers,
                key,
                flags,
            } => {
                stream
                    .send_keyboard_event_non_standard(key as i16, action, modifiers, flags)
                    .unwrap();
            }
            InboundPacket::Text { text } => {
                stream.send_text(&text).unwrap();
            }
            InboundPacket::Touch {
                pointer_id,
                x,
                y,
                pressure_or_distance,
                contact_area_major,
                contact_area_minor,
                rotation,
                event_type,
            } => {
                stream
                    .send_touch(
                        pointer_id,
                        x,
                        y,
                        pressure_or_distance,
                        contact_area_major,
                        contact_area_minor,
                        rotation,
                        event_type,
                    )
                    .unwrap();
            }
            InboundPacket::ControllerConnected {
                id,
                ty,
                supported_buttons,
                capabilities,
            } => {
                // TODO
                todo!();
            }
            InboundPacket::ControllerDisconnected { id } => {
                // TODO
                todo!();
            }
            InboundPacket::ControllerState {
                id,
                buttons,
                left_trigger,
                right_trigger,
                left_stick_x,
                left_stick_y,
                right_stick_x,
                right_stick_y,
            } => {
                // TODO
                todo!();
            }
        }
    }

    async fn on_ipc_message(self: &Arc<Self>, message: ServerIpcMessage) {
        let this = self.clone();

        self.runtime.spawn_blocking(move || {
            let mut sender = this.transport_sender.blocking_lock();

            sender.on_ipc_message(message);
        });
    }

    // Start Moonlight Stream
    async fn start_stream(self: &Arc<Self>) -> Result<(), anyhow::Error> {
        // Send stage
        let mut ipc_sender = self.ipc_sender.clone();
        ipc_sender
            .send(StreamerIpcMessage::WebSocket(
                StreamServerMessage::StageStarting {
                    stage: "Moonlight Stream".to_string(),
                },
            ))
            .await;

        let mut host = self.info.host.lock().await;

        let video_decoder = StreamVideoDecoder {
            stream: Arc::downgrade(self),
            supported_formats: SupportedVideoFormats::empty(),
        };

        let audio_decoder = StreamAudioDecoder {
            stream: Arc::downgrade(self),
        };

        let connection_listener = StreamConnectionListener {
            stream: Arc::downgrade(self),
        };

        let stream = match host
            .start_stream(
                &self.moonlight,
                self.info.app_id,
                self.settings.width,
                self.settings.height,
                self.settings.fps,
                false,
                true,
                self.settings.play_audio_local,
                ActiveGamepads::empty(),
                false,
                self.settings.video_colorspace,
                if self.settings.video_color_range_full {
                    ColorRange::Full
                } else {
                    ColorRange::Limited
                },
                self.settings.bitrate,
                self.settings.packet_size,
                EncryptionFlags::all(),
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
                        ipc_sender
                            .send(StreamerIpcMessage::WebSocket(
                                StreamServerMessage::AlreadyStreaming,
                            ))
                            .await;
                    }
                    _ => {}
                }

                return Err(err.into());
            }
        };

        let host_features = stream.host_features().unwrap_or_else(|err| {
            warn!("[Stream]: failed to get host features: {err:?}");
            HostFeatures::empty()
        });

        let capabilities = StreamCapabilities {
            touch: host_features.contains(HostFeatures::PEN_TOUCH_EVENTS),
        };

        let video_setup = {
            let video_setup = self.stream_info.lock().await;
            video_setup.unwrap_or_else(|| {
                warn!("failed to query video setup information. Giving the browser guessed information");
                VideoSetup { format: VideoFormat::H264, width: self.settings.width, height: self.settings.height, redraw_rate: self.settings.fps, flags: 0 }
            })
        };

        spawn(async move {
            ipc_sender
                .send(StreamerIpcMessage::WebSocket(
                    StreamServerMessage::ConnectionComplete {
                        capabilities,
                        format: video_setup.format as u32,
                        width: video_setup.width,
                        height: video_setup.height,
                        fps: video_setup.redraw_rate,
                    },
                ))
                .await;
        });

        let mut stream_guard = self.stream.write().await;
        stream_guard.replace(stream);

        Ok(())
    }

    fn stop(&self) {
        if self
            .is_terminating
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            debug!("[Stream]: stream is already terminating, won't stop twice");
            return;
        }

        debug!("[Stream]: Stopping...");

        let stream = {
            let mut stream = self.stream.blocking_write();
            stream.take()
        };
        drop(stream);

        let mut ipc_sender = self.ipc_sender.clone();
        ipc_sender.blocking_send(StreamerIpcMessage::Stop);

        info!("Terminating Self");
        // TODO
    }
}

struct StreamConnectionListener {
    stream: Weak<StreamConnection>,
}

impl ConnectionListener for StreamConnectionListener {
    fn stage_starting(&mut self, stage: Stage) {
        let Some(stream) = self.stream.upgrade() else {
            warn!("Failed to get stream because it is already deallocated");
            return;
        };

        let mut ipc_sender = stream.ipc_sender.clone();

        stream.runtime.spawn(async move {
            ipc_sender
                .send(StreamerIpcMessage::WebSocket(
                    StreamServerMessage::StageStarting {
                        stage: stage.name().to_string(),
                    },
                ))
                .await;
        });
    }

    fn stage_complete(&mut self, stage: Stage) {
        let Some(stream) = self.stream.upgrade() else {
            warn!("Failed to get stream because it is already deallocated");
            return;
        };

        let mut ipc_sender = stream.ipc_sender.clone();
        ipc_sender.blocking_send(StreamerIpcMessage::WebSocket(
            StreamServerMessage::StageComplete {
                stage: stage.name().to_string(),
            },
        ));
    }

    fn stage_failed(&mut self, stage: Stage, error_code: i32) {
        let Some(stream) = self.stream.upgrade() else {
            warn!("Failed to get stream because it is already deallocated");
            return;
        };

        let mut ipc_sender = stream.ipc_sender.clone();
        ipc_sender.blocking_send(StreamerIpcMessage::WebSocket(
            StreamServerMessage::StageFailed {
                stage: stage.name().to_string(),
                error_code,
            },
        ));
    }

    fn connection_started(&mut self) {}

    fn connection_terminated(&mut self, error_code: i32) {
        let Some(stream) = self.stream.upgrade() else {
            warn!("Failed to get stream because it is already deallocated");
            return;
        };

        let mut ipc_sender = stream.ipc_sender.clone();
        ipc_sender.blocking_send(StreamerIpcMessage::WebSocket(
            StreamServerMessage::ConnectionTerminated { error_code },
        ));

        stream.stop();
    }

    fn log_message(&mut self, message: &str) {
        info!(target: "moonlight", "{}", message.trim());
    }

    fn connection_status_update(&mut self, status: ConnectionStatus) {
        let Some(stream) = self.stream.upgrade() else {
            warn!("Failed to get stream because it is already deallocated");
            return;
        };

        let mut sender = stream.transport_sender.blocking_lock();
        sender
            .send(OutboundPacket::General {
                message: StreamServerMessage::ConnectionStatusUpdate {
                    status: status.into(),
                },
            })
            .unwrap();
    }

    fn set_hdr_mode(&mut self, _hdr_enabled: bool) {}

    fn controller_rumble(
        &mut self,
        controller_number: u16,
        low_frequency_motor: u16,
        high_frequency_motor: u16,
    ) {
        let Some(stream) = self.stream.upgrade() else {
            warn!("Failed to get stream because it is already deallocated");
            return;
        };

        let mut sender = stream.transport_sender.blocking_lock();
        sender
            .send(OutboundPacket::ControllerRumble {
                controller_number: controller_number as u8,
                low_frequency_motor,
                high_frequency_motor,
            })
            .unwrap();
    }

    fn controller_rumble_triggers(
        &mut self,
        controller_number: u16,
        left_trigger_motor: u16,
        right_trigger_motor: u16,
    ) {
        let Some(stream) = self.stream.upgrade() else {
            warn!("Failed to get stream because it is already deallocated");
            return;
        };

        let mut sender = stream.transport_sender.blocking_lock();

        sender
            .send(OutboundPacket::ControllerTriggerRumble {
                controller_number: controller_number as u8,
                left_trigger_motor,
                right_trigger_motor,
            })
            .unwrap();
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

struct StreamVideoDecoder {
    stream: Weak<StreamConnection>,
    supported_formats: SupportedVideoFormats,
}

impl VideoDecoder for StreamVideoDecoder {
    fn setup(&mut self, setup: VideoSetup) -> i32 {
        let Some(stream) = self.stream.upgrade() else {
            warn!("Failed to setup video because stream is deallocated");
            return -1;
        };

        {
            let mut stream_info = stream.stream_info.blocking_lock();
            *stream_info = Some(setup);
        }

        {
            let mut sender = stream.transport_sender.blocking_lock();

            sender.setup_video(setup)
        }
    }

    fn start(&mut self) {}
    fn stop(&mut self) {}

    fn submit_decode_unit(&mut self, unit: VideoDecodeUnit<'_>) -> DecodeResult {
        let Some(stream) = self.stream.upgrade() else {
            warn!("Failed to send video decode unit because stream is deallocated");
            return DecodeResult::Ok;
        };

        let mut sender = stream.transport_sender.blocking_lock();

        match sender.send_video_unit(unit) {
            Err(err) => {
                warn!("Failed to send video decode unit: {err}");
                DecodeResult::Ok
            }
            Ok(value) => value,
        }
    }

    fn supported_formats(&self) -> SupportedVideoFormats {
        self.supported_formats
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::empty()
    }
}

struct StreamAudioDecoder {
    stream: Weak<StreamConnection>,
}

impl AudioDecoder for StreamAudioDecoder {
    fn setup(
        &mut self,
        audio_config: AudioConfig,
        stream_config: OpusMultistreamConfig,
        _ar_flags: i32,
    ) -> i32 {
        let Some(stream) = self.stream.upgrade() else {
            warn!("Failed to setup audio because stream is deallocated");
            return -1;
        };

        let mut sender = stream.transport_sender.blocking_lock();

        sender.setup_audio(audio_config, stream_config)
    }

    fn start(&mut self) {}
    fn stop(&mut self) {}

    fn decode_and_play_sample(&mut self, data: &[u8]) {
        let Some(stream) = self.stream.upgrade() else {
            warn!("Failed to send audio sample because stream is deallocated");
            return;
        };

        let mut stream = stream.transport_sender.blocking_lock();
        if let Err(err) = stream.send_audio_sample(data) {
            warn!("Failed to send audio sample: {err}");
        }
    }

    fn config(&self) -> AudioConfig {
        AudioConfig::STEREO
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::empty()
    }
}
