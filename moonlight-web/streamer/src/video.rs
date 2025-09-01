use std::{
    io::Cursor,
    ops::Range,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, SystemTime},
};

use bytes::{BufMut, Bytes, BytesMut};
use log::{debug, error, info};
use moonlight_common::stream::{
    bindings::{DecodeResult, FrameType, SupportedVideoFormats, VideoDecodeUnit, VideoFormat},
    video::VideoDecoder,
};
use webrtc::{
    api::media_engine::{MIME_TYPE_AV1, MIME_TYPE_H264, MIME_TYPE_HEVC, MediaEngine},
    media::Sample,
    rtcp::payload_feedbacks::{
        picture_loss_indication::PictureLossIndication,
        receiver_estimated_maximum_bitrate::ReceiverEstimatedMaximumBitrate,
    },
    rtp_transceiver::{
        RTCPFeedback,
        rtp_codec::{RTCRtpCodecCapability, RTCRtpCodecParameters, RTPCodecType},
    },
    track::track_local::track_local_static_sample::TrackLocalStaticSample,
};

use crate::{
    StreamConnection,
    decoder::TrackSampleDecoder,
    video::{
        annexb::{AnnexBSplitter, AnnexBStartCode},
        h264::H264Reader,
        h265::H265Reader,
    },
};

mod annexb;
mod h264;
mod h265;

pub fn register_video_codecs(
    media_engine: &mut MediaEngine,
    supported_video_formats: SupportedVideoFormats,
) -> Result<(), webrtc::Error> {
    for format in VideoFormat::all() {
        if !format.contained_in(supported_video_formats) {
            continue;
        }

        let Some(codec) = video_format_to_codec(format) else {
            continue;
        };
        debug!(
            "Registering Video Format {format:?}, Codec: {:?}",
            codec.capability
        );

        media_engine.register_codec(codec, RTPCodecType::Video)?;
    }

    Ok(())
}

enum VideoCodec {
    H264 {
        nal_reader: H264Reader<Cursor<Vec<u8>>>,
    },
    H265 {
        nal_reader: H265Reader<Cursor<Vec<u8>>>,
    },
    Av1 {
        annex_b: AnnexBSplitter<Cursor<Vec<u8>>>,
    },
}

