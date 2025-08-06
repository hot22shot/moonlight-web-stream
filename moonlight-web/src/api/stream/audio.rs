use std::{sync::Arc, time::Duration};

use actix_web::web::Bytes;
use log::warn;
use moonlight_common::{
    audio::{AudioConfig, AudioDecoder, OpusMultistreamConfig},
    stream::Capabilities,
};
use tokio::runtime::Handle;
use webrtc::{
    media::Sample, track::track_local::track_local_static_sample::TrackLocalStaticSample,
};

use crate::api::stream::StreamState;

pub struct OpusTrackSampleAudioDecoder {
    runtime: Handle,
    audio_track: Arc<TrackLocalStaticSample>,
    state: Arc<StreamState>,
    config: Option<OpusMultistreamConfig>,
}

impl OpusTrackSampleAudioDecoder {
    pub fn new(audio_track: Arc<TrackLocalStaticSample>, state: Arc<StreamState>) -> Self {
        Self {
            runtime: Handle::current(),
            audio_track,
            state,
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
        self.state.stop.set_reached();
    }

    fn decode_and_play_sample(&mut self, data: &[u8]) {
        if self.state.stop.is_reached() {
            return;
        }

        if !self.state.connected.is_reached() {
            return;
        }

        let Some(config) = self.config.as_ref() else {
            return;
        };

        let duration = Duration::from_millis(config.sample_rate as u64);

        let data = Bytes::copy_from_slice(data);
        let audio_track = self.audio_track.clone();

        self.runtime.spawn(async move {
            if let Err(err) = audio_track
                .write_sample(&Sample {
                    data,
                    duration,
                    ..Default::default()
                })
                .await
            {
                warn!("[Stream]: audio_track.write_sample failed: {err}");
            }
        });
    }

    fn config(&self) -> AudioConfig {
        AudioConfig::STEREO
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::empty()
    }
}
