use std::{
    io::Cursor,
    ops::Range,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, SystemTime},
};

use bytes::{Bytes, BytesMut};
use log::{debug, error, info};
use moonlight_common::stream::{
    bindings::{DecodeResult, SupportedVideoFormats, VideoDecodeUnit, VideoFormat},
    video::VideoDecoder,
};
use webrtc::{
    api::media_engine::{MIME_TYPE_AV1, MIME_TYPE_H264, MIME_TYPE_HEVC, MediaEngine},
    media::Sample,
    rtcp::payload_feedbacks::{
        picture_loss_indication::PictureLossIndication,
        receiver_estimated_maximum_bitrate::ReceiverEstimatedMaximumBitrate,
    },
    rtp::{
        codecs::{
            av1::Av1Payloader,
            h264::H264Payloader,
            h265::{HevcPayloader, RTP_OUTBOUND_MTU},
        },
        extension::abs_send_time_extension::AbsSendTimeExtension,
        header::Header,
        packet::Packet,
        packetizer::Payloader,
    },
    rtp_transceiver::{
        RTCPFeedback,
        rtp_codec::{RTCRtpCodecCapability, RTCRtpCodecParameters, RTPCodecType},
    },
    track::track_local::track_local_static_rtp::TrackLocalStaticRTP,
    util::{Marshal, MarshalSize},
};

use crate::{
    StreamConnection,
    sender::{SequencedTrackLocalStaticRTP, TrackLocalSender},
    video::{annexb::AnnexBSplitter, h264::H264Reader, h265::H265Reader},
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
        payloader: H264Payloader,
    },
    H265 {
        nal_reader: H265Reader<Cursor<Vec<u8>>>,
        payloader: HevcPayloader,
    },
    Av1 {
        annex_b: AnnexBSplitter<Cursor<Vec<u8>>>,
        payloader: Av1Payloader,
    },
}

pub struct TrackSampleVideoDecoder {
    sender: TrackLocalSender<SequencedTrackLocalStaticRTP>,
    clock_rate: u32,
    supported_formats: SupportedVideoFormats,
    // Video important
    video_codec: Option<VideoCodec>,
    samples: Vec<BytesMut>,
    needs_idr: Arc<AtomicBool>,
    old_presentation_time: Duration,
}

impl TrackSampleVideoDecoder {
    pub fn new(
        stream: Arc<StreamConnection>,
        supported_formats: SupportedVideoFormats,
        channel_queue_size: usize,
    ) -> Self {
        Self {
            sender: TrackLocalSender::new(stream, channel_queue_size),
            clock_rate: 0,
            supported_formats,
            video_codec: None,
            samples: Vec::new(),
            needs_idr: Default::default(),
            old_presentation_time: Duration::ZERO,
        }
    }
}

impl VideoDecoder for TrackSampleVideoDecoder {
    fn setup(
        &mut self,
        format: VideoFormat,
        width: u32,
        height: u32,
        redraw_rate: u32,
        _flags: i32,
    ) -> i32 {
        info!("[Stream] Stream setup: {width}x{height}x{redraw_rate} and {format:?}");

        {
            let mut video_size = self.sender.stream.video_size.blocking_lock();

            *video_size = (width, height);
        }

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

        self.clock_rate = codec.capability.clock_rate;

        let needs_idr = self.needs_idr.clone();
        if let Err(err) = self.sender.blocking_create_track(
            TrackLocalStaticRTP::new(
                codec.capability.clone(),
                "video".to_string(),
                "moonlight".to_string(),
            )
            .into(),
            move |packet| {
                let packet = packet.as_any();

                if packet.is::<PictureLossIndication>() {
                    needs_idr.store(true, Ordering::Release);
                }
                if let Some(_max_bitrate) = packet.downcast_ref::<ReceiverEstimatedMaximumBitrate>()
                {
                    // Moonlight doesn't support dynamic bitrate changing :(
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
                    payloader: Default::default(),
                });
            }
            // -- H265
            VideoFormat::H265
            | VideoFormat::H265Main10
            | VideoFormat::H265Rext8_444
            | VideoFormat::H265Rext10_444 => {
                self.video_codec = Some(VideoCodec::H265 {
                    nal_reader: H265Reader::new(Cursor::new(Vec::new()), 0),
                    payloader: Default::default(),
                });
            }
            // -- AV1
            VideoFormat::Av1Main8
            | VideoFormat::Av1Main10
            | VideoFormat::Av1High8_444
            | VideoFormat::Av1High10_444 => {
                self.video_codec = Some(VideoCodec::Av1 {
                    annex_b: AnnexBSplitter::new(Cursor::new(Vec::new()), 0),
                    payloader: Default::default(),
                });
            }
        }

