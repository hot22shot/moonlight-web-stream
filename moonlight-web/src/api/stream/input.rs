use std::{pin::Pin, sync::Arc};

use log::info;
use moonlight_common::stream::MoonlightStream;
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
        let _ = (stream, message);

        todo!()
    }

    fn on_keyboard_message(stream: &MoonlightStream, message: DataChannelMessage) {
        info!("[Stream Input]: received keyboard message");
        let mut buffer = ByteBuffer::new(message.data);

        let ty = buffer.get_u8();
        if ty == 0 {
            info!("updown");
            todo!()
        } else {
            let key = buffer.get_utf8(1).unwrap();
            info!("text: \"{key}\"");

            stream.send_text(key).unwrap();
        }
    }
}
