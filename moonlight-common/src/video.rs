use bitflags::bitflags;
use moonlight_common_sys::limelight;

use crate::stream::Colorspace;

bitflags! {
    #[derive(Debug, Clone, Copy, Default)]
    pub struct SupportedVideoFormats: u32 {
        const VIDEO_FORMAT_H264 = limelight::VIDEO_FORMAT_H264;          // H.264 High Profile
        const H264_HIGH8_444 = limelight::VIDEO_FORMAT_H264_HIGH8_444;   // H.264 High 4:4:4 8-bit Profile
        const H265 = limelight::VIDEO_FORMAT_H265;                       // HEVC Main Profile
        const H265_MAIN10 = limelight::VIDEO_FORMAT_H265_MAIN10;         // HEVC Main10 Profile
        const H265_REXT8_444 = limelight::VIDEO_FORMAT_H265_REXT8_444;   // HEVC RExt 4:4:4 8-bit Profile
        const H265_REXT10_444 = limelight::VIDEO_FORMAT_H265_REXT10_444; // HEVC RExt 4:4:4 10-bit Profile
        const AV1_MAIN8 = limelight::VIDEO_FORMAT_AV1_MAIN8;             // AV1 Main 8-bit profile
        const AV1_MAIN10 = limelight::VIDEO_FORMAT_AV1_MAIN10;           // AV1 Main 10-bit profile
        const AV1_HIGH8_444 = limelight::VIDEO_FORMAT_AV1_HIGH8_444;     // AV1 High 4:4:4 8-bit profile
        const AV1_HIGH10_444 = limelight::VIDEO_FORMAT_AV1_HIGH10_444;   // AV1 High 4:4:4 10-bit profile
    }
}

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
