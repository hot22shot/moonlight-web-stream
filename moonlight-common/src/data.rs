use std::ffi::CStr;

use moonlight_common_sys::limelight::{
    COLOR_RANGE_FULL, COLOR_RANGE_LIMITED, COLORSPACE_REC_601, COLORSPACE_REC_709,
    COLORSPACE_REC_2020, ENCFLG_AUDIO, ENCFLG_NONE, ENCFLG_VIDEO, LI_TOUCH_EVENT_BUTTON_ONLY,
    LI_TOUCH_EVENT_CANCEL, LI_TOUCH_EVENT_CANCEL_ALL, LI_TOUCH_EVENT_DOWN, LI_TOUCH_EVENT_HOVER,
    LI_TOUCH_EVENT_HOVER_LEAVE, LI_TOUCH_EVENT_MOVE, LI_TOUCH_EVENT_UP, LiGetStageName,
    STAGE_AUDIO_STREAM_INIT, STAGE_AUDIO_STREAM_START, STAGE_CONTROL_STREAM_INIT,
    STAGE_CONTROL_STREAM_START, STAGE_INPUT_STREAM_INIT, STAGE_INPUT_STREAM_START, STAGE_MAX,
    STAGE_NAME_RESOLUTION, STAGE_NONE, STAGE_PLATFORM_INIT, STAGE_RTSP_HANDSHAKE,
    STAGE_VIDEO_STREAM_INIT, STAGE_VIDEO_STREAM_START, STREAM_CFG_AUTO, STREAM_CFG_LOCAL,
    STREAM_CFG_REMOTE, VIDEO_FORMAT_AV1_HIGH8_444, VIDEO_FORMAT_AV1_HIGH10_444,
    VIDEO_FORMAT_AV1_MAIN8, VIDEO_FORMAT_AV1_MAIN10, VIDEO_FORMAT_H264,
    VIDEO_FORMAT_H264_HIGH8_444, VIDEO_FORMAT_H265, VIDEO_FORMAT_H265_MAIN10,
    VIDEO_FORMAT_H265_REXT8_444, VIDEO_FORMAT_H265_REXT10_444,
};

use crate::{flag_if, network::ServerVersion};

pub struct ServerInfo<'a> {
    pub address: &'a str,
    pub app_version: ServerVersion,
    pub gfe_version: &'a str,
    pub rtsp_session_url: &'a str,
    // TODO: enum?
    pub server_codec_mode_support: i32,
}

#[derive(Debug, Clone, Copy)]
pub enum StreamingConfig {
    Local = STREAM_CFG_LOCAL as isize,
    Remote = STREAM_CFG_REMOTE as isize,
    Auto = STREAM_CFG_AUTO as isize,
}