pub struct TrackSampleVideoDecoder {
    decoder: TrackSampleDecoder,
    supported_formats: SupportedVideoFormats,
    clock_rate: u32,
    // Video important
    video_codec: Option<VideoCodec>,
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
            supported_formats,
            clock_rate: 90000,
            video_codec: None,
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
                self.supported_formats().iter_names().collect::<Vec<_>>()
            );
            return -1;
        }

        let Some(codec) = video_format_to_codec(format) else {
            error!("Failed to get video codec with format {format:?}");
            return -1;
        };

        let needs_idr = self.needs_idr.clone();
        if let Err(err) = self.decoder.blocking_create_track(
            TrackLocalStaticSample::new(
                codec.capability.clone(),
                "video".to_string(),
                "moonlight".to_string(),
            ),
            move |packet| {
                let packet = packet.as_any();

                if packet.is::<PictureLossIndication>() {
                    needs_idr.store(true, Ordering::Release);
                }
                if let Some(max_bitrate) = packet.downcast_ref::<ReceiverEstimatedMaximumBitrate>()
                {
                    // TODO: set moonlight bitrate if possible?
                    // TODO: make this an option
                }
            },
        ) {
            error!(
                "Failed to create video track with format {format:?} and codec \"{codec:?}\": {err:?}"
            );
            return -1;
        }

        match format {
            // -- H264
            VideoFormat::H264 | VideoFormat::H264High8_444 => {
                self.video_codec = Some(VideoCodec::H264 {
                    nal_reader: H264Reader::new(Cursor::new(Vec::new()), 0),
                });
            }
            // -- H265
            VideoFormat::H265
            | VideoFormat::H265Main10
            | VideoFormat::H265Rext8_444
            | VideoFormat::H265Rext10_444 => {
                self.video_codec = Some(VideoCodec::H265 {
                    nal_reader: H265Reader::new(Cursor::new(Vec::new()), 0),
                });
            }
            // -- AV1
            VideoFormat::Av1Main8
            | VideoFormat::Av1Main10
            | VideoFormat::Av1High8_444
            | VideoFormat::Av1High10_444 => {
                self.video_codec = Some(VideoCodec::Av1 {
                    annex_b: AnnexBSplitter::new(Cursor::new(Vec::new()), 0),
                });
            }
        }

        self.frame_time = 1.0 / redraw_rate as f32;

        0
    }
    fn start(&mut self) {}
    fn stop(&mut self) {}

    fn submit_decode_unit(&mut self, unit: VideoDecodeUnit<'_>) -> DecodeResult {
        let frame_time = self.frame_time;
        let duration = Duration::from_secs_f32(frame_time);
        let timestamp = SystemTime::UNIX_EPOCH + unit.presentation_time;
        let packet_timestamp =
            (unit.frame_number as f32 * self.frame_time * self.clock_rate as f32) as u32;
        let prev_dropped_packets = (unit.frame_number - self.last_frame_number) as u16;
        self.last_frame_number = unit.frame_number;

        match &mut self.video_codec {
            // -- H264
            Some(VideoCodec::H264 { nal_reader }) => {
                let mut full_frame = Vec::new();
                for buffer in unit.buffers {
                    full_frame.extend_from_slice(buffer.data);
                }

                // TODO: do we need this match?
                match unit.frame_type {
                    FrameType::Idr => {
                        let data = Bytes::from(full_frame);

                        // We need this to be delivered
                        self.decoder.blocking_send_sample(Sample {
                            data,
                            timestamp,
                            duration,
                            packet_timestamp,
                            prev_dropped_packets,
                            prev_padding_packets: 0,
                        });
                    }
                    FrameType::PFrame => {
                        nal_reader.reset(Cursor::new(full_frame));

                        while let Ok(Some(nal)) = nal_reader.next_nal() {
                            let data = trim_bytes_to_range(
                                nal.full,
                                nal.header_range.start..nal.payload_range.end,
                            );

                            self.decoder.blocking_send_sample(Sample {
                                data: data.freeze(),
                                timestamp,
                                duration,
                                packet_timestamp,
                                ..Default::default() // <-- Must be default
                            });
                        }
                    }
                };
            }
            // -- H265
            Some(VideoCodec::H265 { nal_reader }) => {
                let mut full_frame = Vec::new();
                for buffer in unit.buffers {
                    full_frame.extend_from_slice(buffer.data);
                }

                nal_reader.reset(Cursor::new(full_frame));

                while let Ok(Some(nal)) = nal_reader.next_nal() {
                    let nal_data = trim_bytes_to_range(
                        nal.full,
                        nal.header_range.start..nal.payload_range.end,
                    );

                    log::debug!("NAL: {:?}", nal.header);

                    // TODO: use pushfront on nal or if already b3 use it
                    let mut data = BytesMut::new();
                    data.put(AnnexBStartCode::B3.code());
                    data.put(nal_data);

                    self.decoder.blocking_send_sample(Sample {
                        data: data.freeze(),
                        timestamp,
                        duration,
                        packet_timestamp,
                        ..Default::default() // <-- Must be default
                    });
                }
            }
            // -- AV1
            Some(VideoCodec::Av1 { annex_b }) => {
                let mut full_frame = Vec::new();
                for buffer in unit.buffers {
                    full_frame.extend_from_slice(buffer.data);
                }

                annex_b.reset(Cursor::new(full_frame));

                while let Ok(Some(annex_b_payload)) = annex_b.next() {
                    let data =
                        trim_bytes_to_range(annex_b_payload.full, annex_b_payload.payload_range);

                    self.decoder.blocking_send_sample(Sample {
                        data: data.freeze(),
                        timestamp,
                        duration,
                        packet_timestamp,
                        ..Default::default()
                    });
                }
            }
            None => {
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
}

fn video_format_to_codec(format: VideoFormat) -> Option<RTCRtpCodecParameters> {
    let rtcp_feedback = vec![
        RTCPFeedback {
            typ: "nack".to_string(),
            parameter: "".to_string(),
        },
        RTCPFeedback {
            typ: "nack".to_string(),
            parameter: "pli".to_string(),
        },
        RTCPFeedback {
            typ: "goog-remb".to_string(),
            parameter: "".to_string(),
        },
    ];

    match format {
        // -- H264 Constrained Baseline Profile
        VideoFormat::H264 => Some(RTCRtpCodecParameters {
            capability: RTCRtpCodecCapability {
                mime_type: MIME_TYPE_H264.to_owned(),
                clock_rate: 90000,
                channels: 0,
                sdp_fmtp_line:
                    "level-asymmetry-allowed=1;packetization-mode=1;profile-level-id=42e01f"
                        .to_owned(),
                rtcp_feedback: rtcp_feedback.clone(),
            },
            payload_type: 123,
            ..Default::default()
        }),
        // -- H264 High Profile
        VideoFormat::H264High8_444 => Some(RTCRtpCodecParameters {
            capability: RTCRtpCodecCapability {
                mime_type: MIME_TYPE_H264.to_owned(),
                clock_rate: 90000,
                channels: 0,
                sdp_fmtp_line:
                    "level-asymmetry-allowed=1;packetization-mode=1;profile-level-id=640032"
                        .to_owned(),
                rtcp_feedback: rtcp_feedback.clone(),
            },
            payload_type: 124,
            ..Default::default()
        }),

        // TODO: h265 requires resolution in the level-id field, set it based on resolution and fps
        // -- H265 Main Profile
        VideoFormat::H265 => Some(RTCRtpCodecParameters {
            capability: RTCRtpCodecCapability {
                mime_type: MIME_TYPE_HEVC.to_owned(),
                clock_rate: 90000,
                channels: 0,
                // They're the same
                // sdp_fmtp_line: "profile-id=1;level-id=93;tier-flag=0;tx-mode=1".to_owned(),
                sdp_fmtp_line: "".to_owned(),
                rtcp_feedback: rtcp_feedback.clone(),
            },
            payload_type: 126,
            ..Default::default()
        }),
        // -- H265 Main10 Profile
        VideoFormat::H265Main10 => Some(RTCRtpCodecParameters {
            capability: RTCRtpCodecCapability {
                mime_type: MIME_TYPE_HEVC.to_owned(),
                clock_rate: 90000,
                channels: 0,
                sdp_fmtp_line: "profile-id=2;tier-flag=0;level-id=93;tx-mode=SRST".to_owned(),
                rtcp_feedback: rtcp_feedback.clone(),
            },
            payload_type: 127,
            ..Default::default()
        }),
        // -- H265 RExt 4:4:4 8-bit
        VideoFormat::H265Rext8_444 => Some(RTCRtpCodecParameters {
            capability: RTCRtpCodecCapability {
                mime_type: MIME_TYPE_HEVC.to_owned(),
                clock_rate: 90000,
                channels: 0,
                sdp_fmtp_line: "profile-id=4;tier-flag=0;level-id=120;tx-mode=SRST".to_owned(),
                rtcp_feedback: rtcp_feedback.clone(),
            },
            payload_type: 128,
            ..Default::default()
        }),
        // -- H265 RExt 4:4:4 10-bit
        VideoFormat::H265Rext10_444 => Some(RTCRtpCodecParameters {
            capability: RTCRtpCodecCapability {
                mime_type: MIME_TYPE_HEVC.to_owned(),
                clock_rate: 90000,
                channels: 0,
                sdp_fmtp_line: "profile-id=5;tier-flag=0;level-id=93;tx-mode=SRST".to_owned(),
                rtcp_feedback: rtcp_feedback.clone(),
            },
            payload_type: 129,
            ..Default::default()
        }),

        // -- Av1
        VideoFormat::Av1Main8 | VideoFormat::Av1Main10 => Some(RTCRtpCodecParameters {
            capability: RTCRtpCodecCapability {
                mime_type: MIME_TYPE_AV1.to_owned(),
                clock_rate: 90000,
                channels: 0,
                sdp_fmtp_line: "profile=0".to_owned(),
                rtcp_feedback: rtcp_feedback.clone(),
            },
            payload_type: 41,
            ..Default::default()
        }),
        VideoFormat::Av1High8_444 | VideoFormat::Av1High10_444 => Some(RTCRtpCodecParameters {
            capability: RTCRtpCodecCapability {
                mime_type: MIME_TYPE_AV1.to_owned(),
                clock_rate: 90000,
                channels: 0,
                sdp_fmtp_line: "profile=1".to_owned(),
                rtcp_feedback: rtcp_feedback.clone(),
            },
            payload_type: 130,
            ..Default::default()
        }),
    }
}

fn trim_bytes_to_range(mut buf: BytesMut, range: Range<usize>) -> BytesMut {
    if range.start > 0 {
        let _ = buf.split_to(range.start);
    }

    if range.end - range.start < buf.len() {
        let _ = buf.split_off(range.end - range.start);
    }

    buf
}
