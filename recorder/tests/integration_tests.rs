// Copyright (c) 2025 Worik Turei Stanton
// License: GPL-3.0

use std::{
    sync::{Arc, atomic::AtomicBool, mpsc::channel},
    thread,
    time::Duration,
};

#[test]
fn test_basic_functionality() {
    assert!(1 + 1 == 2);
}

/// Generate one second of sin wave
fn generate_sin_wave(frequency: u16, volume: f32, sample_rate: u32) -> Vec<f32> {
    let duration_samples = sample_rate as usize;
    let angular_frequency = 2.0 * std::f32::consts::PI * frequency as f32 / sample_rate as f32;

    (0..duration_samples)
        .map(|i| (angular_frequency * i as f32).sin() * volume)
        .collect()
}
#[test]
fn play_note() {
    let (tx, rx) = channel::<f32>();
    let audio_run = Arc::new(AtomicBool::new(true));
    let name = "outport";
    let _ac =
        qzn3t_recorder::send_audio_to_jack::send_audo_to_jack(name, rx, audio_run.clone()).unwrap();
    let sin = generate_sin_wave(220, 0.75, 48_000);
    for s in sin {
        if let Err(err) = tx.send(s) {
            panic!("Send err: {err}");
        }
    }
    thread::sleep(Duration::from_millis(1_100));
}
