use std::{pin::Pin, sync::Arc};

use log::{info, warn};
use moonlight_common::stream::{
    KeyAction, KeyFlags, KeyModifiers, MoonlightStream, MouseButton, MouseButtonAction,
};
use num_traits::FromPrimitive;
use tokio::sync::RwLock;
use webrtc::data_channel::{RTCDataChannel, data_channel_message::DataChannelMessage};

use crate::api::stream::buffer::ByteBuffer;

pub struct StreamInput {
    inner: Arc<Inner>,
}

#[derive(Default)]
struct Inner {
    moonlight: RwLock<Option<Arc<MoonlightStream>>>,
}

impl StreamInput {
    pub fn new() -> Self {
        Self {
            inner: Default::default(),
        }
    }

    pub async fn set_stream(&self, stream: Arc<MoonlightStream>) {
        let mut guard = self.inner.moonlight.write().await;

        guard.replace(stream);
    }

    /// Returns if this added events
    pub fn on_data_channel(&self, data_channel: Arc<RTCDataChannel>) -> bool {
        info!(
            "[Stream Input]: adding data channel: \"{}\"",
            data_channel.label()
        );

        match data_channel.label() {
            "mouse" => data_channel.on_message(Self::create_handler(
                self.inner.clone(),
                Self::on_mouse_message,
            )),
            "keyboard" => data_channel.on_message(Self::create_handler(
                self.inner.clone(),
                Self::on_keyboard_message,
            )),
            _ => return false,
        };

        true
    }

    #[allow(clippy::type_complexity)]
    fn create_handler(
        inner: Arc<Inner>,
        f: impl Fn(&MoonlightStream, DataChannelMessage) + Send + Sync + 'static,
    ) -> Box<
        dyn FnMut(DataChannelMessage) -> Pin<Box<dyn Future<Output = ()> + Send + 'static>>
            + Send
            + Sync,
    > {
        info!("HANDLER");
        let f = Arc::new(f);

        Box::new(move |message| {
            let inner = inner.clone();

            let f = f.clone();
            Box::pin(async move {
                let stream = inner.moonlight.read().await;
                if let Some(stream) = stream.as_ref() {
                    f(stream, message);
                }
            })
        })
    }

    fn on_mouse_message(stream: &MoonlightStream, message: DataChannelMessage) {
        let mut buffer = ByteBuffer::new(message.data);

        let ty = buffer.get_u8();
        if ty == 0 {
            todo!()
        } else if ty == 1 {
            let action = if buffer.get_bool() {
                MouseButtonAction::Press
            } else {
                MouseButtonAction::Release
            };
            let Some(button) = MouseButton::from_u8(buffer.get_u8()) else {
                warn!("[Stream Input]: recieved invalid mouse button");
                return;
            };

            let _ = stream.send_mouse_button(action, button);
        }
    }

    fn on_keyboard_message(stream: &MoonlightStream, message: DataChannelMessage) {
        let mut buffer = ByteBuffer::new(message.data);

        let ty = buffer.get_u8();
        if ty == 0 {
            let action = if buffer.get_bool() {
                KeyAction::Down
            } else {
                KeyAction::Up
            };
            let modifiers = KeyModifiers::from_bits(buffer.get_u8() as i8).unwrap_or_else(|| {
                warn!("[Stream Input]: received invalid key modifiers");
                KeyModifiers::empty()
            });
            let key = buffer.get_u16();

            let _ = stream.send_keyboard_event_non_standard(
                key as i16,
                action,
                modifiers,
                KeyFlags::empty(),
            );
        } else if ty == 1 {
            let Ok(key) = buffer.get_utf8(1) else {
                warn!("[Stream Input]: received invalid keyboard text message");
                return;
            };

            let _ = stream.send_text(key);
        }
    }
}
