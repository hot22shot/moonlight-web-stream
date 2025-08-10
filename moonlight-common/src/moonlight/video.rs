use std::{ffi::c_void, slice, sync::Mutex};

use bitflags::bitflags;
use moonlight_common_sys::limelight::{
    self, _DECODER_RENDERER_CALLBACKS, BUFFER_TYPE_PICDATA, BUFFER_TYPE_PPS, BUFFER_TYPE_SPS,
    BUFFER_TYPE_VPS, DR_NEED_IDR, DR_OK, FRAME_TYPE_IDR, FRAME_TYPE_PFRAME, PDECODE_UNIT,
    VIDEO_FORMAT_MASK_10BIT, VIDEO_FORMAT_MASK_AV1, VIDEO_FORMAT_MASK_H264, VIDEO_FORMAT_MASK_H265,
    VIDEO_FORMAT_MASK_YUV444,
};
use num::FromPrimitive;
use num_derive::FromPrimitive;

use crate::moonlight::stream::{Capabilities, Colorspace};

// TODO: make time values into Duration or other fitting values

bitflags! {
    #[derive(Debug, Clone, Copy, Default)]
    pub struct SupportedVideoFormats: u32 {
        const H264 = limelight::VIDEO_FORMAT_H264;          // H.264 High Profile
        const H264_HIGH8_444 = limelight::VIDEO_FORMAT_H264_HIGH8_444;   // H.264 High 4:4:4 8-bit Profile
        const H265 = limelight::VIDEO_FORMAT_H265;                       // HEVC Main Profile
        const H265_MAIN10 = limelight::VIDEO_FORMAT_H265_MAIN10;         // HEVC Main10 Profile
        const H265_REXT8_444 = limelight::VIDEO_FORMAT_H265_REXT8_444;   // HEVC RExt 4:4:4 8-bit Profile
        const H265_REXT10_444 = limelight::VIDEO_FORMAT_H265_REXT10_444; // HEVC RExt 4:4:4 10-bit Profile
        const AV1_MAIN8 = limelight::VIDEO_FORMAT_AV1_MAIN8;             // AV1 Main 8-bit profile
        const AV1_MAIN10 = limelight::VIDEO_FORMAT_AV1_MAIN10;           // AV1 Main 10-bit profile
        const AV1_HIGH8_444 = limelight::VIDEO_FORMAT_AV1_HIGH8_444;     // AV1 High 4:4:4 8-bit profile
        const AV1_HIGH10_444 = limelight::VIDEO_FORMAT_AV1_HIGH10_444;   // AV1 High 4:4:4 10-bit profile

        // Preconfigured
        const MASK_H264 = VIDEO_FORMAT_MASK_H264;
        const MASK_H265 = VIDEO_FORMAT_MASK_H265;
        const MASK_AV1 = VIDEO_FORMAT_MASK_AV1;
        const MASK_10BIT = VIDEO_FORMAT_MASK_10BIT;
        const MASK_YUV444 = VIDEO_FORMAT_MASK_YUV444;
    }
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, FromPrimitive)]
pub enum VideoFormat {
    H264 = limelight::VIDEO_FORMAT_H264, // H.264 High Profile
    H264High8_444 = limelight::VIDEO_FORMAT_H264_HIGH8_444, // H.264 High 4:4:4 8-bit Profile
    H265 = limelight::VIDEO_FORMAT_H265, // HEVC Main Profile
    H265Main10 = limelight::VIDEO_FORMAT_H265_MAIN10, // HEVC Main10 Profile
    H265Rext8_444 = limelight::VIDEO_FORMAT_H265_REXT8_444, // HEVC RExt 4:4:4 8-bit Profile
    H265Rext10_444 = limelight::VIDEO_FORMAT_H265_REXT10_444, // HEVC RExt 4:4:4 10-bit Profile
    Av1Main8 = limelight::VIDEO_FORMAT_AV1_MAIN8, // AV1 Main 8-bit profile
    Av1Main10 = limelight::VIDEO_FORMAT_AV1_MAIN10, // AV1 Main 10-bit profile
    Av1High8_444 = limelight::VIDEO_FORMAT_AV1_HIGH8_444, // AV1 High 4:4:4 8-bit profile
    Av1High10_444 = limelight::VIDEO_FORMAT_AV1_HIGH10_444, // AV1 High 4:4:4 10-bit profile
}

