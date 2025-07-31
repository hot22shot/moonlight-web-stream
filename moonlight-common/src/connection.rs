use std::{ffi::CStr, sync::Mutex};

use bitflags::bitflags;
use moonlight_common_sys::limelight::{
    _CONNECTION_LISTENER_CALLBACKS, CONN_STATUS_OKAY, CONN_STATUS_POOR, DS_EFFECT_LEFT_TRIGGER,
    DS_EFFECT_PAYLOAD_SIZE, DS_EFFECT_RIGHT_TRIGGER, LiGetStageName, STAGE_AUDIO_STREAM_INIT,
    STAGE_AUDIO_STREAM_START, STAGE_CONTROL_STREAM_INIT, STAGE_CONTROL_STREAM_START,
    STAGE_INPUT_STREAM_INIT, STAGE_INPUT_STREAM_START, STAGE_MAX, STAGE_NAME_RESOLUTION,
    STAGE_NONE, STAGE_PLATFORM_INIT, STAGE_RTSP_HANDSHAKE, STAGE_VIDEO_STREAM_INIT,
    STAGE_VIDEO_STREAM_START,
};
use num::FromPrimitive;
use num_derive::FromPrimitive;

#[repr(u32)]
#[derive(Debug, Clone, Copy, FromPrimitive)]
pub enum Stage {
    None = STAGE_NONE,
    PlatformInit = STAGE_PLATFORM_INIT,
    NameResolution = STAGE_NAME_RESOLUTION,
    AudioStreamInit = STAGE_AUDIO_STREAM_INIT,
    RtspHandshake = STAGE_RTSP_HANDSHAKE,
    ControlStreamInit = STAGE_CONTROL_STREAM_INIT,
    VideoStreamInit = STAGE_VIDEO_STREAM_INIT,
    InputStreamInit = STAGE_INPUT_STREAM_INIT,
    ControlStreamStart = STAGE_CONTROL_STREAM_START,
    VideoStreamStart = STAGE_VIDEO_STREAM_START,
    AudioStreamStart = STAGE_AUDIO_STREAM_START,
    InputStreamStart = STAGE_INPUT_STREAM_START,
    Max = STAGE_MAX,
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

#[repr(u32)]
#[derive(Debug, Clone, Copy, FromPrimitive)]
pub enum ConnectionStatus {
    Ok = CONN_STATUS_OKAY,
    Poor = CONN_STATUS_POOR,
}

// TODO: what is this used for: set_adaptive_triggers
bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub struct DualSenseEffect: u32 {
        const PAYLOAD_SIZE = DS_EFFECT_PAYLOAD_SIZE;
        const RIGHT_TRIGGER = DS_EFFECT_RIGHT_TRIGGER;
        const LEFT_TRIGGER = DS_EFFECT_LEFT_TRIGGER;
    }
}

pub trait ConnectionListener {
    /// This callback is invoked to indicate that a stage of initialization is about to begin
    fn stage_starting(&mut self, stage: Stage);
    /// This callback is invoked to indicate that a stage of initialization has completed
    fn stage_complete(&mut self, stage: Stage);

    /// This callback is invoked to indicate that a stage of initialization has failed.
    /// ConnListenerConnectionTerminated() will not be invoked because the connection was
    /// not yet fully established. LiInterruptConnection() and LiStopConnection() may
    /// result in this callback being invoked, but it is not guaranteed.
    fn stage_failed(&mut self, stage: Stage, error_code: i32);

    /// This callback is invoked after the connection is successfully established
    fn connection_started(&mut self);

    /// This callback is invoked when a connection is terminated after establishment.
    /// The errorCode will be 0 if the termination was reported to be intentional
    /// from the server (for example, the user closed the game). If errorCode is
    /// non-zero, it means the termination was probably unexpected (loss of network,
    /// crash, or similar conditions). This will not be invoked as a result of a call
    /// to LiStopConnection() or LiInterruptConnection().
    fn connection_terminated(&mut self, error_code: i32);

    /// This callback is invoked to log debug message
    fn log_message(&mut self, message: &str) {
        let _ = message;

        // Not yet implemented because of variadic cpp args
        unimplemented!()
    }

    /// This callback is used to notify the client of a connection status change.
    /// Consider displaying an overlay for the user to notify them why their stream
    /// is not performing as expected.
    fn connection_status_update(&mut self, status: ConnectionStatus);

    /// This callback is invoked to notify the client of a change in HDR mode on
    /// the host. The client will probably want to update the local display mode
    /// to match the state of HDR on the host. This callback may be invoked even
    /// if the stream is not using an HDR-capable codec.
    fn set_hdr_mode(&mut self, hdr_enabled: bool);

    /// This callback is invoked to rumble a gamepad. The rumble effect values
    /// set in this callback are expected to persist until a future call sets a
    /// different haptic effect or turns off the motors by passing 0 for both
    /// motors. It is possible to receive rumble events for gamepads that aren't
    /// physically present, so your callback should handle this possibility.
    fn controller_rumble(
        &mut self,
        controller_number: u16,
        low_frequency_motor: u16,
        high_frequency_motor: u16,
    );

    /// This callback is invoked to rumble a gamepad's triggers. For more details,
    /// see the comment above on ConnListenerRumble().
    fn controller_rumble_triggers(
        &mut self,
        controller_number: u16,
        left_trigger_motor: u16,
        right_trigger_motor: u16,
    );

    /// This callback is invoked to notify the client that the host would like motion
    /// sensor reports for the specified gamepad (see LiSendControllerMotionEvent())
    /// at the specified reporting rate (or as close as possible).
    ///
    /// If reportRateHz is 0, the host is asking for motion event reporting to stop.
    fn controller_set_motion_event_state(
        &mut self,
        controller_number: u16,
        motion_type: u8,
        report_rate_hz: u16,
    );

