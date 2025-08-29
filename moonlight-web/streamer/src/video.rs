use std::{
    io::Cursor,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, SystemTime},
};

use bytes::Bytes;
use log::{error, info};
use moonlight_common::moonlight::{
    stream::Capabilities,
    video::{
        BufferType, DecodeResult, FrameType, SupportedVideoFormats, VideoDecodeUnit, VideoDecoder,
        VideoFormat,
    },
};
use webrtc::{
    media::{Sample, io::h264_reader::H264Reader},
    rtcp::payload_feedbacks::{
        full_intra_request::FullIntraRequest, picture_loss_indication::PictureLossIndication,
    },
    rtp_transceiver::rtp_codec::RTCRtpCodecCapability,
    track::track_local::track_local_static_sample::TrackLocalStaticSample,
};

use crate::{StreamConnection, decoder::TrackSampleDecoder};

pub struct TrackSampleVideoDecoder {
    decoder: TrackSampleDecoder,
    supported_formats: SupportedVideoFormats,
    clock_rate: u32,
    // Video important
    current_video_format: Option<VideoFormat>,
    needs_idr: Arc<AtomicBool>,
    frame_time: f32,
    last_frame_number: i32,
}

impl TrackSampleVideoDecoder {
    pub fn new(
        stream: Arc<StreamConnection>,
        supported_formats: SupportedVideoFormats,
        channel_queue_size: usize,
    ) -> Self {
        Self {
            decoder: TrackSampleDecoder::new(stream, channel_queue_size),
            // TODO: implement other formats?
            supported_formats: supported_formats & SupportedVideoFormats::MASK_H264,
            clock_rate: 90000,
            current_video_format: None,
            needs_idr: Default::default(),
            frame_time: 0.0,
            last_frame_number: 0,
        }
    }
}

impl VideoDecoder for TrackSampleVideoDecoder {
    fn setup(
        &mut self,
        format: VideoFormat,
        _width: u32,
        _height: u32,
        redraw_rate: u32,
        _flags: i32,
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

        let needs_idr = self.needs_idr.clone();

        if let Err(err) = self.decoder.blocking_create_track(
            TrackLocalStaticSample::new(
                RTCRtpCodecCapability {
                    // TODO: is it possible to make the video channel unreliable?
                    mime_type: mime_type.clone(),
                    clock_rate: self.clock_rate,
                    ..Default::default()
                },
                "video".to_string(),
                "moonlight".to_string(),
            ),
            move |packet| {
                let packet = packet.as_any();

                if packet.is::<PictureLossIndication>() || packet.is::<FullIntraRequest>() {
                    needs_idr.store(true, Ordering::Release);
                }
            },
        ) {
            error!(
                "Failed to create video track with format {format:?} and mime \"{mime_type}\": {err:?}"
            );
            return -1;
        }

        self.current_video_format = Some(format);

        self.frame_time = 1.0 / redraw_rate as f32;

        0
    }
    fn start(&mut self) {}
    fn stop(&mut self) {}

    fn submit_decode_unit(&mut self, unit: VideoDecodeUnit<'_>) -> DecodeResult {
        let frame_time = self.frame_time;
        let timestamp = SystemTime::UNIX_EPOCH + unit.presentation_time;
        let packet_timestamp =
            (unit.frame_number as f32 * self.frame_time * self.clock_rate as f32) as u32;
        let prev_dropped_packets = (unit.frame_number - self.last_frame_number) as u16;
        self.last_frame_number = unit.frame_number;

        match self.current_video_format {
            // -- H264
            Some(VideoFormat::H264) | Some(VideoFormat::H264High8_444) => {
                let mut full_frame = Vec::new();
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

                        // We need this to be delivered
                        self.decoder.blocking_send_sample(Sample {
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
                            self.decoder.blocking_send_sample(Sample {
                                data: nal.data.into(),
                                timestamp,
                                duration: Duration::from_secs_f32(frame_time),
                                packet_timestamp,
                                ..Default::default()
                            });
                        }
                    }
                };
            }
            // -- H265
            Some(VideoFormat::H265)
            | Some(VideoFormat::H265Main10)
            | Some(VideoFormat::H265Rext8_444)
            | Some(VideoFormat::H265Rext10_444) => {
                // https://stackoverflow.com/questions/59311873/how-to-depacketize-the-fragmented-frames-in-rtp-data-over-udp-for-h265-hevc
                todo!()
            }
            // -- AV1
            Some(VideoFormat::Av1Main8)
            | Some(VideoFormat::Av1Main10)
            | Some(VideoFormat::Av1High8_444)
            | Some(VideoFormat::Av1High10_444) => {
                // https://github.com/memorysafety/rav1d
                todo!()
            }
            _ => {
                // this shouldn't happen
                unreachable!()
            }
        }

        if self
            .needs_idr
            .compare_exchange_weak(true, false, Ordering::SeqCst, Ordering::Relaxed)
            .is_ok()
        {
            return DecodeResult::NeedIdr;
        }

        DecodeResult::Ok
    }

    fn supported_formats(&self) -> SupportedVideoFormats {
        self.supported_formats
    }
    fn capabilities(&self) -> Capabilities {
        Capabilities::empty()
    }
}

fn video_format_to_mime_type(format: VideoFormat) -> Option<String> {
    match format {
        VideoFormat::H264 => Some("video/H264".to_string()),
        VideoFormat::H264High8_444 => Some("video/H264".to_string()),
        VideoFormat::H265 => Some("video/H265".to_string()),
        VideoFormat::H265Main10 => Some("video/H265".to_string()),
        VideoFormat::H265Rext8_444 => Some("video/H265".to_string()),
        VideoFormat::H265Rext10_444 => Some("video/H265".to_string()),
        VideoFormat::Av1Main8 => Some("video/AV1".to_string()),
        VideoFormat::Av1Main10 => Some("video/AV1".to_string()),
        VideoFormat::Av1High8_444 => Some("video/AV1".to_string()),
        VideoFormat::Av1High10_444 => Some("video/AV1".to_string()),
    }
}
