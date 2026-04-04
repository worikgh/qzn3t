use qzn3t_audio_buffer::{AudioBuffer, get_sample_rate};
// Copyright (c) 2026 Worik Turei Stanton
// License: GPL-3.0
#[allow(unused_imports)]
use qzn3t_engine::Engine;
use qzn3t_engine::{Session, converter, stepper::StepCommand};
#[allow(unused_imports)]
use std::path::PathBuf;
use std::{env, thread, time::Duration};

#[allow(unused_variables, unused_mut)]
/// Play audio
fn main() {
    let mut engine = Engine::new();
    let path: PathBuf = PathBuf::from(format!(
        "{}/test_audio.wav",
        env::var("CARGO_MANIFEST_DIR").unwrap()
    ));
    let audio_data = converter::decode_audio(&path).unwrap();

    let audio_buffer = AudioBuffer::new_data(audio_data).unwrap();
    let sleep_ms = audio_buffer.len() * 1000 / get_sample_rate() as usize;

    let channels = audio_buffer.channels();
    let session = Session::default_mono_play(&path);
    let sink_port_names = session
        .out_ports
        .iter()
        .map(|p| p.to_string())
        .collect::<Vec<String>>();
    let mut engine = Engine::new();

    engine.start_session(session).unwrap();
    engine.connect_outputs(&sink_port_names).unwrap();

    if let Err(err) = engine.run() {
        panic!("{err}");
    }
    if let Err(err) = engine.send_to_loop(StepCommand::NewStepper(Box::new(
        engine.get_player(audio_buffer),
    ))) {
        panic!("{err}");
    }

    thread::sleep(Duration::from_millis(sleep_ms as u64));
}
