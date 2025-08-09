use std::{
    io::{BufReader, Cursor},
    sync::Arc,
    time::{Duration, SystemTime},
};

use actix_web::{rt::System, web::Bytes};
use log::{info, warn};
use moonlight_common::{
    stream::Capabilities,
    video::{
        BufferType, DecodeResult, FrameType, SupportedVideoFormats, VideoDecodeUnit, VideoDecoder,
        VideoFormat,
    },
};
use tokio::runtime::Handle;
use webrtc::{
    media::{Sample, io::h264_reader::H264Reader},
    track::track_local::track_local_static_sample::TrackLocalStaticSample,
};

use crate::api::stream::StreamStages;

pub struct H264TrackSampleVideoDecoder {
    runtime: Handle,
    video_track: Arc<TrackLocalStaticSample>,
    stages: Arc<StreamStages>,
    // Video important
    needs_idr: bool,
    frame_time: f32,
    last_frame_number: i32,
}

impl H264TrackSampleVideoDecoder {
    // TODO: maybe allow the Moonlight crate to decide the video format?
    pub fn new(video_track: Arc<TrackLocalStaticSample>, stages: Arc<StreamStages>) -> Self {
        Self {
            runtime: Handle::current(),
            video_track,
            stages,
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
        flags: (),
    ) -> i32 {
        info!("[Stream] Streaming with format: {format:?}");

        if !format.contained_in(self.supported_formats()) {
            warn!(
                "tried to setup a video stream with a non supported video format: {format:?}, supported formats: {:?}",
                self.supported_formats()
            );
            return -1;
        }

        self.frame_time = 1.0 / redraw_rate as f32;

        0
    }
    fn start(&mut self) {}
    fn stop(&mut self) {
        self.stages.stop.set_reached();
    }

    fn submit_decode_unit(&mut self, unit: VideoDecodeUnit<'_>) -> DecodeResult {
        if self.stages.stop.is_reached() {
            return DecodeResult::Ok;
        }

        if !self.stages.connected.is_reached() {
            return DecodeResult::Ok;
        }

        let mut full_frame = Vec::new();

        let frame_time = self.frame_time;
        let timestamp =
            SystemTime::UNIX_EPOCH + Duration::from_millis(unit.presentation_time_ms as u64);
        let packet_timestamp = (unit.frame_number as f32
            * self.frame_time
            * self.video_track.codec().clock_rate as f32) as u32;
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
                let video_track = self.video_track.clone();

                self.runtime.spawn(async move {
                    if let Err(err) = video_track
                        .write_sample(&Sample {
                            data,
                            timestamp,
                            duration: Duration::from_secs_f32(frame_time),
                            packet_timestamp,
                            prev_dropped_packets,
                            prev_padding_packets: 0,
                        })
                        .await
                    {
                        warn!("[Stream]: video_track.write_sample failed: {err}");
                    }
                });
            }
            FrameType::PFrame => {
                for buffer in unit.buffers {
                    full_frame.extend_from_slice(buffer.data);
                }

                let len = full_frame.len();
                let reader = Cursor::new(full_frame);
                let mut nal_reader = H264Reader::new(reader, len);
                let video_track = self.video_track.clone();

                while let Ok(nal) = nal_reader.next_nal() {
                    let video_track = video_track.clone();

                    self.runtime.spawn(async move {
                        if let Err(err) = video_track
                            .write_sample(&Sample {
                                data: nal.data.into(),
                                timestamp,
                                duration: Duration::from_secs_f32(frame_time),
                                packet_timestamp,
                                ..Default::default()
                            })
                            .await
                        {
                            warn!("write_sample failed: {err}");
                        }
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
        // TODO: mask or just h264?
        SupportedVideoFormats::H264
    }
    fn capabilities(&self) -> Capabilities {
        Capabilities::empty()
    }
}
