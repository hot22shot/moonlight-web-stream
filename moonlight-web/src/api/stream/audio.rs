use std::{sync::Arc, time::Duration};

use actix_web::web::Bytes;
use log::warn;
use moonlight_common::moonlight::{
    audio::{AudioConfig, AudioDecoder, OpusMultistreamConfig},
    stream::Capabilities,
};
use tokio::{
    spawn,
    sync::mpsc::{Receiver, Sender, channel},
};
use webrtc::{
    media::Sample,
    rtp::extension::{HeaderExtension, playout_delay_extension::PlayoutDelayExtension},
    track::track_local::track_local_static_sample::TrackLocalStaticSample,
};

use crate::api::stream::StreamStages;

pub struct OpusTrackSampleAudioDecoder {
    audio_track: Arc<TrackLocalStaticSample>,
    sender: Sender<Sample>,
    stages: Arc<StreamStages>,
    config: Option<OpusMultistreamConfig>,
}

impl OpusTrackSampleAudioDecoder {
    pub fn new(
        audio_track: Arc<TrackLocalStaticSample>,
        stages: Arc<StreamStages>,
        sample_send_queue_size: usize,
    ) -> Self {
        let (sender, receiver) = channel(sample_send_queue_size);

        spawn({
            let audio_track = audio_track.clone();
            async move {
                sample_sender(audio_track, receiver).await;
            }
        });

        Self {
            sender,
            audio_track,
            stages,
            config: None,
        }
    }
}

impl AudioDecoder for OpusTrackSampleAudioDecoder {
    fn setup(
        &mut self,
        audio_config: AudioConfig,
        stream_config: OpusMultistreamConfig,
        ar_flags: (),
    ) -> i32 {
        self.config = Some(stream_config);
        0
    }

    fn start(&mut self) {}

    fn stop(&mut self) {
        self.stages.stop.set_reached();
    }

    fn decode_and_play_sample(&mut self, data: &[u8]) {
        if self.stages.stop.is_reached() || !self.stages.connected.is_reached() {
            return;
        }

        let Some(config) = self.config.as_ref() else {
            return;
        };

        let duration =
            Duration::from_secs_f64(config.samples_per_frame as f64 / config.sample_rate as f64);

        let data = Bytes::copy_from_slice(data);

        let sample = Sample {
            data,
            duration,
            // Time should be set if you want fine-grained sync
            ..Default::default()
        };
        let _ = self.sender.try_send(sample);
    }

    fn config(&self) -> AudioConfig {
        AudioConfig::STEREO
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::empty()
    }
}

// TODO: this should be common between audio and video
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
            warn!("[Stream]: video_track.write_sample failed: {err}");
        }
    }
}
