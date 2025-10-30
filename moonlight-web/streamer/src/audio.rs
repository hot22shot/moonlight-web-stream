use std::{sync::Arc, time::Duration};

use bytes::Bytes;
use log::{error, info, warn};
use moonlight_common::stream::{
    audio::AudioDecoder,
    bindings::{AudioConfig, OpusMultistreamConfig},
};
use webrtc::{
    api::media_engine::{MIME_TYPE_OPUS, MediaEngine},
    media::Sample,
    rtp_transceiver::rtp_codec::{RTCRtpCodecCapability, RTCRtpCodecParameters, RTPCodecType},
    track::track_local::track_local_static_sample::TrackLocalStaticSample,
};

use crate::{StreamConnection, sender::TrackLocalSender};

pub fn register_audio_codecs(media_engine: &mut MediaEngine) -> Result<(), webrtc::Error> {
    media_engine.register_codec(
        RTCRtpCodecParameters {
            capability: RTCRtpCodecCapability {
                mime_type: MIME_TYPE_OPUS.to_owned(),
                clock_rate: 48000,
                channels: 2,
                sdp_fmtp_line: "minptime=10;useinbandfec=1".to_owned(),
                rtcp_feedback: vec![],
            },
            payload_type: 111,
            ..Default::default()
        },
        RTPCodecType::Audio,
    )?;

    Ok(())
}

pub struct OpusTrackSampleAudioDecoder {
    decoder: TrackLocalSender<TrackLocalStaticSample>,
    config: Option<OpusMultistreamConfig>,
}

impl OpusTrackSampleAudioDecoder {
    pub fn new(stream: Arc<StreamConnection>, channel_queue_size: usize) -> Self {
        Self {
            decoder: TrackLocalSender::new(stream, channel_queue_size),
            config: None,
        }
    }
}

impl AudioDecoder for OpusTrackSampleAudioDecoder {
    fn setup(
        &mut self,
        audio_config: AudioConfig,
        stream_config: OpusMultistreamConfig,
        _ar_flags: i32,
    ) -> i32 {
        info!("[Stream] Audio setup: {audio_config:?}, {stream_config:?}");

        const SUPPORTED_SAMPLE_RATES: &[u32] = &[80000, 12000, 16000, 24000, 48000];
        if !SUPPORTED_SAMPLE_RATES.contains(&stream_config.sample_rate) {
            warn!(
                "[Stream] Audio could have problems because of the sample rate, Selected: {}, Expected one of: {SUPPORTED_SAMPLE_RATES:?}",
                stream_config.sample_rate
            );
        }
        if audio_config != self.config() {
            warn!(
                "[Stream] A different audio configuration than requested was selected, Expected: {:?}, Found: {audio_config:?}",
                self.config()
            );
        }

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
}
