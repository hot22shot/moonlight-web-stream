use std::sync::Arc;

use log::warn;
use tokio::sync::mpsc::{Receiver, Sender, channel};
use webrtc::{
    media::Sample,
    rtcp::packet::Packet,
    rtp::extension::{HeaderExtension, playout_delay_extension::PlayoutDelayExtension},
    track::track_local::{TrackLocal, track_local_static_sample::TrackLocalStaticSample},
};

use crate::StreamConnection;

pub struct TrackLocalSender<Track>
where
    Track: TrackLike,
{
    channel_queue_size: usize,
    pub(crate) stream: Arc<StreamConnection>,
    sender: Option<Sender<Track::Sample>>,
}

impl<Track> TrackLocalSender<Track>
where
    Track: TrackLike,
{
    pub fn new(stream: Arc<StreamConnection>, channel_queue_size: usize) -> Self {
        Self {
            channel_queue_size,
            stream,
            sender: Default::default(),
        }
    }

    pub fn blocking_create_track(
        &mut self,
        track: Track,
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

    pub fn blocking_send_sample(&self, sample: Track::Sample) {
        if let Some(sender) = self.sender.as_ref() {
            let _ = sender.blocking_send(sample);
        }
    }
}

async fn sample_sender<Track>(track: Arc<Track>, mut receiver: Receiver<Track::Sample>)
where
    Track: TrackLike,
{
    while let Some(sample) = receiver.recv().await {
        if let Err(err) = track
            .write_with_extensions(
                sample,
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

pub trait TrackLike: TrackLocal + Send + Sync + 'static {
    type Sample: Send + 'static;

    fn write_with_extensions(
        &self,
        sample: Self::Sample,
        extensions: &[HeaderExtension],
    ) -> impl Future<Output = Result<(), anyhow::Error>> + Send;
}

impl TrackLike for TrackLocalStaticSample {
    type Sample = Sample;

    fn write_with_extensions(
        &self,
        sample: Self::Sample,
        extensions: &[HeaderExtension],
    ) -> impl Future<Output = Result<(), anyhow::Error>> {
        async move {
            self.write_sample_with_extensions(&sample, extensions)
                .await
                .map_err(anyhow::Error::from)
        }
    }
}
