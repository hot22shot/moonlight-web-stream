use std::sync::Arc;

use log::warn;
use tokio::sync::mpsc::{Receiver, Sender, channel};
use webrtc::{
    media::Sample,
    rtcp::packet::Packet,
    rtp::extension::{HeaderExtension, playout_delay_extension::PlayoutDelayExtension},
    track::track_local::track_local_static_sample::TrackLocalStaticSample,
};

use crate::StreamConnection;

pub struct TrackSampleDecoder {
    channel_queue_size: usize,
    pub(crate) stream: Arc<StreamConnection>,
    sender: Option<Sender<Sample>>,
}

impl TrackSampleDecoder {
    pub fn new(stream: Arc<StreamConnection>, channel_queue_size: usize) -> Self {
        Self {
            channel_queue_size,
            stream,
            sender: Default::default(),
        }
    }

    pub fn blocking_create_track(
        &mut self,
        track: TrackLocalStaticSample,
        mut on_packet: impl FnMut(Box<dyn Packet + Send + Sync>) + Send + 'static,
    ) -> Result<(), anyhow::Error> {
        let stream = self.stream.clone();

        let track = Arc::new(track);

        let (sender, receiver) = channel(self.channel_queue_size);

        self.stream.runtime.spawn({
            let track = track.clone();
            async move {
                sample_sender(track, receiver).await;
            }
        });

        let track_sender = self.stream.runtime.block_on({
            let track = track.clone();
            async move { stream.peer.add_track(track).await }
        })?;

        // Read incoming RTCP packets
        // Before these packets are returned they are processed by interceptors. For things
        // like NACK this needs to be called.
        self.stream.runtime.spawn(async move {
            let mut rtcp_buf = vec![0u8; 1500];
            while let Ok((packets, _)) = track_sender.read(&mut rtcp_buf).await {
                for packet in packets {
                    on_packet(packet);
                }
            }
        });

        self.sender.replace(sender);

        Ok(())
    }

    pub fn blocking_send_sample(&self, sample: Sample) {
        if let Some(sender) = self.sender.as_ref() {
            let _ = sender.blocking_send(sample);
        }
    }
}

async fn sample_sender(track: Arc<TrackLocalStaticSample>, mut receiver: Receiver<Sample>) {
    while let Some(sample) = receiver.recv().await {
        if let Err(err) = track
            .write_sample_with_extensions(
                &sample,
                &[HeaderExtension::PlayoutDelay(PlayoutDelayExtension::new(
                    0, 0,
                ))],
            )
            .await
        {
            warn!("[Stream]: track.write_sample failed: {err}");
        }
    }
}
