// Copyright (c) 2026 Worik Turei Stanton
// License: GPL-3.0

//! This test fails, and I do not know why.
//! Other integration tests that use `send_audo_to_jack` indirectly pass.
use qzn3t_recorder::test_utils::common::{
    WaveForm, generate_test_audio, make_test_play_client, trim_audio,
};

use jack::PortFlags;
use qzn3t_recorder::send_audio_to_jack::send_audo_to_jack;

use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};

/// There is a timing bug
/// Using `cargo test play_three_channels`
/// `play_three_channels` fails when AUDIO_DURATION <=  95
/// `play_three_channels` pass  when AUDIO_DURATION >=  99
/// Using `cargo test`
/// AUDIO_DURATION must be approximately at least 1_000 to pass all
/// tests.  But it is very sensitive, non-linear and intermittent
/// The duration of the audio in ms
const AUDIO_DURATION: u32 = 1_000;

#[test]
#[ignore]
fn test_send_audo_to_jack() {
    // The audio buffers
    let audio_buffer_sine = generate_test_audio(220, 0.25, AUDIO_DURATION, WaveForm::Sine);
    let audio_buffer_sine = trim_audio(&audio_buffer_sine);
    let audio_buffer_triangle = generate_test_audio(220, 0.25, AUDIO_DURATION, WaveForm::Triangle);
    let audio_buffer_triangle = trim_audio(&audio_buffer_triangle);
    let audio_buffer_square = generate_test_audio(220, 0.25, AUDIO_DURATION, WaveForm::Square);
    let audio_buffer_square = trim_audio(&audio_buffer_square);

    // The three channels to send the data to playback with
    let (sine_tx, sine_rx) = mpsc::channel::<f32>();
    let (triangle_tx, triangle_rx) = mpsc::channel::<f32>();
    let (square_tx, square_rx) = mpsc::channel::<f32>();

    // When this is true data on channels is send to the output.  When
    // it is false the data is ignored (audio off, but still a sink
    // for audio data)
    let run_flag = Arc::new(AtomicBool::new(true));

    // Create a Jack client to sink the audio.  Three ports, that each
    // fill a buffer that is accessible from here to test (and reset)
    let audio_buffer_sink_sine: Arc<Mutex<Vec<f32>>> = Arc::new(Mutex::new(Vec::new()));
    let audio_buffer_sink_triangle: Arc<Mutex<Vec<f32>>> = Arc::new(Mutex::new(Vec::new()));
    let audio_buffer_sink_square: Arc<Mutex<Vec<f32>>> = Arc::new(Mutex::new(Vec::new()));
    let buffers = vec![
        audio_buffer_sink_sine.clone(),
        audio_buffer_sink_triangle.clone(),
        audio_buffer_sink_square.clone(),
    ];
    let port_names = vec![
        "playback_1".to_string(),
        "playback_2".to_string(),
        "playback_3".to_string(),
    ];

    let client_name = "test_play_client";
    let (ac, _, _) = make_test_play_client(client_name, port_names, buffers).unwrap();
    let port_names = ac
        .as_client()
        .ports(Some(client_name), None, PortFlags::IS_INPUT);

    // The process that will be tested
    let _ac = send_audo_to_jack(
        vec![
            (sine_rx, port_names[0].clone()),
            (triangle_rx, port_names[1].clone()),
            (square_rx, port_names[2].clone()),
        ],
        run_flag.clone(),
    )
    .map_err(|err| panic!("send_audio_to_jack failed: {err}"))
    .unwrap();
    for item in &audio_buffer_sine {
        sine_tx
            .send(*item)
            .map_err(|err| panic!("Failed to send sine: {err}"));
    }
    for item in &audio_buffer_triangle {
        triangle_tx
            .send(*item)
            .map_err(|err| panic!("Failed to send triangle: {err}"));
    }
    for item in &audio_buffer_square {
        square_tx
            .send(*item)
            .map_err(|err| panic!("Failed to send square: {err}"));
    }
    run_flag.store(true, Ordering::Relaxed);
    // Let audio playback run.  A real-time process that takes time
    thread::sleep(Duration::from_millis(AUDIO_DURATION as u64));

    // Capture copies of the sinks, and trim leading and trailing zeros
    let audio_buffer_sink_square = trim_audio(&audio_buffer_sink_square.lock().unwrap());
    let audio_buffer_sink_triangle = trim_audio(&audio_buffer_sink_triangle.lock().unwrap());
    let audio_buffer_sink_sine = trim_audio(&audio_buffer_sink_sine.lock().unwrap());
    // Test that the sinks are the same as the original buffers
    assert_eq!(audio_buffer_sink_square.len(), audio_buffer_square.len());
    assert_eq!(audio_buffer_sink_sine.len(), audio_buffer_sine.len());
    assert_eq!(
        audio_buffer_sink_triangle.len(),
        audio_buffer_triangle.len()
    );
    for (idx, sample) in audio_buffer_sink_square.iter().enumerate() {
        assert!((sample - audio_buffer_square[idx]).abs() < f32::EPSILON);
    }
    for (idx, sample) in audio_buffer_sink_sine.iter().enumerate() {
        assert!((sample - audio_buffer_sine[idx]).abs() < f32::EPSILON);
    }
    for (idx, sample) in audio_buffer_sink_triangle.iter().enumerate() {
        assert!((sample - audio_buffer_triangle[idx]).abs() < f32::EPSILON);
    }
}
