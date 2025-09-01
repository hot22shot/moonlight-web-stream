use std::{
    ffi::CStr,
    sync::{Arc, LazyLock, Mutex},
};

use moonlight_common_sys::limelight::{LiGetLaunchUrlQueryParameters, LiInterruptConnection};

use crate::{
    MoonlightError,
    stream::{
        audio::AudioDecoder,
        connection::ConnectionListener,
        stream::{MoonlightStream, ServerInfo, StreamConfiguration},
        video::VideoDecoder,
    },
};

// TODO: maybe rename this mod to stream?

pub mod audio;
pub mod connection;
pub mod debug;
pub mod input;
pub mod stream;
pub mod video;

static INSTANCE: LazyLock<Arc<Handle>> = LazyLock::new(|| {
    Arc::new(Handle {
        connection_exists: Mutex::new(false),
    })
});

pub(crate) struct Handle {
    /// This is also the lock because start / stop Connection is not thread safe
    connection_exists: Mutex<bool>,
}

impl Handle {
    fn aquire() -> Option<Arc<Self>> {
        Some(Arc::clone(&INSTANCE))
    }
}

#[derive(Clone)]
pub struct MoonlightInstance {
    handle: Arc<Handle>,
}

impl MoonlightInstance {
    pub fn global() -> Result<Self, MoonlightError> {
        let handle = Handle::aquire().ok_or(MoonlightError::InstanceAquire)?;

        Ok(Self { handle })
    }

    pub fn launch_url_query_parameters(&self) -> &str {
        unsafe {
            // # Safety
            // The returned string is not freed by the caller and should live long enough
            // https://github.com/moonlight-stream/moonlight-common-c/blob/5f2280183cb62cba1052894d76e64e5f4153377d/src/Connection.c#L537
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
    ) -> Result<MoonlightStream, MoonlightError> {
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
}
