use std::{io::Cursor, sync::Arc, time::Duration};

use actix_web::web::Bytes;
use log::warn;
use moonlight_common::{
    audio::{AudioConfig, AudioDecoder, OpusMultistreamConfig},
    stream::Capabilities,
};
use tokio::runtime::Handle;
use webrtc::{
    media::{
        Sample,
        io::{ogg_reader::OggReader, ogg_writer::OggWriter},
    },
    track::track_local::track_local_static_sample::TrackLocalStaticSample,
};

use crate::api::stream::StreamState;

pub struct OpusTrackSampleAudioDecoder {
    runtime: Handle,
    audio_track: Arc<TrackLocalStaticSample>,
    state: Arc<StreamState>,
    reader: Option<OggReader<Cursor<Bytes>>>,
    last_granule: u64,
}

impl OpusTrackSampleAudioDecoder {
    pub fn new(audio_track: Arc<TrackLocalStaticSample>, state: Arc<StreamState>) -> Self {
        Self {
            runtime: Handle::current(),
            audio_track,
            state,
            reader: None,
            last_granule: 0,
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
        let mut config_data = Cursor::new(Vec::new());
        let _ = OggWriter::new(
            &mut config_data,
            stream_config.sample_rate,
            stream_config.channel_count as u8,
        )
        .unwrap();

        let config_data = Cursor::new(Bytes::from(config_data.into_inner()));
        let (reader, _) = OggReader::new(config_data, true).unwrap();
        self.reader = Some(reader);

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

        let Some(reader) = self.reader.as_mut() else {
            return;
        };

        let data = Bytes::copy_from_slice(data);
        reader.reset_reader(Box::new(move |_| Cursor::new(data.clone())));

        while let Ok((page_data, page_header)) = reader.parse_next_page() {
            // The amount of samples is the difference between the last and current timestamp
            let sample_count = page_header.granule_position - self.last_granule;
            self.last_granule = page_header.granule_position;
            let sample_duration = Duration::from_millis(sample_count * 1000 / 48000);

            let audio_track = self.audio_track.clone();
            // self.runtime.spawn(async move {
            //     if let Err(err) = audio_track
            //         .write_sample(&Sample {
            //             data: page_data.into(),
            //             duration: sample_duration,
            //             ..Default::default()
            //         })
            //         .await
            //     {
            //         warn!("[Stream]: audio_track.write_sample failed: {err}");
            //     }
            // });
        }
    }

    fn config(&self) -> AudioConfig {
        AudioConfig::STEREO
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::empty()
    }
}
