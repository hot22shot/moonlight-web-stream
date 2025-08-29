use std::{
    io::Cursor,
    ops::Range,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, SystemTime},
};

use base64::{Engine, engine::general_purpose};
use bytes::{BufMut, Bytes, BytesMut};
use log::{error, info};
use moonlight_common::moonlight::{
    stream::Capabilities,
    video::{
        DecodeResult, FrameType, SupportedVideoFormats, VideoDecodeUnit, VideoDecoder, VideoFormat,
    },
};
use tokio::time::sleep;
use webrtc::{
    api::media_engine::{MIME_TYPE_AV1, MIME_TYPE_H264, MIME_TYPE_HEVC},
    media::Sample,
    rtcp::payload_feedbacks::{
        full_intra_request::FullIntraRequest, picture_loss_indication::PictureLossIndication,
    },
    rtp_transceiver::rtp_codec::RTCRtpCodecCapability,
    track::track_local::track_local_static_sample::TrackLocalStaticSample,
};

use crate::{
    StreamConnection,
    decoder::TrackSampleDecoder,
    video::{h264::H264Reader, h265::H265Reader},
};

mod annexb;
mod h264;
mod h265;

enum Reader {
    H264 {
        nal_reader: H264Reader<Cursor<Vec<u8>>>,
    },
    H265Init {
        format: VideoFormat,
        nal_reader: H265Reader<Cursor<Vec<u8>>>,
        vps: Option<BytesMut>,
        sps: Option<BytesMut>,
        pps: Option<BytesMut>,
        idr: Option<BytesMut>,
    },
    H265 {
        nal_reader: H265Reader<Cursor<Vec<u8>>>,
    },
}

pub struct TrackSampleVideoDecoder {
    decoder: TrackSampleDecoder,
    supported_formats: SupportedVideoFormats,
    clock_rate: u32,
    // Video important
    current_state: Option<Reader>,
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
            supported_formats: supported_formats & SupportedVideoFormats::H265,
            clock_rate: 90000,
            current_state: None,
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

        let mime_type = video_format_to_mime_type(format);

        let needs_idr = self.needs_idr.clone();

        match format {
            // -- H264
            VideoFormat::H264 | VideoFormat::H264High8_444 => {
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

                self.current_state = Some(Reader::H264 {
                    nal_reader: H264Reader::new(Cursor::new(Vec::new()), 0),
                });
            }
            // -- H265
            VideoFormat::H265
            | VideoFormat::H265Main10
            | VideoFormat::H265Rext8_444
            | VideoFormat::H265Rext10_444 => {
                self.current_state = Some(Reader::H265Init {
                    format,
                    nal_reader: H265Reader::new(Cursor::new(Vec::new()), 0),
                    vps: None,
                    sps: None,
                    pps: None,
                    idr: None,
                });
            }
            // -- AV1
            VideoFormat::Av1Main8
            | VideoFormat::Av1Main10
            | VideoFormat::Av1High8_444
            | VideoFormat::Av1High10_444 => {
                todo!()
            }
        }

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

        match &mut self.current_state {
            // -- H264
            Some(Reader::H264 { nal_reader }) => {
                // TODO: implement other modes: https://datatracker.ietf.org/doc/html/rfc3984#section-6

                let mut full_frame = Vec::new();
                for buffer in unit.buffers {
                    full_frame.extend_from_slice(buffer.data);
                }

                match unit.frame_type {
                    FrameType::Idr => {
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
                        let reader = Cursor::new(full_frame);
                        nal_reader.reset(reader);

                        while let Ok(Some(nal)) = nal_reader.next_nal() {
                            let data = trim_bytes_to_range(
                                nal.full,
                                nal.header_range.start..nal.payload_range.end,
                            );

                            self.decoder.blocking_send_sample(Sample {
                                data: data.freeze(),
                                timestamp,
                                duration: Duration::from_secs_f32(frame_time),
                                packet_timestamp,
                                ..Default::default() // <-- Must be default
                            });
                        }
                    }
                };
            }
            // -- H265
            Some(Reader::H265Init {
                nal_reader,
                vps,
                sps,
                pps,
                idr,
                ..
            }) => {
                // Collect vps, sps, pps

                let mut full_frame = Vec::new();
                for buffer in unit.buffers {
                    full_frame.extend_from_slice(buffer.data);
                }
                let reader = Cursor::new(full_frame);
                nal_reader.reset(reader);

                while let Ok(Some(nal)) = nal_reader.next_nal() {
                    if matches!(
                        nal.header.nal_unit_type,
                        h265::NALUnitType::VpsNut
                            | h265::NALUnitType::SpsNut
                            | h265::NALUnitType::PpsNut
                            | h265::NALUnitType::IdrNLp
                            | h265::NALUnitType::IdrWRadl
                    ) {
                        let data = trim_bytes_to_range(nal.full, nal.payload_range);

                        match nal.header.nal_unit_type {
                            h265::NALUnitType::VpsNut => {
                                vps.replace(data);
                            }
                            h265::NALUnitType::SpsNut => {
                                sps.replace(data);
                            }
                            h265::NALUnitType::PpsNut => {
                                pps.replace(data);
                            }
                            h265::NALUnitType::IdrNLp | h265::NALUnitType::IdrWRadl => {
                                idr.replace(data);
                            }
                            _ => unreachable!(),
                        }
                    }

                    if vps.is_some() && pps.is_some() && sps.is_some() && idr.is_some() {
                        break;
                    }
                }
            }
            Some(Reader::H265 { nal_reader }) => {
                let mut full_frame = Vec::new();
                for buffer in unit.buffers {
                    full_frame.extend_from_slice(buffer.data);
                }
                let reader = Cursor::new(full_frame);
                nal_reader.reset(reader);

                match unit.frame_type {
                    FrameType::Idr => {
                        let mut full_idr = BytesMut::new();

                        while let Ok(Some(nal)) = nal_reader.next_nal() {
                            let data = trim_bytes_to_range(
                                nal.full,
                                nal.header_range.start..nal.payload_range.end,
                            );

                            full_idr.put(data);
                        }

                        self.decoder.blocking_send_sample(Sample {
                            data: full_idr.freeze(),
                            timestamp,
                            duration: Duration::from_secs_f32(frame_time),
                            packet_timestamp,
                            ..Default::default() // <-- Must be default
                        });
                    }
                    FrameType::PFrame => {
                        while let Ok(Some(nal)) = nal_reader.next_nal() {
                            let data = trim_bytes_to_range(
                                nal.full,
                                nal.header_range.start..nal.payload_range.end,
                            );

                            self.decoder.blocking_send_sample(Sample {
                                data: data.freeze(),
                                timestamp,
                                duration: Duration::from_secs_f32(frame_time),
                                packet_timestamp,
                                ..Default::default() // <-- Must be default
                            });
                        }
                    }
                }
            }
            // -- AV1
            // _ => {
            //     todo!()
            // }
            None => {
                // this shouldn't happen
                unreachable!()
            }
        }

