use std::{
    ffi::NulError,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

use moonlight_common_sys::LiInterruptConnection;
use thiserror::Error;

use crate::{
    connection::MoonlightConnection,
    data::{ServerInfo, StreamConfiguration},
};

#[derive(Debug, Error)]
pub enum Error {
    #[error("the host doesn't support this feature")]
    NotSupportedOnHost,
    #[error("an error happened whilst sending an event")]
    EventSendError,
    #[error("this call requires a GFE version which uses ENet")]
    ENetRequired,
    #[error("a string contained a nul byte which is not allowed in c strings")]
    StringNulError(#[from] NulError),
    #[error("a moonlight instance already exists")]
    InstanceAlreadyExists,
}

pub mod connection;
pub mod data;

static INSTANCE_EXISTS: AtomicBool = AtomicBool::new(false);

struct Handle {
    connection: Mutex<()>,
}

impl Handle {
    fn aquire() -> Option<Self> {
        // TODO: ordering?
        if INSTANCE_EXISTS
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
        {
            Some(Self {
                connection: Mutex::new(()),
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

    pub fn start_connection(
        &self,
        server_info: ServerInfo,
        stream_config: StreamConfiguration,
    ) -> Result<MoonlightConnection, Error> {
        MoonlightConnection::start(self.handle.clone(), server_info, stream_config)
    }

    pub fn interrupt_connection(&self) {
        unsafe {
            LiInterruptConnection();
        }
    }
}
