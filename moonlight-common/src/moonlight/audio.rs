use std::{ffi::c_void, slice, sync::Mutex};

use moonlight_common_sys::limelight::{
    _AUDIO_RENDERER_CALLBACKS, AUDIO_CONFIGURATION_MAX_CHANNEL_COUNT,
    POPUS_MULTISTREAM_CONFIGURATION,
};

use crate::moonlight::stream::Capabilities;

/// This structure provides the Opus multistream decoder parameters required to successfully
/// decode the audio stream being sent from the computer. See opus_multistream_decoder_init docs
/// for details about these fields.
///
/// The supplied mapping array is indexed according to the following output channel order:
/// 0 - Front Left
/// 1 - Front Right
/// 2 - Center
/// 3 - LFE
/// 4 - Back Left
/// 5 - Back Right
/// 6 - Side Left
/// 7 - Side Right
///
/// If the mapping order does not match the channel order of the audio renderer, you may swap
/// the values in the mismatched indices until the mapping array matches the desired channel order.
#[derive(Debug)]
pub struct OpusMultistreamConfig {
    pub sample_rate: u32,
    pub channel_count: u32,
    pub coupled_streams: u32,
    pub samples_per_frame: u32,
    pub mapping: [u8; AUDIO_CONFIGURATION_MAX_CHANNEL_COUNT as usize],
}

#[derive(Debug, Clone, Copy)]
pub struct AudioConfig(pub u32);

impl AudioConfig {
    /// Specifies that the audio stream should be encoded in stereo (default)
    pub const STEREO: AudioConfig = Self::new(2, 0x03);
    /// Specifies that the audio stream should be in 5.1 surround sound if the PC is able
    pub const SURROUND_51: AudioConfig = Self::new(6, 0x3F);
    /// Specifies that the audio stream should be in 7.1 surround sound if the PC is able
    pub const SURROUND_71: AudioConfig = Self::new(8, 0x63F);

    /// Specifies an audio configuration by channel count and channel mask
    /// See https://docs.microsoft.com/en-us/windows-hardware/drivers/audio/channel-mask for channelMask values
    /// NOTE: Not all combinations are supported by GFE and/or this library.
    pub const fn new(channel_count: u32, channel_mask: u32) -> Self {
        Self(channel_mask << 16 | channel_count << 8 | 0xCA)
    }
}

pub trait AudioDecoder {
    /// This callback initializes the audio renderer. The audio configuration parameter
    /// provides the negotiated audio configuration. This may differ from the one
    /// specified in the stream configuration. Returns 0 on success, non-zero on failure.
    fn setup(
        &mut self,
        audio_config: AudioConfig,
        stream_config: OpusMultistreamConfig,
        ar_flags: (),
    ) -> i32;

    /// This callback notifies the decoder that the stream is starting. No audio can be submitted before this callback returns.
    fn start(&mut self);

    /// This callback notifies the decoder that the stream is stopping. Audio samples may still be submitted but they may be safely discarded.
    fn stop(&mut self);

    /// This callback provides Opus audio data to be decoded and played. sampleLength is in bytes.
    fn decode_and_play_sample(&mut self, data: &[u8]);

    fn config(&self) -> AudioConfig;
    fn capabilities(&self) -> Capabilities;
}

static GLOBAL_AUDIO_DECODER: Mutex<Option<Box<dyn AudioDecoder + Send + 'static>>> =
    Mutex::new(None);

fn global_decoder<R>(f: impl FnOnce(&mut dyn AudioDecoder) -> R) -> R {
    let lock = GLOBAL_AUDIO_DECODER.lock();
    let mut lock = lock.expect("global audio decoder");

    let decoder = lock.as_mut().expect("global audio decoder");
    f(decoder.as_mut())
}

pub(crate) fn set_global(decoder: impl AudioDecoder + Send + 'static) {
    let mut global_audio_decoder = GLOBAL_AUDIO_DECODER
        .lock()
        .expect("global audio decoder lock");

    *global_audio_decoder = Some(Box::new(decoder));
}
pub(crate) fn clear_global() {
    let mut decoder = GLOBAL_AUDIO_DECODER.lock().expect("global video decoder");

    *decoder = None;
}

#[allow(non_snake_case)]
unsafe extern "C" fn setup(
    audioConfiguration: i32,
    opusConfig: POPUS_MULTISTREAM_CONFIGURATION,
    _context: *mut c_void,
    _arFlags: i32,
) -> i32 {
    global_decoder(|decoder| {
        let audio_config = AudioConfig(audioConfiguration as u32);

        let raw_opus_config = unsafe { *opusConfig };
        let opus_config = OpusMultistreamConfig {
            sample_rate: raw_opus_config.sampleRate as u32,
            channel_count: raw_opus_config.channelCount as u32,
            coupled_streams: raw_opus_config.coupledStreams as u32,
            samples_per_frame: raw_opus_config.samplesPerFrame as u32,
            mapping: raw_opus_config.mapping,
        };

        decoder.setup(audio_config, opus_config, ())
    })
}
unsafe extern "C" fn start() {
    global_decoder(|decoder| {
        decoder.start();
    })
}

unsafe extern "C" fn decode_and_play_sample(data: *mut i8, len: i32) {
    global_decoder(|decoder| unsafe {
        let data = slice::from_raw_parts(data as *mut u8, len as usize);

        decoder.decode_and_play_sample(data);
    })
}

unsafe extern "C" fn stop() {
    global_decoder(|decoder| {
        decoder.stop();
    })
}

unsafe extern "C" fn cleanup() {
    clear_global();
}

pub(crate) unsafe fn raw_callbacks() -> _AUDIO_RENDERER_CALLBACKS {
    let capabilities = global_decoder(|decoder| decoder.capabilities());

    _AUDIO_RENDERER_CALLBACKS {
        init: Some(setup),
        start: Some(start),
        stop: Some(stop),
        cleanup: Some(cleanup),
        decodeAndPlaySample: Some(decode_and_play_sample),
        capabilities: capabilities.bits() as i32,
    }
}