    /// This callback is invoked to notify the client of a change in the dualsense
    /// adaptive trigger configuration.
    fn controller_set_adaptive_triggers(
        &mut self,
        controller_number: u16,
        event_flags: u8,
        type_left: u8,
        type_right: u8,
        left: &mut u8,
        right: &mut u8,
    );

    /// This callback is invoked to set a controller's RGB LED (if present).
    fn controller_set_led(&mut self, controller_number: u16, r: u8, g: u8, b: u8);
}

static GLOBAL_CONNECTION_LISTENER: Mutex<Option<Box<dyn ConnectionListener + Send + 'static>>> =
    Mutex::new(None);

fn global_listener<R>(f: impl FnOnce(&mut dyn ConnectionListener) -> R) -> R {
    let lock = GLOBAL_CONNECTION_LISTENER.lock();
    let mut lock = lock.expect("global connection listener");

    let listener = lock.as_mut().expect("global connection listener");
    f(listener.as_mut())
}

pub(crate) fn new_global(listener: impl ConnectionListener + Send + 'static) -> Result<(), ()> {
    let mut global_listener = GLOBAL_CONNECTION_LISTENER.lock().map_err(|_| ())?;

    if global_listener.is_some() {
        return Err(());
    }
    *global_listener = Some(Box::new(listener));

    Ok(())
}
pub(crate) fn clear_global() {
    let mut decoder = GLOBAL_CONNECTION_LISTENER
        .lock()
        .expect("global video decoder");

    *decoder = None;
}

unsafe extern "C" fn stage_starting(stage: i32) {
    global_listener(|listener| {
        listener.stage_starting(Stage::from_i32(stage).expect("valid stage"));
    });
}
unsafe extern "C" fn stage_complete(stage: i32) {
    global_listener(|listener| {
        listener.stage_complete(Stage::from_i32(stage).expect("valid stage"));
    });
}
unsafe extern "C" fn stage_failed(stage: i32, error_code: i32) {
    global_listener(|listener| {
        listener.stage_failed(Stage::from_i32(stage).expect("valid stage"), error_code);
    });
}
unsafe extern "C" fn connection_started() {
    global_listener(|listener| {
        listener.connection_started();
    });
}
unsafe extern "C" fn connection_terminated(error_code: i32) {
    global_listener(|listener| {
        listener.connection_terminated(error_code);
    });
}
unsafe extern "C" fn connection_status_update(status: i32) {
    global_listener(|listener| {
        listener.connection_status_update(
            ConnectionStatus::from_i32(status).expect("valid connection status"),
        );
    });
}

// TODO: variadic args
// unsafe extern "C" fn log_message(message: *const i8) {
//     global_listener(|listener| unsafe {
//         let c_str = CStr::from_ptr(message);
//         let str = c_str.to_str().expect("valid utf8 string as log message");
//
//         listener.log_message(str);
//     });
// }

unsafe extern "C" fn set_hdr_mode(hdr_enabled: bool) {
    global_listener(|listener| {
        listener.set_hdr_mode(hdr_enabled);
    })
}

unsafe extern "C" fn controller_rumble(
    controller_number: u16,
    low_frequency_motor: u16,
    high_frequency_motor: u16,
) {
    global_listener(|listener| {
        listener.controller_rumble(controller_number, low_frequency_motor, high_frequency_motor);
    });
}
unsafe extern "C" fn controller_rumble_triggers(
    controller_number: u16,
    left_trigger_motor: u16,
    right_trigger_motor: u16,
) {
    global_listener(|listener| {
        listener.controller_rumble_triggers(
            controller_number,
            left_trigger_motor,
            right_trigger_motor,
        );
    });
}
unsafe extern "C" fn controller_set_motion_event_state(
    controller_number: u16,
    motion_type: u8,
    report_rate_hz: u16,
) {
    global_listener(|listener| {
        listener.controller_set_motion_event_state(controller_number, motion_type, report_rate_hz);
    })
}
unsafe extern "C" fn controller_set_led(controller_number: u16, r: u8, g: u8, b: u8) {
    global_listener(|listener| {
        listener.controller_set_led(controller_number, r, g, b);
    })
}
unsafe extern "C" fn controller_set_adaptive_triggers(
    controller_number: u16,
    event_flags: u8,
    type_left: u8,
    type_right: u8,
    left: *mut u8,
    right: *mut u8,
) {
    global_listener(|listener| {
        let (left, right) = unsafe { (&mut *left, &mut *right) };

        listener.controller_set_adaptive_triggers(
            controller_number,
            event_flags,
            type_left,
            type_right,
            left,
            right,
        );
    })
}

pub(crate) unsafe fn raw_callbacks() -> _CONNECTION_LISTENER_CALLBACKS {
    _CONNECTION_LISTENER_CALLBACKS {
        stageStarting: Some(stage_starting),
        stageComplete: Some(stage_complete),
        stageFailed: Some(stage_failed),
        connectionStarted: Some(connection_started),
        connectionTerminated: Some(connection_terminated),
        // TODO: log message
        logMessage: None,
        rumble: Some(controller_rumble),
        connectionStatusUpdate: Some(connection_status_update),
        setHdrMode: Some(set_hdr_mode),
        rumbleTriggers: Some(controller_rumble_triggers),
        setMotionEventState: Some(controller_set_motion_event_state),
        setControllerLED: Some(controller_set_led),
        setAdaptiveTriggers: Some(controller_set_adaptive_triggers),
    }
}
