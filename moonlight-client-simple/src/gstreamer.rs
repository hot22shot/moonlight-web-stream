use gstreamer::{ClockTime, MessageType, State, prelude::ElementExt};

pub fn init() {
    gstreamer::init().expect("failed to init gstreamer");

    test();
}

fn test() {
    let pipeline = gstreamer::parse::launch(
        "playbin uri=https://gstreamer.freedesktop.org/data/media/sintel_trailer-480p.webm",
    )
    .unwrap();

    pipeline.set_state(State::Playing).unwrap();

    let bus = pipeline.bus().unwrap();

    let msg = bus.timed_pop_filtered(ClockTime::NONE, &[MessageType::Error, MessageType::Eos]);
    if let Some(msg) = msg {
        panic!("error {msg:?}");
    }

    pipeline.set_state(State::Null).unwrap();
}
