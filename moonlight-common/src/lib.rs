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
    audio::AudioDecoder,
    connection::ConnectionListener,
    stream::{MoonlightStream, ServerInfo, StreamConfiguration},
    video::VideoDecoder,
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
pub mod debug;
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
    // TODO: don't error but use global handle
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
