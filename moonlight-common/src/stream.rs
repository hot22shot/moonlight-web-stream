use std::{ffi::CString, mem::transmute, ptr::null_mut, str::FromStr, sync::Arc};

use moonlight_common_sys::limelight::{
    _SERVER_INFORMATION, _STREAM_CONFIGURATION, LI_ERR_UNSUPPORTED, LI_FF_CONTROLLER_TOUCH_EVENTS,
    LI_FF_PEN_TOUCH_EVENTS, LI_ROT_UNKNOWN, LiGetEstimatedRttInfo, LiGetHostFeatureFlags,
    LiInitializeAudioCallbacks, LiInitializeConnectionCallbacks, LiInitializeVideoCallbacks,
    LiSendMouseMoveAsMousePositionEvent, LiSendMouseMoveEvent, LiSendMousePositionEvent,
    LiSendTouchEvent, LiStartConnection, LiStopConnection, PSERVER_INFORMATION,
    PSTREAM_CONFIGURATION,
};

use crate::{
    Error, Handle,
    data::{ServerInfo, StreamConfiguration, TouchEventType},
};

#[derive(Debug, Clone)]
pub struct HostFeatures {
    pub pen_touch_events: bool,
    pub controller_touch_events: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct EstimatedRttInfo {
    pub rtt: u32,
    pub rtt_variance: u32,
}

pub struct MoonlightStream {
    handle: Arc<Handle>,
}

impl MoonlightStream {
    pub(crate) fn start(
        handle: Arc<Handle>,
        server_info: ServerInfo,
        stream_config: StreamConfiguration,
    ) -> Result<Self, Error> {
        unsafe {
            let mut connection_guard = handle
                .connection_exists
                .lock()
                .expect("connection lock poisoned");
            if *connection_guard {
                return Err(Error::ConnectionAlreadyExists);
            }

            let address = CString::from_str(server_info.address)?;
            let app_version = server_info.app_version.to_string();
            let app_version = CString::from_str(&app_version)?;
            let gfe_version = CString::from_str(server_info.gfe_version)?;
            let rtsp_session_url = CString::from_str(server_info.rtsp_session_url)?;

            let mut server_info_raw = _SERVER_INFORMATION {
                address: address.as_ptr(),
                serverInfoAppVersion: app_version.as_ptr(),
                serverInfoGfeVersion: gfe_version.as_ptr(),
                rtspSessionUrl: rtsp_session_url.as_ptr(),
                serverCodecModeSupport: server_info.server_codec_mode_support,
            };

            let mut stream_config = _STREAM_CONFIGURATION {
                width: stream_config.width,
                height: stream_config.height,
                fps: stream_config.fps,
                bitrate: stream_config.bitrate,
                packetSize: stream_config.packet_size,
                streamingRemotely: stream_config.streaming_remotely.raw(),
                audioConfiguration: stream_config.audio_configuration,
                supportedVideoFormats: stream_config.supported_video_formats.raw(),
                clientRefreshRateX100: stream_config.client_refresh_rate_x100,
                colorSpace: stream_config.color_space.raw(),
                colorRange: stream_config.color_range.raw(),
                encryptionFlags: stream_config.encryption_flags.raw(),
                remoteInputAesKey: transmute::<[u8; 16], [i8; 16]>(
                    stream_config.remote_input_aes_key,
                ),
                remoteInputAesIv: transmute::<[u8; 16], [i8; 16]>(
                    stream_config.remote_input_aes_iv,
                ),
            };

            // # Safety
            // LiStartConnection is not thread safe so we are using the connection_guard mutex
            let result = LiStartConnection(
                &mut server_info_raw as PSERVER_INFORMATION,
                &mut stream_config as PSTREAM_CONFIGURATION,
                null_mut(),
                null_mut(),
                null_mut(),
                null_mut(),
                0,
                null_mut(),
                0,
            );

            if result != 0 {
                todo!()
            }

            *connection_guard = true;

            drop(connection_guard);

            Ok(Self { handle })
        }
    }

    pub fn host_features(&self) -> HostFeatures {
        let features = unsafe { LiGetHostFeatureFlags() };

        let pen_touch_events = (features & LI_FF_PEN_TOUCH_EVENTS) != 0;
        let controller_touch_events = (features & LI_FF_CONTROLLER_TOUCH_EVENTS) != 0;

        HostFeatures {
            pen_touch_events,
            controller_touch_events,
        }
    }

    pub fn estimated_rtt_info(&self) -> Result<EstimatedRttInfo, Error> {
        // TODO: look if we're connected on fail
        unsafe {
            let mut output = EstimatedRttInfo {
                rtt: 0,
                rtt_variance: 0,
            };

            if !LiGetEstimatedRttInfo(
                &mut output.rtt as *mut _,
                &mut output.rtt_variance as *mut _,
            ) {
                return Err(Error::ENetRequired);
            }

            Ok(output)
        }
    }

    // TODO: what are the ints for return values?
    fn send_event_error(error: i32) -> Option<Error> {
        match error {
            0 => None,
            LI_ERR_UNSUPPORTED => Some(Error::NotSupportedOnHost),
            _ => Some(Error::EventSendError),
        }
    }

