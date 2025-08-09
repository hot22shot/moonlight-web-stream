use std::{pin::Pin, sync::Arc};

use log::{info, warn};
use moonlight_common::{
    input::TouchEventType,
    stream::{KeyAction, KeyFlags, KeyModifiers, MoonlightStream, MouseButton, MouseButtonAction},
};
use num_traits::FromPrimitive;
use webrtc::data_channel::{RTCDataChannel, data_channel_message::DataChannelMessage};

use crate::api::stream::{StreamConnection, buffer::ByteBuffer};

pub struct StreamInput {}

impl StreamInput {
    pub fn new() -> Self {
        Self {}
    }

    /// Returns if this added events
    pub fn on_data_channel(
        &self,
        connection: &Arc<StreamConnection>,
        data_channel: Arc<RTCDataChannel>,
    ) -> bool {
        info!(
            "[Stream Input]: adding data channel: \"{}\"",
            data_channel.label()
        );

        match data_channel.label() {
            "mouse" => {
                data_channel.on_message(Self::create_handler(
                    connection.clone(),
                    Self::on_mouse_message,
                ));
            }
            "touch" => {
                data_channel.on_message(Self::create_handler(
                    connection.clone(),
                    Self::on_touch_message,
                ));

                // TODO: send supported on open
            }
            "keyboard" => {
                data_channel.on_message(Self::create_handler(
                    connection.clone(),
                    Self::on_keyboard_message,
                ));
            }
            _ => return false,
        };

        true
    }

    #[allow(clippy::type_complexity)]
    fn create_handler(
        connection: Arc<StreamConnection>,
        f: impl Fn(&MoonlightStream, DataChannelMessage) + Send + Sync + 'static,
    ) -> Box<
        dyn FnMut(DataChannelMessage) -> Pin<Box<dyn Future<Output = ()> + Send + 'static>>
            + Send
            + Sync,
    > {
        info!("HANDLER");
        let f = Arc::new(f);

        Box::new(move |message| {
            let connection = connection.clone();

            let f = f.clone();
            Box::pin(async move {
                let stream = connection.stream.read().await;
                if let Some(stream) = stream.as_ref() {
                    f(stream, message);
                }
            })
        })
    }

    fn on_touch_message(stream: &MoonlightStream, message: DataChannelMessage) {
        let mut buffer = ByteBuffer::new(message.data);

        let event_type = match buffer.get_u8() {
            0 => TouchEventType::Down,
            1 => TouchEventType::Move,
            2 => TouchEventType::Cancel,
            _ => {
                warn!("[Stream Input]: received invalid touch event type");
                return;
            }
        };
        let pointer_id = buffer.get_u32();
        let x = buffer.get_f32();
        let y = buffer.get_f32();
        let pressure_or_distance = buffer.get_f32();
        let contact_area_major = buffer.get_f32();
        let contact_area_minor = buffer.get_f32();
        let rotation = buffer.get_u16();

        let _ = stream.send_touch(
            pointer_id,
            x,
            y,
            pressure_or_distance,
            contact_area_major,
            contact_area_minor,
            Some(rotation),
            event_type,
        );
    }

    fn on_mouse_message(stream: &MoonlightStream, message: DataChannelMessage) {
        let mut buffer = ByteBuffer::new(message.data);

        let ty = buffer.get_u8();
        if ty == 0 {
            // Move
            let delta_x = buffer.get_i16();
            let delta_y = buffer.get_i16();

            let _ = stream.send_mouse_move(delta_x, delta_y);
        } else if ty == 1 {
            // Button Press / Release
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
