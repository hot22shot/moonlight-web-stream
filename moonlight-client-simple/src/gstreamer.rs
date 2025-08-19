use std::{str::FromStr, thread::spawn, time::Duration};

use audiopus_sys::{
    OPUS_OK, OpusMSDecoder, opus_multistream_decode, opus_multistream_decoder_create,
    opus_multistream_decoder_destroy,
};
use gstreamer::{
    Buffer, BufferFlags, Caps, ClockTime, DebugGraphDetails, Element, ElementFactory, Format,
    Pipeline, State, StreamType, Structure,
    event::Eos,
    glib::{
        self, Value, ValueArray,
        gobject_ffi::{GObject, GValue, GValueArray},
        object::ObjectExt,
    },
    prelude::{ElementExt, ElementExtManual, GstBinExt, GstBinExtManual},
};
use gstreamer_app::{AppSrc, AppStreamType};
use moonlight_common::moonlight::{
    audio::{AudioConfig, AudioDecoder, OpusMultistreamConfig},
    stream::Capabilities,
    video::{
        DecodeResult, FrameType, SupportedVideoFormats, VideoDecodeUnit, VideoDecoder, VideoFormat,
    },
};

pub fn init() {
    gstreamer::init().expect("failed to init gstreamer");
}

pub fn gstreamer_pipeline()
-> Result<(GStreamerVideoHandler, GStreamerAudioHandler), glib::BoolError> {
    let pipeline = Pipeline::new();

    // Video
    let (video_decoder, video_output) = GStreamerVideoHandler::new(pipeline.clone())?;

    let video_sink = ElementFactory::make_with_name("autovideosink", Some("play video"))?;
    video_sink.set_property("sync", false);
    video_sink.set_property("async-handling", true);

    pipeline.add(&video_sink)?;

    video_output.link(&video_sink)?;

    // Audio
    let (audio_decoder, audio_output) = GStreamerAudioHandler::new(pipeline.clone())?;

    let audio_sink = ElementFactory::make_with_name("wasapisink", Some("play audio"))?;
    // audio_sink.set_property("sync", false);
    // audio_sink.set_property("async-handling", true);
    audio_sink.set_property(
        "device",
        "{0.0.0.00000000}.{e1f9c34a-388e-4b8f-a780-a2e2348fe6c3}",
    );

    pipeline.add(&audio_sink)?;

    audio_output.link(&audio_sink)?;

    let dot_data = pipeline.debug_to_dot_data(DebugGraphDetails::all());
    std::fs::write("appimages/pipeline.dot", dot_data).unwrap();

    Ok((video_decoder, audio_decoder))
}

pub struct GStreamerVideoHandler {
    pipeline: Pipeline,
    app_src: AppSrc,
}

impl GStreamerVideoHandler {
    pub fn new(pipeline: Pipeline) -> Result<(Self, Element), glib::BoolError> {
        let app_src = AppSrc::builder().name("video input").build();
        app_src.set_is_live(true);
        app_src.set_format(Format::Buffers);
        app_src.set_block(false);
        app_src.set_do_timestamp(true);
        app_src.set_min_latency(-1);

        let parse = ElementFactory::make_with_name("h265parse", Some("parse packets"))?;
        parse.set_property("config-interval", 0);

        let decode = ElementFactory::make_with_name("avdec_h265", Some("decode video"))?;
        let convert = ElementFactory::make_with_name("videoconvert", Some("convert video"))?;

        pipeline
            .add_many([app_src.as_ref(), &parse, &decode, &convert])
            .unwrap();

        app_src.link(&parse)?;
        parse.link(&decode)?;
        decode.link(&convert)?;

        Ok((Self { pipeline, app_src }, convert))
    }
}

impl VideoDecoder for GStreamerVideoHandler {
    fn setup(
        &mut self,
        format: VideoFormat,
        width: u32,
        height: u32,
        redraw_rate: u32,
        flags: (),
    ) -> i32 {
        let _ = (format, width, height, redraw_rate, flags);
        0
    }

    fn start(&mut self) {
        self.pipeline.set_state(State::Playing).unwrap();
    }
    fn stop(&mut self) {
        self.pipeline.send_event(Eos::new());
        self.pipeline.set_state(State::Null).unwrap();
    }

    fn submit_decode_unit(&mut self, unit: VideoDecodeUnit<'_>) -> DecodeResult {
        if matches!(self.pipeline.current_state(), State::Null) {
            return DecodeResult::Ok;
        }

        for buffer in unit.buffers {
            let mut gst_buffer = Buffer::with_size(buffer.data.len()).unwrap();
            {
                let buffer_mut = gst_buffer.get_mut().unwrap();

                buffer_mut.copy_from_slice(0, buffer.data).unwrap();

                let pts_ns = unit.presentation_time_ms as u64 * 1_000_000;
                buffer_mut.set_pts(ClockTime::from_nseconds(pts_ns));
                buffer_mut.set_dts(ClockTime::from_nseconds(pts_ns));

                match unit.frame_type {
                    FrameType::Idr => {
                        // Keyframe (contains SPS/PPS/VPS + I-frame)
                        buffer_mut.set_flags(BufferFlags::empty());
                    }
                    FrameType::PFrame => {
                        // Predictive frame
                        buffer_mut.set_flags(BufferFlags::DELTA_UNIT);
                    }
                }
            }

            self.app_src.push_buffer(gst_buffer).unwrap();
        }

        DecodeResult::Ok
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::empty()
    }
    fn supported_formats(&self) -> SupportedVideoFormats {
        SupportedVideoFormats::H265
    }
}

pub struct GStreamerAudioHandler {
    pipeline: Pipeline,
    app_src: AppSrc,
    decoder: Option<OpusMultistreamDecoder>,
    buffer: Vec<i16>,
    frame_size: usize,
}

