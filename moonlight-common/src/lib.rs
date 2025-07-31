// Sadly moonlight log message requires variadic args
#![feature(c_variadic)]

use std::{
    ffi::{CStr, NulError},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

use moonlight_common_sys::limelight::{LiGetLaunchUrlQueryParameters, LiInterruptConnection};
use thiserror::Error;

use crate::{
    audio::{AudioConfig, AudioDecoder, OpusMultistreamConfig},
    connection::{ConnectionListener, ConnectionStatus, Stage},
    stream::{Capabilities, MoonlightStream, ServerInfo, StreamConfiguration},
    video::{DecodeResult, SupportedVideoFormats, VideoDecoder, VideoFormat},
};

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum Error {
    #[error("the host doesn't support this feature")]
    NotSupportedOnHost,
    #[error("an error happened whilst sending an event")]
    EventSendError(i32),
    #[error("this call requires a GFE version which uses ENet")]
    ENetRequired,
    #[error("a string contained a nul byte which is not allowed in c strings")]
    StringNulError(#[from] NulError),
    #[error("a moonlight instance already exists")]
    ConnectionAlreadyExists,
    #[error("couldn't establish a connection")]
    ConnectionFailed,
    #[error("a moonlight instance already exists")]
    InstanceAlreadyExists,
    #[error("the client is not paired")]
    NotPaired,
}

pub mod audio;
pub mod connection;
pub mod input;
pub mod pair;
pub mod stream;
pub mod video;

#[cfg(feature = "crypto")]
pub mod crypto;
#[cfg(feature = "network")]
pub mod network;

#[cfg(feature = "high")]
pub mod high;

static INSTANCE_EXISTS: AtomicBool = AtomicBool::new(false);

struct Handle {
    /// This is also the lock because start / stop Connection is not thread safe
    connection_exists: Mutex<bool>,
}

impl Handle {
    fn aquire() -> Option<Self> {
        if INSTANCE_EXISTS
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
        {
            Some(Self {
                connection_exists: Mutex::new(false),
            })
        } else {
            None
        }
    }
}
impl Drop for Handle {
    fn drop(&mut self) {
        INSTANCE_EXISTS.store(false, Ordering::Relaxed);
    }
}

#[derive(Clone)]
pub struct MoonlightInstance {
    handle: Arc<Handle>,
}

impl MoonlightInstance {
    pub fn global() -> Result<Self, Error> {
        let handle = Handle::aquire().ok_or(Error::InstanceAlreadyExists)?;

        Ok(Self {
            handle: Arc::new(handle),
        })
    }

    pub fn launch_url_query_parameters(&self) -> &str {
        unsafe {
            // # Safety
            // The returned string is not freed by the caller and should live long enough
            let str_raw = LiGetLaunchUrlQueryParameters();
            let str = CStr::from_ptr(str_raw);
            str.to_str().expect("valid moonlight query parameters")
        }
    }

    pub fn start_connection(
        &self,
        server_info: ServerInfo,
        stream_config: StreamConfiguration,
        connection_listener: impl ConnectionListener + Send + 'static,
        video_decoder: impl VideoDecoder + Send + 'static,
        audio_decoder: impl AudioDecoder + Send + 'static,
    ) -> Result<MoonlightStream, Error> {
        MoonlightStream::start(
            self.handle.clone(),
            server_info,
            stream_config,
            connection_listener,
            video_decoder,
            audio_decoder,
        )
    }

    pub fn interrupt_connection(&self) {
        unsafe {
            LiInterruptConnection();
        }
    }

    #[cfg(feature = "crypto")]
    pub fn crypto(&self) -> crypto::MoonlightCrypto {
        crypto::MoonlightCrypto::new(self)
    }
}

pub struct NullHandler;

impl VideoDecoder for NullHandler {
    fn setup(
        &mut self,
        format: VideoFormat,
        width: u32,
        height: u32,
        redraw_rate: u32,
        flags: (),
    ) -> i32 {
        let _ = (format, width, height, redraw_rate, flags);

        0
    }

    fn start(&mut self) {}

    fn submit_decode_unit(&mut self, unit: video::VideoDecodeUnit<'_>) -> DecodeResult {
        let _ = unit;

        DecodeResult::Ok
    }

    fn stop(&mut self) {}

    fn supported_formats(&self) -> SupportedVideoFormats {
        SupportedVideoFormats::all()
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::empty()
    }
}

impl AudioDecoder for NullHandler {
    fn setup(
        &mut self,
        audio_config: AudioConfig,
        stream_config: OpusMultistreamConfig,
        ar_flags: (),
    ) -> i32 {
        let _ = (audio_config, stream_config, ar_flags);

        0
    }

    fn start(&mut self) {}
    fn decode_and_play_sample(&mut self, data: &[u8]) {
        let _ = data;
    }

    fn stop(&mut self) {}

    fn config(&self) -> AudioConfig {
        AudioConfig::STEREO
    }
    fn capabilities(&self) -> Capabilities {
        Capabilities::empty()
    }
}

impl ConnectionListener for NullHandler {
    fn stage_starting(&mut self, stage: Stage) {
        let _ = stage;
    }
    fn stage_complete(&mut self, stage: Stage) {
        let _ = stage;
    }
    fn stage_failed(&mut self, stage: Stage, error_code: i32) {
        let _ = (stage, error_code);
    }

    fn connection_started(&mut self) {}
    fn connection_status_update(&mut self, status: ConnectionStatus) {
        let _ = status;
    }
    fn connection_terminated(&mut self, error_code: i32) {
        let _ = error_code;
    }

    fn log_message(&mut self, message: &str) {
        println!("[Moonlight] {message}");
    }

    fn set_hdr_mode(&mut self, hdr_enabled: bool) {
        let _ = hdr_enabled;
    }

    fn controller_rumble(
        &mut self,
        controller_number: u16,
        low_frequency_motor: u16,
        high_frequency_motor: u16,
    ) {
        let _ = (controller_number, low_frequency_motor, high_frequency_motor);
    }
    fn controller_rumble_triggers(
        &mut self,
        controller_number: u16,
        left_trigger_motor: u16,
        right_trigger_motor: u16,
    ) {
        let _ = (controller_number, left_trigger_motor, right_trigger_motor);
    }
    fn controller_set_adaptive_triggers(
        &mut self,
        controller_number: u16,
        event_flags: u8,
        type_left: u8,
        type_right: u8,
        left: &mut u8,
        right: &mut u8,
    ) {
        let _ = (
            controller_number,
            event_flags,
            type_left,
            type_right,
            left,
            right,
        );
    }
    fn controller_set_led(&mut self, controller_number: u16, r: u8, g: u8, b: u8) {
        let _ = (controller_number, r, g, b);
    }
    fn controller_set_motion_event_state(
        &mut self,
        controller_number: u16,
        motion_type: u8,
        report_rate_hz: u16,
    ) {
        let _ = (controller_number, motion_type, report_rate_hz);
    }
}
