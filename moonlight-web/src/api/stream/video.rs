use std::{
    io::Cursor,
    sync::Arc,
    time::{Duration, SystemTime},
};

use actix_web::web::Bytes;
use log::{error, info};
use moonlight_common::moonlight::{
    stream::Capabilities,
    video::{
        BufferType, DecodeResult, FrameType, SupportedVideoFormats, VideoDecodeUnit, VideoDecoder,
        VideoFormat,
    },
};
use webrtc::{
    api::media_engine::{MIME_TYPE_H264, MIME_TYPE_HEVC},
    media::{Sample, io::h264_reader::H264Reader},
    rtp_transceiver::rtp_codec::RTCRtpCodecCapability,
    track::track_local::track_local_static_sample::TrackLocalStaticSample,
};

use crate::api::stream::{StreamConnection, decoder::TrackSampleDecoder};

pub struct H264TrackSampleVideoDecoder {
    decoder: TrackSampleDecoder,
    clock_rate: u32,
    // Video important
    needs_idr: bool,
    frame_time: f32,
    last_frame_number: i32,
}

impl H264TrackSampleVideoDecoder {
    // TODO: maybe allow the Moonlight crate to decide the video format?
    pub fn new(stream: Arc<StreamConnection>, channel_queue_size: usize) -> Self {
        Self {
            decoder: TrackSampleDecoder::new(stream, channel_queue_size),
            clock_rate: 90000,
            needs_idr: false,
            frame_time: 0.0,
            last_frame_number: 0,
        }
    }
}

impl VideoDecoder for H264TrackSampleVideoDecoder {
    fn setup(
        &mut self,
        format: VideoFormat,
        width: u32,
        height: u32,
        redraw_rate: u32,
        _flags: (),
    ) -> i32 {
        info!("[Stream] Streaming with format: {format:?}");

        if !format.contained_in(self.supported_formats()) {
            error!(
                "tried to setup a video stream with a non supported video format: {format:?}, supported formats: {:?}",
                self.supported_formats()
            );
            return -1;
        }

        // TODO: send width / height?

        let Some(mime_type) = video_format_to_mime_type(format) else {
            error!("couldn't get mime type for video format: {format:?}");
            return -1;
        };

        // TODO: is it possible to make the video channel unreliable?
        if let Err(err) = self.decoder.blocking_create_track(
            TrackLocalStaticSample::new(
                RTCRtpCodecCapability {
                    mime_type: mime_type.clone(),
                    clock_rate: self.clock_rate,
                    ..Default::default()
                },
                "video".to_string(),
                "moonlight".to_string(),
            ),
            |_| {
                // TODO: idr frames
            },
        ) {
            error!(
                "Failed to create video track with format {format:?} and mime \"{mime_type}\": {err:?}"
            );
            return -1;
        }

        self.frame_time = 1.0 / redraw_rate as f32;

        0
    }
    fn start(&mut self) {}
    fn stop(&mut self) {}

    fn submit_decode_unit(&mut self, unit: VideoDecodeUnit<'_>) -> DecodeResult {
        let mut full_frame = Vec::new();

        let frame_time = self.frame_time;
        let timestamp =
            SystemTime::UNIX_EPOCH + Duration::from_millis(unit.presentation_time_ms as u64);
        let packet_timestamp =
            (unit.frame_number as f32 * self.frame_time * self.clock_rate as f32) as u32;
        let prev_dropped_packets = (unit.frame_number - self.last_frame_number) as u16;
        self.last_frame_number = unit.frame_number;

        match unit.frame_type {
            FrameType::Idr => {
                for buffer in unit.buffers {
                    match buffer.ty {
                        BufferType::Sps
                        | BufferType::Pps
                        | BufferType::Vps
                        | BufferType::PicData => {
                            full_frame.extend_from_slice(buffer.data);
                        }
                    }
                }

                let data = Bytes::from(full_frame);

                self.decoder.send_sample(Sample {
                    data,
                    timestamp,
                    duration: Duration::from_secs_f32(frame_time),
                    packet_timestamp,
                    prev_dropped_packets,
                    prev_padding_packets: 0,
                });
            }
            FrameType::PFrame => {
                for buffer in unit.buffers {
                    full_frame.extend_from_slice(buffer.data);
                }

                let len = full_frame.len();
                let reader = Cursor::new(full_frame);
                let mut nal_reader = H264Reader::new(reader, len);

                while let Ok(nal) = nal_reader.next_nal() {
                    self.decoder.send_sample(Sample {
                        data: nal.data.into(),
                        timestamp,
                        duration: Duration::from_secs_f32(frame_time),
                        packet_timestamp,
                        ..Default::default()
                    });
                }
            }
        }

        if self.needs_idr {
            self.needs_idr = false;
            return DecodeResult::NeedIdr;
        }

        DecodeResult::Ok
    }

    fn supported_formats(&self) -> SupportedVideoFormats {
        // TODO: mask or just h264 and what about other formats?
        SupportedVideoFormats::H264
    }
    fn capabilities(&self) -> Capabilities {
        Capabilities::empty()
    }
}

fn video_format_to_mime_type(format: VideoFormat) -> Option<String> {
    match format {
        VideoFormat::H264 => Some(MIME_TYPE_H264.to_string()),
        VideoFormat::H265 => Some(MIME_TYPE_HEVC.to_string()),
        // TODO: more formats
        _ => None,
    }
}
