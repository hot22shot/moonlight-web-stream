use async_trait::async_trait;
use common::{
    StreamSettings,
    api_bindings::TransportChannelId,
    ipc::{ServerIpcMessage, StreamerIpcMessage},
};
use log::debug;
use moonlight_common::stream::{
    bindings::{AudioConfig, DecodeResult, OpusMultistreamConfig, VideoDecodeUnit},
    video::VideoSetup,
};
use tokio::sync::mpsc::{Receiver, Sender, channel};

use crate::{
    buffer::ByteBuffer,
    transport::{OutboundPacket, TransportError, TransportEvent, TransportEvents, TransportSender},
};

pub async fn new(
    stream_settings: StreamSettings,
) -> Result<(WebSocketTransportSender, WebSocketTransportEvents), anyhow::Error> {
    let (event_sender, event_receiver) = channel::<TransportEvent>(20);

    event_sender
        .send(TransportEvent::StartStream {
            settings: stream_settings,
        })
        .await
        .unwrap();

    Ok((
        WebSocketTransportSender { event_sender },
        WebSocketTransportEvents { event_receiver },
    ))
}

pub struct WebSocketTransportEvents {
    event_receiver: Receiver<TransportEvent>,
}

#[async_trait]
impl TransportEvents for WebSocketTransportEvents {
    async fn poll_event(&mut self) -> Result<TransportEvent, TransportError> {
        debug!("Polling WebSocketEvents");
        self.event_receiver
            .recv()
            .await
            .ok_or(TransportError::Closed)
    }
}

pub struct WebSocketTransportSender {
    event_sender: Sender<TransportEvent>,
}

#[async_trait]
impl TransportSender for WebSocketTransportSender {
    async fn setup_video(&self, setup: VideoSetup) -> i32 {
        // empty
        0
    }
    async fn send_video_unit<'a>(
        &'a self,
        unit: &'a VideoDecodeUnit<'a>,
    ) -> Result<DecodeResult, TransportError> {
        let mut new_buffer = vec![0];

        let mut byte_buffer = ByteBuffer::new(new_buffer.as_mut_slice());
        byte_buffer.put_u8(TransportChannelId::HOST_VIDEO);

        for buffer in unit.buffers {
            new_buffer.extend_from_slice(buffer.data);
        }

        self.event_sender
            .send(TransportEvent::SendIpc(
                StreamerIpcMessage::WebSocketTransport(new_buffer),
            ))
            .await
            .unwrap();

        Ok(DecodeResult::Ok)
    }

    async fn setup_audio(
        &self,
        audio_config: AudioConfig,
        stream_config: OpusMultistreamConfig,
    ) -> i32 {
        // empty
        0
    }
    async fn send_audio_sample(&self, data: &[u8]) -> Result<(), TransportError> {
        let mut new_buffer = vec![0];

        let mut byte_buffer = ByteBuffer::new(new_buffer.as_mut_slice());
        byte_buffer.put_u8(TransportChannelId::HOST_AUDIO);

        new_buffer.extend_from_slice(data);

        self.event_sender
            .send(TransportEvent::SendIpc(
                StreamerIpcMessage::WebSocketTransport(new_buffer),
            ))
            .await
            .unwrap();

        Ok(())
    }

    async fn send(&self, packet: OutboundPacket) -> Result<(), TransportError> {
        let mut new_buffer = Vec::new();

        let (id, range) = packet.serialize(&mut new_buffer).unwrap();

        new_buffer.drain(..range.start + 1);
        new_buffer.resize(range.end - range.start + 1, 0);

        new_buffer[0] = id.0;

        self.event_sender
            .send(TransportEvent::SendIpc(
                StreamerIpcMessage::WebSocketTransport(new_buffer),
            ))
            .await
            .unwrap();

        Ok(())
    }

    async fn on_ipc_message(&self, message: ServerIpcMessage) -> Result<(), TransportError> {
        // TODO
        Ok(())
    }

    async fn close(&self) -> Result<(), TransportError> {
        // emtpy
        Ok(())
    }
}
