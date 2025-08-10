use log::info;
use moonlight_common::moonlight::connection::{ConnectionListener, ConnectionStatus, Stage};

pub struct StreamConnectionListener {}

impl ConnectionListener for StreamConnectionListener {
    fn stage_starting(&mut self, stage: Stage) {
        todo!()
    }

    fn stage_complete(&mut self, stage: Stage) {
        todo!()
    }

    fn stage_failed(&mut self, stage: Stage, error_code: i32) {
        todo!()
    }

    fn connection_started(&mut self) {
        todo!()
    }

    fn connection_terminated(&mut self, error_code: i32) {
        todo!()
    }

    fn log_message(&mut self, message: &str) {
        info!("[Stream Moonlight]: {message}");
    }

    fn connection_status_update(&mut self, status: ConnectionStatus) {
        todo!()
    }

    fn set_hdr_mode(&mut self, hdr_enabled: bool) {
        todo!()
    }

    fn controller_rumble(
        &mut self,
        controller_number: u16,
        low_frequency_motor: u16,
        high_frequency_motor: u16,
    ) {
        todo!()
    }

    fn controller_rumble_triggers(
        &mut self,
        controller_number: u16,
        left_trigger_motor: u16,
        right_trigger_motor: u16,
    ) {
        todo!()
    }

    fn controller_set_motion_event_state(
        &mut self,
        controller_number: u16,
        motion_type: u8,
        report_rate_hz: u16,
    ) {
        todo!()
    }

    fn controller_set_adaptive_triggers(
        &mut self,
        controller_number: u16,
        event_flags: u8,
        type_left: u8,
        type_right: u8,
        left: &mut u8,
        right: &mut u8,
    ) {
        todo!()
    }

    fn controller_set_led(&mut self, controller_number: u16, r: u8, g: u8, b: u8) {
        todo!()
    }
}
