use std::ffi::CStr;

use moonlight_common_sys::limelight::{
    LiGetStageName, STAGE_AUDIO_STREAM_INIT, STAGE_AUDIO_STREAM_START, STAGE_CONTROL_STREAM_INIT,
    STAGE_CONTROL_STREAM_START, STAGE_INPUT_STREAM_INIT, STAGE_INPUT_STREAM_START, STAGE_MAX,
    STAGE_NAME_RESOLUTION, STAGE_NONE, STAGE_PLATFORM_INIT, STAGE_RTSP_HANDSHAKE,
    STAGE_VIDEO_STREAM_INIT, STAGE_VIDEO_STREAM_START,
};

#[repr(i32)]
#[derive(Debug, Clone, Copy)]
pub enum Stage {
    None = STAGE_NONE as i32,
    PlatformInit = STAGE_PLATFORM_INIT as i32,
    NameResolution = STAGE_NAME_RESOLUTION as i32,
    AudioStreamInit = STAGE_AUDIO_STREAM_INIT as i32,
    RtspHandshake = STAGE_RTSP_HANDSHAKE as i32,
    ControlStreamInit = STAGE_CONTROL_STREAM_INIT as i32,
    VideoStreamInit = STAGE_VIDEO_STREAM_INIT as i32,
    InputStreamInit = STAGE_INPUT_STREAM_INIT as i32,
    ControlStreamStart = STAGE_CONTROL_STREAM_START as i32,
    VideoStreamStart = STAGE_VIDEO_STREAM_START as i32,
    AudioStreamStart = STAGE_AUDIO_STREAM_START as i32,
    InputStreamStart = STAGE_INPUT_STREAM_START as i32,
    Max = STAGE_MAX as i32,
}

impl Stage {
    pub fn name(&self) -> &str {
        unsafe {
            let raw_c_str = LiGetStageName(*self as i32);
            let c_str = CStr::from_ptr(raw_c_str);
            c_str.to_str().expect("convert stage name into utf8")
        }
    }
}