impl VideoFormat {
    pub fn contained_in(&self, supported_video_formats: SupportedVideoFormats) -> bool {
        let Some(single_format) = SupportedVideoFormats::from_bits(*self as u32) else {
            return false;
        };

        supported_video_formats.contains(single_format)
    }
}

/// These identify codec configuration data in the buffer lists
/// of frames identified as IDR frames for H.264 and HEVC formats.
/// For other codecs, all data is marked as BUFFER_TYPE_PICDATA.
#[repr(u32)]
#[derive(Debug, Clone, Copy, FromPrimitive, PartialEq, Eq)]
pub enum BufferType {
    PicData = BUFFER_TYPE_PICDATA,
    Sps = BUFFER_TYPE_SPS,
    Pps = BUFFER_TYPE_PPS,
    Vps = BUFFER_TYPE_VPS,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, FromPrimitive)]
pub enum FrameType {
    /// This is a standard frame which references the IDR frame and
    /// previous P-frames.
    PFrame = FRAME_TYPE_PFRAME,
    /// This is a key frame.
    ///
    /// For H.264 and HEVC, this means the frame contains SPS, PPS, and VPS (HEVC only) NALUs
    /// as the first buffers in the list. The I-frame data follows immediately
    /// after the codec configuration NALUs.
    ///
    /// For other codecs, any configuration data is not split into separate buffers.
    Idr = FRAME_TYPE_IDR,
}

/// A decode unit describes a buffer chain of video data from multiple packets
pub struct VideoDecodeUnit<'a> {
    /// Frame Number
    pub frame_number: i32,
    /// Frame Type
    pub frame_type: FrameType,
    /// Optional host processing latency of the frame, in 1/10 ms units.
    /// Zero when the host doesn't provide the latency data
    /// or frame processing latency is not applicable to the current frame
    /// (happens when the frame is repeated).
    pub frame_processing_latency: u16,
    /// Receive time of first buffer. This value uses an implementation-defined epoch,
    /// but the same epoch as enqueueTimeMs and LiGetMillis().
    pub receive_time_ms: u64,
    /// Time the frame was fully assembled and queued for the video decoder to process.
    /// This is also approximately the same time as the final packet was received, so
    /// enqueueTimeMs - receiveTimeMs is the time taken to receive the frame. At the
    /// time the decode unit is passed to submitDecodeUnit(), the total queue delay
    /// can be calculated by LiGetMillis() - enqueueTimeMs.
    pub enqueue_time_ms: u64,
    /// Presentation time in milliseconds with the epoch at the first captured frame.
    /// This can be used to aid frame pacing or to drop old frames that were queued too
    /// long prior to display.
    pub presentation_time_ms: u32,
    /// Determines if this frame is SDR or HDR
    ///
    /// Note: This is not currently parsed from the actual bitstream, so if your
    /// client has access to a bitstream parser, prefer that over this field.
    pub hdr_active: bool,
    /// Provides the colorspace of this frame (see COLORSPACE_* defines above)
    ///
    /// Note: This is not currently parsed from the actual bitstream, so if your
    /// client has access to a bitstream parser, prefer that over this field.
    pub color_space: Colorspace,
    pub buffers: &'a [VideoDataBuffer<'a>],
}
pub struct VideoDataBuffer<'a> {
    /// Buffer type (listed above, only set for H.264 and HEVC formats)
    pub ty: BufferType,
    pub data: &'a [u8],
}

#[repr(i32)]
#[derive(Debug, Clone, Copy)]
pub enum DecodeResult {
    Ok = DR_OK as i32,
    NeedIdr = DR_NEED_IDR,
}

pub trait VideoDecoder {
    /// This callback is invoked to provide details about the video stream and allow configuration of the decoder.
    /// Returns 0 on success, non-zero on failure.
    fn setup(
        &mut self,
        format: VideoFormat,
        width: u32,
        height: u32,
        redraw_rate: u32,
        flags: (),
    ) -> i32;

    /// This callback notifies the decoder that the stream is starting. No frames can be submitted before this callback returns.
    fn start(&mut self);

