use std::{
    sync::Arc,
    time::{Duration, SystemTime},
};

use actix_web::web::Bytes;
use log::{info, warn};
use moonlight_common::{
    stream::Capabilities,
    video::{
        BufferType, DecodeResult, FrameType, SupportedVideoFormats, VideoDecodeUnit, VideoDecoder,
        VideoFormat,
    },
};
use tokio::{
    runtime::Handle,
    sync::mpsc::{Receiver, Sender, channel},
};
use webrtc::{
    media::{Sample, io::h264_reader::NAL},
    rtp::packetizer::Depacketizer,
    track::track_local::track_local_static_sample::TrackLocalStaticSample,
};

pub struct H264TrackSampleVideoDecoder {
    runtime: Handle,
    video_track: Option<Arc<TrackLocalStaticSample>>,
    receiver: Receiver<Arc<TrackLocalStaticSample>>,
    sender: Sender<Arc<TrackLocalStaticSample>>,
    stopped: bool,
    // Video important
    needs_idr: bool,
    frame_time: f32,
    last_frame_number: i32,
}

impl H264TrackSampleVideoDecoder {
    // TODO: maybe allow the Moonlight crate to decide the video format?
    pub fn new(video_track: Option<Arc<TrackLocalStaticSample>>) -> Self {
        let (sender, receiver) = channel(1);

        Self {
            runtime: Handle::current(),
            video_track,
            needs_idr: false,
            frame_time: 0.0,
            sender,
            receiver,
            stopped: false,
            last_frame_number: 0,
        }
    }

    fn receive_video_tracks(&mut self) {
        while let Ok(video_track) = self.receiver.try_recv() {
            self.video_track = Some(video_track);
            self.needs_idr = true;
        }
    }

    pub fn video_track_setter(&self) -> Sender<Arc<TrackLocalStaticSample>> {
        self.sender.clone()
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
    fn start(&mut self) {
        self.receive_video_tracks();
    }
    fn stop(&mut self) {
        self.stopped = true;
        self.video_track = None;

        // TODO: call the RTC Peer to stop
    }

    fn submit_decode_unit(&mut self, unit: VideoDecodeUnit<'_>) -> DecodeResult {
        if self.stopped {
            return DecodeResult::Ok;
        }

        // Maybe this will help: https://github.com/moonlight-stream/moonlight-android/blob/master/app/src/main/java/com/limelight/binding/video/MediaCodecDecoderRenderer.java#L1397
        self.receive_video_tracks();

        let Some(video_track) = self.video_track.as_ref() else {
            return DecodeResult::Ok;
        };

        for buffer in unit.buffers {
            let prev_dropped_packets = (unit.frame_number - self.last_frame_number) as u16;
            self.last_frame_number = unit.frame_number;

            let mut buffer_data = buffer.data;

            // https://github.com/moonlight-stream/moonlight-android/blob/master/app/src/main/java/com/limelight/binding/video/MediaCodecDecoderRenderer.java#L1473
            // H264 SPS
            // if buffer.ty == BufferType::Sps {
            //     // numSpsIn++; ?

            //     // Skip to the start of the NALU data
            //     let start_sequence_len = if buffer_data[2] == 0x01 { 3 } else { 4 };
            //     buffer_data = &buffer_data[(start_sequence_len + 1)..];
            // }

            // TODO: fill in these values

            let data = Bytes::copy_from_slice(buffer_data);
            let frame_time = self.frame_time;
            let packet_timestamp = unit.frame_number as u32;

            let video_track = video_track.clone();
            self.runtime.spawn(async move {
                video_track
                    .write_sample(&Sample {
                        data,
                        timestamp: SystemTime::now(),
                        duration: Duration::from_secs_f32(frame_time),
                        packet_timestamp,
                        prev_dropped_packets,
                        prev_padding_packets: 0,
                    })
                    .await
                    .unwrap();
            });
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
