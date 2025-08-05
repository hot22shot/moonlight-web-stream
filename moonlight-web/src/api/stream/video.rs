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

fn split_annexb_nals(data: &[u8]) -> Vec<&[u8]> {
    let mut nal_units = Vec::new();
    let mut i = 0;

    while i + 3 < data.len() {
        let start = if &data[i..i + 3] == [0, 0, 1] {
            i
        } else if i + 4 <= data.len() && &data[i..i + 4] == [0, 0, 0, 1] {
            i
        } else {
            i += 1;
            continue;
        };

        let next = (i + 3..data.len())
            .find(|&j| {
                j + 3 < data.len()
                    && (&data[j..j + 3] == [0, 0, 1]
                        || (j + 4 < data.len() && &data[j..j + 4] == [0, 0, 0, 1]))
            })
            .unwrap_or(data.len());

        nal_units.push(&data[start..next]);
        i = next;
    }

    nal_units
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

        self.receive_video_tracks();
        let Some(video_track) = self.video_track.as_ref() else {
            return DecodeResult::Ok;
        };

        let frame_time = self.frame_time;
        let packet_timestamp = unit.frame_number as u32;
        let prev_dropped_packets = (unit.frame_number - self.last_frame_number) as u16;
        self.last_frame_number = unit.frame_number;

        match unit.frame_type {
            FrameType::Idr => {
                let mut full_frame = Vec::new();
                for buffer in unit.buffers {
                    full_frame.extend_from_slice(buffer.data);
                }

                if full_frame.is_empty() {
                    warn!("Frame had no data");
                    return DecodeResult::Ok;
                }

                let data = Bytes::from(full_frame);

                let video_track = video_track.clone();
                self.runtime.spawn(async move {
                    if let Err(err) = video_track
                        .write_sample(&Sample {
                            data,
                            timestamp: SystemTime::now(),
                            duration: Duration::from_secs_f32(frame_time),
                            packet_timestamp,
                            prev_dropped_packets,
                            prev_padding_packets: 0,
                        })
                        .await
                    {
                        warn!("write_sample failed: {err}");
                    }
                });
            }
            FrameType::PFrame => {
                for buffer in unit.buffers {
                    let data = Bytes::copy_from_slice(buffer.data);
                    let video_track = video_track.clone();

                    self.runtime.spawn(async move {
                        if let Err(err) = video_track
                            .write_sample(&Sample {
                                data,
                                timestamp: SystemTime::now(),
                                duration: Duration::from_secs_f32(frame_time),
                                packet_timestamp,
                                prev_dropped_packets,
                                prev_padding_packets: 0,
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