    /// This callback provides Annex B formatted elementary stream data to the
    /// decoder. If the decoder is unable to process the submitted data for some reason,
    /// it must return DR_NEED_IDR to generate a keyframe.
    fn submit_decode_unit(&mut self, unit: VideoDecodeUnit<'_>) -> DecodeResult;

    /// This callback notifies the decoder that the stream is stopping. Frames may still be submitted but they may be safely discarded.
    fn stop(&mut self);

    fn supported_formats(&self) -> SupportedVideoFormats;
    fn capabilities(&self) -> Capabilities;
}

// TODO: Pull based renderers

static GLOBAL_VIDEO_DECODER: Mutex<Option<Box<dyn VideoDecoder + Send + 'static>>> =
    Mutex::new(None);

fn global_decoder<R>(f: impl FnOnce(&mut dyn VideoDecoder) -> R) -> R {
    let lock = GLOBAL_VIDEO_DECODER.lock();
    let mut lock = lock.expect("global video decoder");

    let decoder = lock.as_mut().expect("global video decoder");
    f(decoder.as_mut())
}

pub(crate) fn new_global(decoder: impl VideoDecoder + Send + 'static) -> Result<(), ()> {
    let mut global_video_decoder = GLOBAL_VIDEO_DECODER.lock().map_err(|_| ())?;

    if global_video_decoder.is_some() {
        return Err(());
    }
    *global_video_decoder = Some(Box::new(decoder));

    Ok(())
}
pub(crate) fn clear_global() {
    let mut decoder = GLOBAL_VIDEO_DECODER.lock().expect("global video decoder");

    *decoder = None;
}

#[allow(non_snake_case)]
unsafe extern "C" fn setup(
    videoFormat: i32,
    width: i32,
    height: i32,
    redrawRate: i32,
    _context: *mut c_void,
    _drFlags: i32, // TODO: <--
) -> i32 {
    global_decoder(|decoder| {
        decoder.setup(
            VideoFormat::from_i32(videoFormat).expect("invalid video format"),
            width as u32,
            height as u32,
            redrawRate as u32,
            (), // TODO
        )
    })
}
unsafe extern "C" fn start() {
    global_decoder(|decoder| {
        decoder.start();
    })
}

unsafe extern "C" fn submit_decode_unit(decode_unit: PDECODE_UNIT) -> i32 {
    let raw = unsafe { *decode_unit };

    // TODO: store this vec somewhere so we don't realloc every time
    let mut buffers: Vec<VideoDataBuffer> = Vec::new();

    let mut next_element_ptr = raw.bufferList;
    while !next_element_ptr.is_null() {
        unsafe {
            let element_raw = *next_element_ptr;

            let new_element = VideoDataBuffer {
                ty: BufferType::from_i32(element_raw.bufferType).expect("valid buffer type"),
                data: slice::from_raw_parts(
                    element_raw.data as *const u8,
                    element_raw.length as usize,
                ),
            };
            buffers.push(new_element);

            next_element_ptr = element_raw.next;
        }
    }

    let unit = VideoDecodeUnit {
        frame_number: raw.frameNumber,
        frame_type: FrameType::from_i32(raw.frameType).expect("valid FrameType"),
        frame_processing_latency: raw.frameHostProcessingLatency,
        receive_time_ms: raw.receiveTimeMs,
        enqueue_time_ms: raw.enqueueTimeMs,
        presentation_time_ms: raw.presentationTimeMs,
        color_space: Colorspace::from_u8(raw.colorspace).expect("valid Colorspace"),
        hdr_active: raw.hdrActive,
        buffers: &buffers,
    };

    global_decoder(|decoder| decoder.submit_decode_unit(unit) as i32)
}

unsafe extern "C" fn stop() {
    global_decoder(|decoder| {
        decoder.stop();
    })
}

unsafe extern "C" fn cleanup() {
    clear_global();
}

pub(crate) unsafe fn raw_callbacks() -> _DECODER_RENDERER_CALLBACKS {
    let capabilities = global_decoder(|decoder| decoder.capabilities());

    _DECODER_RENDERER_CALLBACKS {
        setup: Some(setup),
        start: Some(start),
        stop: Some(stop),
        cleanup: Some(cleanup),
        submitDecodeUnit: Some(submit_decode_unit),
        capabilities: capabilities.bits() as i32,
    }
}
