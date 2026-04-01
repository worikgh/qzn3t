use qzn3t_audio_buffer::AudioBuffer;
// Copyright (c) 2026 Worik Turei Stanton
// License: GPL-3.0
#[allow(unused_imports)]
use qzn3t_engine::Engine;
use qzn3t_engine::{Session, converter};
#[allow(unused_imports)]
use std::path::PathBuf;
use std::{env, thread, time::Duration};

#[allow(unused_variables, unused_mut)]
/// Play audio
fn main() {
    let mut engine = Engine::new().unwrap();
    let path: PathBuf = PathBuf::from(format!(
        "{}/test_audio.wav",
        env::var("CARGO_MANIFEST_DIR").unwrap()
    ));
    let audio_data = converter::decode_audio(&path).unwrap();
    // dbg!(
    //	&path,
    //	audio_data.len(),
    //	audio_data[0].len(),
    // );
    let audio_buffer = AudioBuffer::new_data(audio_data).unwrap();
    let channels = audio_buffer.channels();
    dbg!(channels);
    let session = Session::default_mono_play(&path);
    let sink_port_names = session
        .out_ports
        .iter()
        .map(|p| p.to_string())
        .collect::<Vec<String>>();
    let mut engine = Engine::new().unwrap();
    engine.start_session(session).unwrap();

    let player = engine.get_player(audio_buffer);
    engine.add_stepper(Box::new(player));
    engine.connect_outputs(&sink_port_names).unwrap();
    if let Err(err) = engine.run() {
        panic!("{err}");
    }
    thread::sleep(Duration::from_secs(2));
}
