use gstreamer::{
    Buffer, BufferFlags, ClockTime, ElementFactory, Format, Pipeline, State,
    event::Eos,
    glib::{self, object::ObjectExt},
    prelude::{ElementExt, ElementExtManual, GstBinExtManual},
};
use gstreamer_app::AppSrc;
use moonlight_common::{
    stream::Capabilities,
    video::{
        DecodeResult, FrameType, SupportedVideoFormats, VideoDecodeUnit, VideoDecoder, VideoFormat,
    },
};

pub fn init() {
    gstreamer::init().expect("failed to init gstreamer");
}

pub struct GStreamerVideoHandler {
    pipeline: Pipeline,
    app_src: AppSrc,
}

impl GStreamerVideoHandler {
    pub fn new() -> Result<Self, glib::BoolError> {
        let pipeline = Pipeline::new();

        let app_src = AppSrc::builder().name("moonlight packets").build();
        app_src.set_is_live(true);
        app_src.set_format(Format::Buffers);
        app_src.set_block(false);
        app_src.set_do_timestamp(true);
        app_src.set_min_latency(-1);

        let parse = ElementFactory::make_with_name("h265parse", Some("parse packets"))?;
        parse.set_property("config-interval", 0);

        let decode = ElementFactory::make_with_name("avdec_h265", Some("decode video"))?;
        let convert = ElementFactory::make_with_name("videoconvert", Some("convert video"))?;

        let sink = ElementFactory::make_with_name("autovideosink", Some("play video"))?;
        sink.set_property("sync", false);
        sink.set_property("async-handling", true);

        pipeline
            .add_many([app_src.as_ref(), &parse, &decode, &convert, &sink])
            .unwrap();

        app_src.link(&parse).unwrap();
        parse.link(&decode).unwrap();
        decode.link(&convert).unwrap();
        convert.link(&sink).unwrap();

        Ok(Self { pipeline, app_src })
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