        // -- Init H265
        if matches!(
            &self.current_state,
            Some(Reader::H265Init {
                vps: Some(_),
                sps: Some(_),
                pps: Some(_),
                idr: Some(_),
                ..
            })
        ) {
            let Some(Reader::H265Init {
                format,
                mut nal_reader,
                vps: Some(vps),
                sps: Some(sps),
                pps: Some(pps),
                idr: Some(idr),
            }) = self.current_state.take()
            else {
                unreachable!()
            };

            // Create the new track with the vps, pps, sps
            let mime_type = video_format_to_mime_type(format);

            let b64_vps = general_purpose::STANDARD.encode(&vps);
            let b64_sps = general_purpose::STANDARD.encode(&sps);
            let b64_pps = general_purpose::STANDARD.encode(&pps);

            let sdp_fmtp_line = format!(
                "sprop-vps={};sprop-sps={};sprop-pps={}",
                b64_vps, b64_sps, b64_pps
            );
            log::info!("{}", sdp_fmtp_line);

            let needs_idr = self.needs_idr.clone();
            if let Err(err) = self.decoder.blocking_create_track(
                TrackLocalStaticSample::new(
                    RTCRtpCodecCapability {
                        // TODO: is it possible to make the video channel unreliable?
                        mime_type: mime_type.clone(),
                        clock_rate: self.clock_rate,
                        sdp_fmtp_line,
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

                let stream = self.decoder.stream.clone();
                self.decoder.stream.runtime.spawn(async move {
                    stream.stop().await;
                });
                return DecodeResult::Ok;
            }

            // renegotiate
            let stream = self.decoder.stream.clone();
            self.decoder.stream.runtime.spawn(async move {
                sleep(Duration::from_secs(1)).await;
                stream.renegotiate().await;
            });

            // send the vps, sps, pps
            let mut first_sample = BytesMut::new();
            first_sample.put(vps);
            first_sample.put(sps);
            first_sample.put(pps);
            first_sample.put(idr);
            self.decoder.blocking_send_sample(Sample {
                data: first_sample.freeze(),
                timestamp,
                duration: Duration::from_secs_f32(frame_time),
                packet_timestamp,
                ..Default::default() // <-- Must be default
            });

            // send the remaining frames
            while let Ok(Some(nal)) = nal_reader.next_nal() {
                let data =
                    trim_bytes_to_range(nal.full, nal.header_range.start..nal.payload_range.end);

                self.decoder.blocking_send_sample(Sample {
                    data: data.freeze(),
                    timestamp,
                    duration: Duration::from_secs_f32(frame_time),
                    packet_timestamp,
                    ..Default::default() // <-- Must be default
                });
            }

            // Set the state
            self.current_state = Some(Reader::H265 { nal_reader });
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

fn video_format_to_mime_type(format: VideoFormat) -> String {
    match format {
        VideoFormat::H264 => MIME_TYPE_H264.to_string(),
        VideoFormat::H264High8_444 => MIME_TYPE_H264.to_string(),
        VideoFormat::H265 => MIME_TYPE_HEVC.to_string(),
        VideoFormat::H265Main10 => MIME_TYPE_HEVC.to_string(),
        VideoFormat::H265Rext8_444 => MIME_TYPE_HEVC.to_string(),
        VideoFormat::H265Rext10_444 => MIME_TYPE_HEVC.to_string(),
        VideoFormat::Av1Main8 => MIME_TYPE_AV1.to_string(),
        VideoFormat::Av1Main10 => MIME_TYPE_AV1.to_string(),
        VideoFormat::Av1High8_444 => MIME_TYPE_AV1.to_string(),
        VideoFormat::Av1High10_444 => MIME_TYPE_AV1.to_string(),
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
