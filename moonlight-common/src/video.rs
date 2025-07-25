use crate::data::{Colorspace, SupportedVideoFormats};

pub struct VideoDecodeUnit {
    frame_number: i32,
    frame_type: i32,
    frame_processing_latency: u16,
    receive_time_ms: u64,
    enqueue_time_ms: u64,
    presentation_time_ms: u32,
    hdr_active: bool,
    color_space: Colorspace,
    // TODO: buffer chain: https://github.com/moonlight-stream/moonlight-common-c/blob/master/src/Limelight.h#L177
}

pub trait VideoHandler {
    fn setup(format: (), width: u32, height: u32, redraw_rate: u32, flags: ());

    fn start();
    fn submit_decode_unit();
    fn stop();

    fn supported_formats(&self) -> SupportedVideoFormats;
    fn capabilities(&self);
}

pub(crate) unsafe fn create_video_callbacks(handler: impl VideoHandler) {
    todo!()
}
