use std::{
    io::Cursor,
    sync::Arc,
    time::{Duration, SystemTime},
};

use actix_web::web::Bytes;
use log::{info, warn};
use moonlight_common::moonlight::{
    stream::Capabilities,
    video::{
        BufferType, DecodeResult, FrameType, SupportedVideoFormats, VideoDecodeUnit, VideoDecoder,
        VideoFormat,
    },
};
use tokio::{
    runtime::Handle,
    spawn,
    sync::mpsc::{Receiver, Sender, channel},
};
use webrtc::{
    media::{Sample, io::h264_reader::H264Reader},
    rtp::extension::{HeaderExtension, playout_delay_extension::PlayoutDelayExtension},
    track::track_local::track_local_static_sample::TrackLocalStaticSample,
};

use crate::api::stream::StreamStages;

pub struct H264TrackSampleVideoDecoder {
    video_track: Arc<TrackLocalStaticSample>,
    sender: Sender<Sample>,
    stages: Arc<StreamStages>,
    // Video important
    needs_idr: bool,
    frame_time: f32,
    last_frame_number: i32,
}

impl H264TrackSampleVideoDecoder {
    // TODO: maybe allow the Moonlight crate to decide the video format?
    pub fn new(
        video_track: Arc<TrackLocalStaticSample>,
        stages: Arc<StreamStages>,
        sample_send_queue_size: usize,
    ) -> Self {
        let (sender, receiver) = channel(20);

        spawn({
            let video_track = video_track.clone();
            async move {
                sample_sender(video_track, receiver).await;
            }
        });

        Self {
            video_track,
            sender,
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

                let _ = self.sender.try_send(Sample {
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
                    let _ = self.sender.try_send(Sample {
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
        // TODO: mask or just h264?
        SupportedVideoFormats::H264
    }
    fn capabilities(&self) -> Capabilities {
        Capabilities::empty()
    }
}

async fn sample_sender(video_track: Arc<TrackLocalStaticSample>, mut receiver: Receiver<Sample>) {
    while let Some(sample) = receiver.recv().await {
        if let Err(err) = video_track
            .write_sample_with_extensions(
                &sample,
                &[HeaderExtension::PlayoutDelay(PlayoutDelayExtension::new(
                    0, 0,
                ))],
            )
            .await
        {
            warn!("[Stream]: video_track.write_sample failed: {err}");
        }
    }
}