        0
    }
    fn start(&mut self) {}
    fn stop(&mut self) {}

    fn submit_decode_unit(&mut self, unit: VideoDecodeUnit<'_>) -> DecodeResult {
        let timestamp = (unit.presentation_time.as_secs_f64() * self.clock_rate as f64) as u32;

        match &mut self.video_codec {
            // -- H264
            Some(VideoCodec::H264 {
                nal_reader,
                payloader,
            }) => {
                let mut full_frame = Vec::new();
                for buffer in unit.buffers {
                    full_frame.extend_from_slice(buffer.data);
                }

                nal_reader.reset(Cursor::new(full_frame));

                while let Ok(Some(nal)) = nal_reader.next_nal() {
                    let data = trim_bytes_to_range(
                        nal.full,
                        nal.header_range.start..nal.payload_range.end,
                    );

                    self.samples.push(data);
                }

                for sample in self.samples.drain(..) {
                    // TODO: don't unwrap
                    for packet in packetize(
                        payloader,
                        RTP_OUTBOUND_MTU,
                        0, // is set in the write fn
                        timestamp,
                        &sample.freeze(),
                    )
                    .unwrap()
                    {
                        self.sender.blocking_send_sample(packet);
                    }
                }
            }
            // -- H265
            Some(VideoCodec::H265 {
                nal_reader,
                payloader,
            }) => {
                let mut full_frame = Vec::new();
                for buffer in unit.buffers {
                    full_frame.extend_from_slice(buffer.data);
                }

                nal_reader.reset(Cursor::new(full_frame));

                while let Ok(Some(nal)) = nal_reader.next_nal() {
                    todo!();
                    // self.decoder.blocking_send_sample(Sample {
                    //     // Full includes annex b prefix which this sample reader requires
                    //     data: nal.full.freeze(),
                    //     timestamp,
                    //     duration,
                    //     packet_timestamp,
                    //     ..Default::default() // <-- Must be default
                    // });
                }
            }
            // -- AV1
            Some(VideoCodec::Av1 { annex_b, payloader }) => {
                let mut full_frame = Vec::new();
                for buffer in unit.buffers {
                    full_frame.extend_from_slice(buffer.data);
                }

                annex_b.reset(Cursor::new(full_frame));

                while let Ok(Some(annex_b_payload)) = annex_b.next() {
                    let data =
                        trim_bytes_to_range(annex_b_payload.full, annex_b_payload.payload_range);

                    todo!();
                    // self.decoder.blocking_send_sample(Sample {
                    //     data: data.freeze(),
                    //     timestamp,
                    //     duration,
                    //     packet_timestamp,
                    //     ..Default::default()
                    // });
                }
            }
            None => {
                // this shouldn't happen
                unreachable!()
            }
        }

        self.old_presentation_time = unit.presentation_time;

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

fn packetize(
    payloader: &mut impl Payloader,
    mtu: usize,
    sequence_number: u16,
    timestamp: u32,
    payload: &Bytes,
) -> Result<Vec<Packet>, anyhow::Error> {
    let payloads = payloader.payload(mtu - 12, payload)?;
    let payloads_len = payloads.len();
    let mut packets = Vec::with_capacity(payloads_len);
    for (i, payload) in payloads.into_iter().enumerate() {
        packets.push(Packet {
            header: Header {
                version: 2,
                padding: false,
                extension: false,
                marker: i == payloads_len - 1,
                sequence_number,
                timestamp,
                payload_type: 0, // Value is handled when writing
                ssrc: 0,         // Value is handled when writing
                ..Default::default()
            },
            payload,
        });
    }

    // if payloads_len != 0 {
    //     let st = SystemTime::now();
    //     let send_time = AbsSendTimeExtension::new(st);

    //     //apply http://www.webrtc.org/experiments/rtp-hdrext/abs-send-time
    //     let mut raw = BytesMut::with_capacity(send_time.marshal_size());
    //     raw.resize(send_time.marshal_size(), 0);
    //     let _ = send_time.marshal_to(&mut raw)?;
    //     packets[payloads_len - 1]
    //         .header
    //         // TODO: what should 0 be?
    //         .set_extension(0, raw.freeze())?;
    // }

    Ok(packets)
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