    pub fn send_mouse_move(&self, delta_x: i16, delta_y: i16) -> Result<(), Error> {
        unsafe {
            if let Some(err) = Self::send_event_error(LiSendMouseMoveEvent(delta_x, delta_y)) {
                return Err(err);
            }
        }
        Ok(())
    }
    pub fn send_mouse_position(
        &self,
        absolute_x: i16,
        absolute_y: i16,
        reference_width: i16,
        reference_height: i16,
    ) -> Result<(), Error> {
        unsafe {
            if let Some(err) = Self::send_event_error(LiSendMousePositionEvent(
                absolute_x,
                absolute_y,
                reference_width,
                reference_height,
            )) {
                return Err(err);
            }
        }
        Ok(())
    }
    pub fn send_mouse_move_as_position(
        &self,
        delta_x: i16,
        delta_y: i16,
        reference_width: i16,
        reference_height: i16,
    ) -> Result<(), Error> {
        unsafe {
            if let Some(err) = Self::send_event_error(LiSendMouseMoveAsMousePositionEvent(
                delta_x,
                delta_y,
                reference_width,
                reference_height,
            )) {
                return Err(err);
            }
        }
        Ok(())
    }

    /// This function allows multi-touch input to be sent directly to Sunshine hosts. The x and y values
    /// are normalized device coordinates stretching top-left corner (0.0, 0.0) to bottom-right corner
    /// (1.0, 1.0) of the video area.
    ///
    /// Pointer ID is an opaque ID that must uniquely identify each active touch on screen. It must
    /// remain constant through any down/up/move/cancel events involved in a single touch interaction.
    ///
    /// Rotation is in degrees from vertical in Y dimension (parallel to screen, 0..360). If rotation is
    /// unknown, pass LI_ROT_UNKNOWN.
    ///
    /// Pressure is a 0.0 to 1.0 range value from min to max pressure. Sending a down/move event with
    /// a pressure of 0.0 indicates the actual pressure is unknown.
    ///
    /// For hover events, the pressure value is treated as a 1.0 to 0.0 range of distance from the touch
    /// surface where 1.0 is the farthest measurable distance and 0.0 is actually touching the display
    /// (which is invalid for a hover event). Reporting distance 0.0 for a hover event indicates the
    /// actual distance is unknown.
    ///
    /// Contact area is modelled as an ellipse with major and minor axis values in normalized device
    /// coordinates. If contact area is unknown, report 0.0 for both contact area axis parameters.
    /// For circular contact areas or if a minor axis value is not available, pass the same value
    /// for major and minor axes. For APIs or devices, that don't report contact area as an ellipse,
    /// approximations can be used such as: https://docs.kernel.org/input/multi-touch-protocol.html#event-computation
    ///
    /// For hover events, the "contact area" is the size of the hovering finger/tool. If unavailable,
    /// pass 0.0 for both contact area parameters.
    ///
    /// Touches can be cancelled using LI_TOUCH_EVENT_CANCEL or LI_TOUCH_EVENT_CANCEL_ALL. When using
    /// LI_TOUCH_EVENT_CANCEL, only the pointerId parameter is valid. All other parameters are ignored.
    /// To cancel all active touches (on focus loss, for example), use LI_TOUCH_EVENT_CANCEL_ALL.
    ///
    /// If unsupported by the host, this will return LI_ERR_UNSUPPORTED and the caller should consider
    /// falling back to other functions to send this input (such as LiSendMousePositionEvent()).
    ///
    /// To determine if LiSendTouchEvent() is supported without calling it, call LiGetHostFeatureFlags()
    /// and check for the LI_FF_PEN_TOUCH_EVENTS flag.
    pub fn send_touch(
        pointer_id: u32,
        x: f32,
        y: f32,
        pressure_or_distance: f32,
        contact_area_major: f32,
        contact_area_minor: f32,
        rotation: Option<u16>,
        event_type: TouchEventType,
    ) -> Result<(), Error> {
        unsafe {
            if let Some(err) = Self::send_event_error(LiSendTouchEvent(
                event_type.raw(),
                pointer_id,
                x,
                y,
                pressure_or_distance,
                contact_area_major,
                contact_area_minor,
                rotation.unwrap_or(LI_ROT_UNKNOWN as u16),
            )) {
                return Err(err);
            }
        }
        Ok(())
    }

    pub fn stop(self) {
        drop(self);
    }
}

impl Drop for MoonlightStream {
    fn drop(&mut self) {
        unsafe {
            // # Safety
            // LiStopConnection is not thread safe so we need a mutex
            let mut connection_guard = self
                .handle
                .connection_exists
                .lock()
                .expect("connection lock poisoned");

            LiStopConnection();

            *connection_guard = false;

            drop(connection_guard);

            // Null out all the callbacks
            LiInitializeAudioCallbacks(null_mut());
            LiInitializeVideoCallbacks(null_mut());
            LiInitializeConnectionCallbacks(null_mut());
        }
    }
}
