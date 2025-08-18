use std::{io::Cursor, sync::Arc, time::Duration};

use actix_web::web::Bytes;
use log::warn;
use moonlight_common::moonlight::{
    audio::{AudioConfig, AudioDecoder, OpusMultistreamConfig},
    stream::Capabilities,
};
use tokio::runtime::Handle;
use webrtc::{
    media::{Sample, io::ogg_reader::OggReader},
    track::track_local::track_local_static_sample::TrackLocalStaticSample,
};

use crate::api::stream::StreamStages;

pub struct OpusTrackSampleAudioDecoder {
    runtime: Handle,
    audio_track: Arc<TrackLocalStaticSample>,
    stages: Arc<StreamStages>,
    config: Option<OpusMultistreamConfig>,
}

impl OpusTrackSampleAudioDecoder {
    pub fn new(audio_track: Arc<TrackLocalStaticSample>, stages: Arc<StreamStages>) -> Self {
        Self {
            runtime: Handle::current(),
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

        let duration = Duration::from_millis((1000.0 / config.sample_rate as f32) as u64);

        let data = Bytes::copy_from_slice(data);
        let audio_track = self.audio_track.clone();

        self.runtime.spawn(async move {
            let sample = Sample {
                data,
                duration,
                // Time should be set if you want fine-grained sync
                ..Default::default()
            };

            if let Err(err) = audio_track.write_sample(&sample).await {
                warn!("[Stream]: audio_track.write_sample failed: {err}");
            }
            println!("sample written: {duration:?}");
        });
    }

    fn config(&self) -> AudioConfig {
        AudioConfig::STEREO
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::empty()
    }
}
