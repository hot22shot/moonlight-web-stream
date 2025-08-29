use std::{sync::Arc, time::Duration};

use bytes::Bytes;
use log::error;
use moonlight_common::moonlight::{
    audio::{AudioConfig, AudioDecoder, OpusMultistreamConfig},
    stream::Capabilities,
};
use webrtc::{
    api::media_engine::MIME_TYPE_OPUS, media::Sample,
    rtp_transceiver::rtp_codec::RTCRtpCodecCapability,
    track::track_local::track_local_static_sample::TrackLocalStaticSample,
};

use crate::{StreamConnection, decoder::TrackSampleDecoder};

pub struct OpusTrackSampleAudioDecoder {
    decoder: TrackSampleDecoder,
    config: Option<OpusMultistreamConfig>,
}

impl OpusTrackSampleAudioDecoder {
    pub fn new(stream: Arc<StreamConnection>, channel_queue_size: usize) -> Self {
        Self {
            decoder: TrackSampleDecoder::new(stream, channel_queue_size),
            config: None,
        }
    }
}

impl AudioDecoder for OpusTrackSampleAudioDecoder {
    fn setup(
        &mut self,
        _audio_config: AudioConfig,
        stream_config: OpusMultistreamConfig,
        _ar_flags: i32,
    ) -> i32 {
        if let Err(err) = self.decoder.blocking_create_track(
            TrackLocalStaticSample::new(
                RTCRtpCodecCapability {
                    mime_type: MIME_TYPE_OPUS.to_string(),
                    ..Default::default()
                },
                "audio".to_string(),
                "moonlight".to_string(),
            ),
            |_| {},
        ) {
            error!("Failed to create opus track: {err:?}");
            return -1;
        };

        self.config = Some(stream_config);

        0
    }

    fn start(&mut self) {}

    fn stop(&mut self) {}

    fn decode_and_play_sample(&mut self, data: &[u8]) {
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

        self.decoder.blocking_send_sample(sample);
    }

    fn config(&self) -> AudioConfig {
        AudioConfig::STEREO
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::empty()
    }
}