impl GStreamerAudioHandler {
    pub fn new(pipeline: Pipeline) -> Result<(Self, Element), glib::BoolError> {
        let app_src = AppSrc::builder().name("audio_input").build();
        app_src.set_is_live(true);
        app_src.set_stream_type(AppStreamType::Stream);
        app_src.set_format(Format::Time);
        app_src.set_block(true);
        app_src.set_do_timestamp(true);
        {
            let caps = Caps::from_str("audio/x-raw,format=S16LE,rate=48000,channels=2").unwrap();
            app_src.set_caps(Some(&caps));
        }

        // let audioparse = ElementFactory::make_with_name("opusparse", Some("audio parse")).unwrap();
        // let audiodec = ElementFactory::make_with_name("opusdec", Some("audio decode")).unwrap();
        // spawn({
        //     let audiodec = audiodec.clone();
        //     move || loop {
        //         std::thread::sleep(Duration::from_secs(1));
        //         println!("{:?}", audiodec.property::<Structure>("stats"))
        //     }
        // });

        let audioconvert =
            ElementFactory::make_with_name("audioconvert", Some("audio convert")).unwrap();
        let audioresample =
            ElementFactory::make_with_name("audioresample", Some("audio resample")).unwrap();

        pipeline.add_many([
            app_src.as_ref(),
            // &audioparse,
            // &audiodec,
            &audioconvert,
            &audioresample,
        ])?;

        Element::link_many([
            app_src.as_ref(),
            // &audioparse,
            // &audiodec,
            &audioconvert,
            &audioresample,
        ])?;

        Ok((
            Self {
                pipeline,
                app_src,
                decoder: None,
                buffer: Vec::new(),
                frame_size: 0,
            },
            audioresample,
        ))
    }
}

fn i16_slice_to_u8_slice(vec: &[i16]) -> &[u8] {
    unsafe { std::slice::from_raw_parts(vec.as_ptr() as *const u8, std::mem::size_of_val(vec)) }
}

impl AudioDecoder for GStreamerAudioHandler {
    fn setup(
        &mut self,
        audio_config: AudioConfig,
        stream_config: OpusMultistreamConfig,
        ar_flags: (),
    ) -> i32 {
        println!("Stream Config: {:?}", stream_config);
        // self.audio_config = Some(audio_config);

        self.buffer =
            vec![0; (stream_config.channel_count * stream_config.samples_per_frame) as usize];
        self.frame_size = stream_config.samples_per_frame as usize;
        self.decoder = Some(OpusMultistreamDecoder::new(stream_config));

        0
    }

    fn start(&mut self) {
        self.pipeline.set_state(State::Playing).unwrap();
    }

    fn stop(&mut self) {
        self.pipeline.send_event(Eos::new());
        self.pipeline.set_state(State::Null).unwrap();
    }

    fn decode_and_play_sample(&mut self, data: &[u8]) {
        let Some(decoder) = self.decoder.as_mut() else {
            return;
        };
        // Check for empty or obviously invalid packets
        if data.is_empty() {
            eprintln!("Received empty packet, skipping");
            return;
        }

        // Optional: sanity check for packet size (Opus packets have max 1275 bytes per frame per RFC)
        if data.len() > 1500 {
            eprintln!(
                "Received suspiciously large packet ({} bytes), skipping",
                data.len()
            );
            return;
        }

        // Decode
        let decode_len = decoder.decode(data, &mut self.buffer, self.frame_size, false);

        if decode_len == 0 {
            eprintln!(
                "Opus decode failed: invalid packet or multistream config mismatch. \
                packet_len={}, frame_size={}, channels={}",
                data.len(),
                self.frame_size,
                decoder.channels
            );
            return;
        }

        let data = i16_slice_to_u8_slice(&self.buffer);

        let mut buffer = Buffer::with_size(data.len()).unwrap();
        let buffer_mut = buffer.get_mut().unwrap();

        let _ = buffer_mut.copy_from_slice(0, data);
        let _ = self.app_src.push_buffer(buffer);
    }

    fn config(&self) -> AudioConfig {
        AudioConfig::STEREO
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::empty()
    }
}

struct OpusMultistreamDecoder {
    channels: usize,
    inner: *mut OpusMSDecoder,
}

unsafe impl Send for OpusMultistreamDecoder {}
unsafe impl Sync for OpusMultistreamDecoder {}

impl OpusMultistreamDecoder {
    pub fn new(config: OpusMultistreamConfig) -> Self {
        unsafe {
            let mut error = OPUS_OK;

            let inner = opus_multistream_decoder_create(
                config.sample_rate as i32,
                config.channel_count as i32,
                config.streams as i32,
                config.coupled_streams as i32,
                config.mapping.as_ptr(),
                &mut error,
            );
            if inner.is_null() || error != OPUS_OK {
                panic!("Failed to create opus multistream decoder");
            }

            Self {
                inner,
                channels: config.channel_count as usize,
            }
        }
    }

    pub fn decode(
        &self,
        input: &[u8],
        output: &mut [i16],
        frame_size: usize,
        decode_fen: bool,
    ) -> usize {
        if frame_size * self.channels > output.len() {
            panic!("buffer too small");
        }

        unsafe {
            let ret = opus_multistream_decode(
                self.inner,
                input.as_ptr(),
                input.len() as i32,
                output.as_mut_ptr(),
                frame_size as i32,
                if decode_fen { 1 } else { 0 },
            );

            if ret < 0 {
                println!("invalid buffer: {}", ret);
                return 0;
            }

            ret as usize
        }
    }
}
impl Drop for OpusMultistreamDecoder {
    fn drop(&mut self) {
        unsafe {
            opus_multistream_decoder_destroy(self.inner);
        }
    }
}