impl StreamingConfig {
    pub(crate) fn raw(self) -> i32 {
        self as i32
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Colorspace {
    Rec601 = COLORSPACE_REC_601 as isize,
    Rec709 = COLORSPACE_REC_709 as isize,
    Rec2020 = COLORSPACE_REC_2020 as isize,
}

impl Colorspace {
    pub(crate) fn raw(self) -> i32 {
        self as i32
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ColorRange {
    Limited = COLOR_RANGE_LIMITED as isize,
    Full = COLOR_RANGE_FULL as isize,
}

impl ColorRange {
    pub(crate) fn raw(self) -> i32 {
        self as i32
    }
}

#[derive(Debug, Clone, Copy)]
pub struct EncryptionFlags {
    pub audio: bool,
    pub video: bool,
}

impl EncryptionFlags {
    pub fn all() -> Self {
        Self {
            audio: true,
            video: true,
        }
    }
    pub fn video() -> Self {
        Self {
            audio: false,
            video: true,
        }
    }
    pub fn audio() -> Self {
        Self {
            audio: true,
            video: false,
        }
    }
    pub fn none() -> Self {
        Self {
            audio: false,
            video: false,
        }
    }

    pub(crate) fn raw(self) -> i32 {
        let mut flags = ENCFLG_NONE;

        flag_if(&mut flags, ENCFLG_AUDIO, self.audio);
        flag_if(&mut flags, ENCFLG_VIDEO, self.video);

        flags as i32
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SupportedVideoFormats {
    pub h264: bool,            // H.264 High Profile
    pub h264_high8_444: bool,  // H.264 High 4:4:4 8-bit Profile
    pub h265: bool,            // HEVC Main Profile
    pub h265_main10: bool,     // HEVC Main10 Profile
    pub h265_rext8_444: bool,  // HEVC RExt 4:4:4 8-bit Profile
    pub h265_rext10_444: bool, // HEVC RExt 4:4:4 10-bit Profile
    pub av1_main8: bool,       // AV1 Main 8-bit profile
    pub av1_main10: bool,      // AV1 Main 10-bit profile
    pub av1_high8_444: bool,   // AV1 High 4:4:4 8-bit profile
    pub av1_high10_444: bool,  // AV1 High 4:4:4 10-bit profile
}

impl SupportedVideoFormats {
    pub fn all() -> Self {
        Self {
            h264: true,
            h264_high8_444: true,
            h265: true,
            h265_main10: true,
            h265_rext8_444: true,
            h265_rext10_444: true,
            av1_main8: true,
            av1_main10: true,
            av1_high8_444: true,
            av1_high10_444: true,
        }
    }

    pub(crate) fn raw(self) -> i32 {
        let mut flags = 0x0;

        flag_if(&mut flags, VIDEO_FORMAT_H264, self.h264);
        flag_if(&mut flags, VIDEO_FORMAT_H264_HIGH8_444, self.h264_high8_444);
        flag_if(&mut flags, VIDEO_FORMAT_H265, self.h265);
        flag_if(&mut flags, VIDEO_FORMAT_H265_MAIN10, self.h265_main10);
        flag_if(&mut flags, VIDEO_FORMAT_H265_REXT8_444, self.h265_rext8_444);
        #[rustfmt::skip]
        flag_if(&mut flags, VIDEO_FORMAT_H265_REXT10_444, self.h265_rext10_444);
        flag_if(&mut flags, VIDEO_FORMAT_AV1_MAIN8, self.av1_main8);
        flag_if(&mut flags, VIDEO_FORMAT_AV1_MAIN10, self.av1_main10);
        flag_if(&mut flags, VIDEO_FORMAT_AV1_HIGH8_444, self.av1_high8_444);
        flag_if(&mut flags, VIDEO_FORMAT_AV1_HIGH10_444, self.av1_high10_444);

        flags as i32
    }
}

pub struct StreamConfiguration {
    pub width: i32,
    pub height: i32,
    pub fps: i32,
    pub bitrate: i32,
    pub packet_size: i32,
    pub streaming_remotely: StreamingConfig,
    pub audio_configuration: i32,
    pub supported_video_formats: SupportedVideoFormats,
    pub client_refresh_rate_x100: i32,
    pub color_space: Colorspace,
    pub color_range: ColorRange,
    pub encryption_flags: EncryptionFlags,
    pub remote_input_aes_key: [u8; 16usize],
    pub remote_input_aes_iv: [u8; 16usize],
}

#[derive(Debug, Clone, Copy)]
pub enum Stage {
    None = STAGE_NONE as isize,
    PlatformInit = STAGE_PLATFORM_INIT as isize,
    NameResolution = STAGE_NAME_RESOLUTION as isize,
    AudioStreamInit = STAGE_AUDIO_STREAM_INIT as isize,
    RtspHandshake = STAGE_RTSP_HANDSHAKE as isize,
    ControlStreamInit = STAGE_CONTROL_STREAM_INIT as isize,
    VideoStreamInit = STAGE_VIDEO_STREAM_INIT as isize,
    InputStreamInit = STAGE_INPUT_STREAM_INIT as isize,
    ControlStreamStart = STAGE_CONTROL_STREAM_START as isize,
    VideoStreamStart = STAGE_VIDEO_STREAM_START as isize,
    AudioStreamStart = STAGE_AUDIO_STREAM_START as isize,
    InputStreamStart = STAGE_INPUT_STREAM_START as isize,
    Max = STAGE_MAX as isize,
}

impl Stage {
    pub(crate) fn raw(self) -> i32 {
        self as i32
    }

    pub fn name(&self) -> &str {
        unsafe {
            let raw_c_str = LiGetStageName(self.raw());
            let c_str = CStr::from_ptr(raw_c_str);
            c_str.to_str().expect("convert stage name into utf8")
        }
    }
}

pub enum TouchEventType {
    Hover = LI_TOUCH_EVENT_HOVER as isize,
    Down = LI_TOUCH_EVENT_DOWN as isize,
    Up = LI_TOUCH_EVENT_UP as isize,
    Move = LI_TOUCH_EVENT_MOVE as isize,
    Cancel = LI_TOUCH_EVENT_CANCEL as isize,
    ButtonOnly = LI_TOUCH_EVENT_BUTTON_ONLY as isize,
    HoverLeave = LI_TOUCH_EVENT_HOVER_LEAVE as isize,
    CancelAll = LI_TOUCH_EVENT_CANCEL_ALL as isize,
}

impl TouchEventType {
    pub(crate) fn raw(self) -> u8 {
        self as i32 as u8
    }
}
