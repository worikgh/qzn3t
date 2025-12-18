// Copyright (c) 2025 Worik Turei Stanton
// License: GPL-3.0

use jack::{AsyncClient, AudioOut, Client, Control, Port, ProcessHandler, ProcessScope};
use qzn3t_recorder::{
    app::{App, AppData},
    io::JackPipes,
    structs::Command,
    utils::get_sample_rate,
};

use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};

#[derive(Debug)]
#[allow(dead_code)]
enum WaveForm {
    Sine,
    Triangle,
    Square,
}

/// Generate a buffer of mono audio samples.
///
/// * `frequency` – Frequency of the tone in Hz.
/// * `volume`    – Linear gain (0.0‑1.0).
/// * `sample_rate` – Samples per second (e.g. 44_100).
/// * `duration_ms` – Length of the buffer in milliseconds.
/// * `wave_form` – Desired waveform.
fn generate_test_audio(
    frequency: u16,
    volume: f32,
    duration_ms: u32,
    wave_form: WaveForm,
) -> Vec<f32> {
    let sample_rate = get_sample_rate() as u32;
    let duration_samples = (duration_ms as u64 * sample_rate as u64) / 1_000;
    const PI: f32 = std::f32::consts::PI;
    match wave_form {
        WaveForm::Sine => {
            // sine(2πft)
            let angular_frequency = 2.0 * PI * frequency as f32 / sample_rate as f32;
            (0..duration_samples)
                .map(|i| (angular_frequency * i as f32).sin() * volume)
                .collect()
        }

        WaveForm::Square => {
            // square wave: sign(sin(2πft))
            let angular_frequency = 2.0 * PI * frequency as f32 / sample_rate as f32;
            (0..duration_samples)
                .map(|i| {
                    if (angular_frequency * i as f32).sin() >= 0.0 {
                        volume
                    } else {
                        -volume
                    }
                })
                .collect()
        }

        WaveForm::Triangle => {
            // triangle wave using a sawtooth then folding it:
            //  (2 / π) * asin(sin(2πft))
            let angular_frequency = 2.0 * PI * frequency as f32 / sample_rate as f32;
            (0..duration_samples)
                .map(|i| {
                    // asin returns values in [-π/2, π/2]; scaling yields a triangle in [-1, 1]
                    (2.0 / PI) * (angular_frequency * i as f32).sin().asin() * volume
                })
                .collect()
        }
    }
}

/// A test client that plays an audio buffer when the `run_flag` is
/// set.  It resets `run_flag` when it is finished.  It sets `active`
/// on the first `process` invocation
#[derive(Debug)]
struct TestAudioOutProcess {
    audio_buffers: Vec<Vec<f32>>,
    outputs: Vec<Port<AudioOut>>,
    position: usize,
    play_audio_f: Arc<AtomicBool>,
    active: Arc<AtomicBool>, // Set this on the first `process` invocation
}

impl ProcessHandler for TestAudioOutProcess {
    fn process(&mut self, _c: &Client, ps: &ProcessScope) -> Control {
        // FIXME: Create a struct to hold the audio buffers and the
        // output ports together
        assert_eq!(self.audio_buffers.len(), self.outputs.len());
        let olen = self.outputs.len();

        self.active.store(true, Ordering::SeqCst);

        let mut outputs: Vec<&mut [f32]> = self
            .outputs
            .iter_mut()
            .map(|o| o.as_mut_slice(ps))
            .collect();

        // The frame lengths must all be the same.
        let frames: Vec<usize> = outputs.iter().map(|o| o.len()).collect();
        assert!(
            frames
                .first()
                .map(|first| frames.iter().all(|x| x == first))
                .unwrap_or(false)
        );
        let frames: &usize = frames.first().unwrap();

        // All the audio buffers must be the same length
        assert!(
            self.audio_buffers
                .first()
                .map(|first| self.audio_buffers.iter().all(|x| x.len() == first.len()))
                .unwrap_or(false)
        );
        let slen = self.audio_buffers[0].len();
        for j in 0..*frames {
            if self.position >= slen {
                self.position = 0;
                self.play_audio_f.store(false, Ordering::SeqCst);
            }

            if self.play_audio_f.load(Ordering::SeqCst) {
                for (i, out) in outputs.iter_mut().enumerate().take(olen) {
                    let sample = self.audio_buffers[i][self.position];
                    out[j] = sample;
                }
                self.position += 1;
            } else {
                for out in outputs.iter_mut() {
                    out[j] = 0_f32;
                }
            }
        }

        Control::Continue
    }
}
pub struct Notifications;
impl jack::NotificationHandler for Notifications {}

/// Create a source for testing.  Creates a client `client_name` with
/// output ports from `port_names` and when the flag `play_audio_f` is
/// set it sends the contents of `audio_buffer` to the pipe.  When the
/// file is played `play_audio_f` is reset
fn make_jack_client_port(
    client_name: &str,
    port_names: Vec<&str>,
    audio_buffers: Vec<Vec<f32>>,
    play_audio_f: Arc<AtomicBool>,
) -> AsyncClient<Notifications, TestAudioOutProcess> {
    // FIXME: Create a struct to hold theport names and the audio
    // buffers
    assert_eq!(port_names.len(), audio_buffers.len());

    // Do not return the active client until it has started
    let active_flag = Arc::new(AtomicBool::new(false));

    let (client, _status) =
        match jack::Client::new(client_name, jack::ClientOptions::NO_START_SERVER) {
            Ok(cs) => cs,
            Err(err) => panic!("Failed creating test client {client_name}: {err}"),
        };

    // The names of the ports to output data on
    let outputs: Vec<jack::Port<jack::AudioOut>> = port_names
        .iter()
        .map(|p| match client.register_port(p, AudioOut::default()) {
            Ok(p) => p,
            Err(err) => {
                panic!("Cannot create output port {p} for client {client_name}. {err}")
            }
        })
        .collect();

    let out_process = TestAudioOutProcess {
        audio_buffers,
        outputs,
        position: 0,
        play_audio_f,
        active: active_flag.clone(),
    };
    match client.activate_async(Notifications, out_process) {
        Ok(ac) => {
            let mut activate_wait = 0;
            loop {
                if active_flag.load(Ordering::SeqCst) {
                    println!("DBG activate_wait: {activate_wait}");
                    return ac;
                }
                thread::sleep(Duration::from_millis(1));
                activate_wait += 1;
                if activate_wait == 100 {
                    panic!("Could not activate client");
                }
            }
        }
        Err(err) => panic!("Cannot create async for client {client_name}. {err}"),
    }
}

// Tests todo:
// `get_audio_from_jack` when the pipe is disconnected.  Test the error

/// Generate two audio tracks: sine and triangle waves.  Record both
/// simultaneously and save them both to disc files.
#[test]
fn record_two_channels() {
    // The test audio
    let audio_duration_ms = 2_000;

    let audio_buffer_sine = generate_test_audio(220, 0.25, audio_duration_ms, WaveForm::Sine);
    let audio_buffer_sine = trim_audio(&audio_buffer_sine);

    let audio_buffer_tri = generate_test_audio(220, 0.25, audio_duration_ms, WaveForm::Triangle);
    let audio_buffer_tri = trim_audio(&audio_buffer_tri);

    // Directory recorded audio is sent to
    let dir = dst_dir();

    // Client to play the output:
    let port_sine = "sine-wave";
    let port_tri = "tri-wave";

    let sine_name = "record_two_channels_sine.raw";
    let tri_name = "record_two_channels_tri.raw";

    let sine_path = dir.join(sine_name);
    let tri_path = dir.join(tri_name);

    let client_name = "integration_test";
    let (ac, play_audio_f) = play_test_audio(
        client_name,
        vec![port_sine, port_tri],
        vec![&audio_buffer_sine, &audio_buffer_tri],
    );

    // Set up the recorder
    // The inputs (Jack pipes to record) first
    let mut inputs = JackPipes::new();
    let port_tri = format!("{}:{port_tri}", ac.as_client().name());
    if let Err(err) = inputs.add_name(&port_tri, tri_name) {
        panic!("{err}");
    }
    let port_sine_complete = format!("{}:{port_sine}", ac.as_client().name());
    if let Err(err) = inputs.add_name(&port_sine_complete, sine_name) {
        panic!("{err}");
    }
    let mut recorder = set_up_recorder(
        vec![port_sine_complete, port_tri],
        vec![sine_name.to_string(), tri_name.to_string()],
        &dir,
    );

    // Start the recorder.
    if let Err(err) = recorder.handle_record() {
        panic!("Called handle_recording(): {err}");
    }

    // Start the test signal
    play_audio_f.store(true, Ordering::SeqCst);

    // Wait for audio to stop
    thread::sleep(Duration::from_millis(audio_duration_ms as u64));
    let mut loop_cnt = 0_u64;
    let delay_ms = 100;
    const LOOP_LIM: u64 = 10;
    loop {
        loop_cnt += 1;
        if !play_audio_f.load(Ordering::SeqCst) {
            break;
        }
        if loop_cnt >= LOOP_LIM {
            panic!(
                "Audio has not stopped playing: {}ms elapsed",
                loop_cnt * delay_ms
            );
        }
        thread::sleep(Duration::from_millis(100));
    }

    if let Err(err) = recorder.handle_audio_stop() {
        panic!("Could not stop audio: {err}");
    }

    // Get two recorded buffers
    let rec_tri = recorder.recorded_audio.get(tri_name).unwrap();
    let rec_tri = trim_audio(rec_tri);
    let rec_sine = recorder.recorded_audio.get(sine_name).unwrap();
    let rec_sine = trim_audio(rec_sine);

    // Reset this on failed tests
    let mut result = true;

    // The two recorded buffers must be identical to the source buffers
    if rec_sine.len() != audio_buffer_sine.len() {
        eprintln!(
            "Fail: Sine lengths differ: recorded: {} Original: {}",
            rec_sine.len(),
            audio_buffer_sine.len()
        );
        result = false;
    } else {
        for i in 0..audio_buffer_sine.len() {
            if (rec_sine[i] - audio_buffer_sine[i]).abs() > f32::EPSILON {
                eprintln!("Fail: Sine differs at {i}");
                result = false;
                break;
            }
        }
    }
    if rec_tri.len() != audio_buffer_tri.len().min(rec_tri.len()) {
        eprintln!("Fail: Triangle lengths differ");
        result = false;
    } else {
        for i in 0..audio_buffer_tri.len() {
            if (rec_tri[i] - audio_buffer_tri[i]).abs() > f32::EPSILON {
                eprintln!("Fail: Triangle differs at {i}");
                result = false;
                break;
            }
        }
    }
    if result {
        // Good so far.  The recorded buffers match the input.  Now
        // check the recorded files are the same

        // Sine wave
        match App::read_f32_vec_from_file(&sine_path) {
            Ok(recovered_sine) => {
                let recovered_sine = trim_audio(&recovered_sine);
                if recovered_sine.len() != rec_sine.len() {
                    eprintln!(
                        "Fail: recovered_sine.len()/{} != rec_sine.len()/{}",
                        recovered_sine.len(),
                        rec_sine.len()
                    );
                    result = false;
                } else {
                    for i in 0..rec_sine.len() {
                        if (rec_sine[i] - recovered_sine[i]).abs() > f32::EPSILON {
                            eprintln!("Fail: Recorded sine differs at {i}");
                            result = false;
                            break;
                        }
                    }
                }
            }
            Err(err) => {
                eprintln!("Fail: Cannot read data from {sine_path:?}.  Error: {err}");
                result = false;
            }
        };

        match App::read_f32_vec_from_file(&tri_path) {
            Ok(recovered_tri) => {
                let recovered_tri = trim_audio(&recovered_tri);
                if recovered_tri.len() != rec_tri.len() {
                    eprintln!(
                        "Fail: recovered_tri.len()/{} != rec_tri.len()/{}",
                        recovered_tri.len(),
                        rec_tri.len()
                    );
                    result = false;
                } else {
                    for i in 0..rec_tri.len() {
                        if (rec_tri[i] - recovered_tri[i]).abs() > f32::EPSILON {
                            eprintln!("Fail: Recorded tri differs at {i}");
                            result = false;
                            break;
                        }
                    }
                }
            }
            Err(err) => {
                eprintln!("Fail: Cannot read data from {tri_path:?}.  Error: {err}");
                result = false;
            }
        };
    }
    assert!(result);
}

#[test]
/// Record one channel of audio.
/// Save it to disc.
fn record_audio() {
    let duration_ms = 2_000;

    // The test audio
    let audio_buffer = generate_test_audio(220, 0.25, duration_ms, WaveForm::Sine);
    let audio_buffer = trim_audio(&audio_buffer);

    let port_name = "record_audio";
    let port_label = "test_record_audio.raw";
    let client_name = "integration_test";
    let (ac, play_audio_flag) = play_test_audio(client_name, vec![port_name], vec![&audio_buffer]);
    let port_name_complete = format!("{}:{port_name}", ac.as_client().name());

    // Directory recordings go to
    let dir = dst_dir();
    let saved_path = dir.join(port_label);

    // Set up recorder
    let mut recorder = set_up_recorder(
        vec![port_name_complete.clone()],
        vec![port_label.to_string()],
        &dir,
    );

    // Record data from `port_name`
    if let Err(err) = recorder.handle_record() {
        panic!("Called handle_recording(): {err}");
    }

    // Start the test signal
    play_audio_flag.store(true, Ordering::SeqCst);

    // Wait for audio to stop
    thread::sleep(Duration::from_millis(duration_ms as u64));

    let mut loop_cnt = 0_u64;
    let delay_ms = 100;
    const LOOP_LIM: u64 = 10;
    loop {
        loop_cnt += 1;
        if !play_audio_flag.load(Ordering::SeqCst) {
            break;
        }
        if loop_cnt >= LOOP_LIM {
            panic!(
                "Audio has not stopped playing: {}ms elapsed",
                loop_cnt * delay_ms
            );
        }
        thread::sleep(Duration::from_millis(delay_ms));
    }

    if let Err(err) = recorder.handle_audio_stop() {
        panic!("Could not stop audio: {err}");
    }

    // Get data out of the recorder
    let new_buffer = trim_audio(recorder.recorded_audio.get(port_label).unwrap());

    // The buffers should be the same length
    assert_eq!(audio_buffer.len(), new_buffer.len());

    // The buffers should be the same exactly
    for i in 0..audio_buffer.len() {
        assert!((audio_buffer[i] - new_buffer[i]).abs() < f32::EPSILON);
    }

    // Check the saved data
    let mut result = true;
    match App::read_f32_vec_from_file(&saved_path) {
        Ok(d) => {
            let imported_data = trim_audio(&d);
            if imported_data.len() != new_buffer.len() {
                eprintln!(
                    "Fail: recovered_tri.len()/{} != rec_tri.len()/{}",
                    imported_data.len(),
                    new_buffer.len()
                );
                result = false;
            } else {
                for i in 0..new_buffer.len() {
                    if (new_buffer[i] - imported_data[i]).abs() > f32::EPSILON {
                        eprintln!("Fail: Recorded tri differs at {i}");
                        result = false;
                        break;
                    }
                }
            }
        }
        Err(err) => panic!("Failed to read data from {saved_path:?}.  Error: {err}"),
    };
    assert!(result);
}

/// Remove leading and trailing zeros from an audio buffer
fn trim_audio(audio_buffer: &[f32]) -> Vec<f32> {
    let mut result = vec![];

    let mut insert = false; // Set first this first non-zero
    for &d in audio_buffer.iter() {
        if d.abs() > f32::EPSILON {
            insert = true;
        }
        if insert {
            result.push(d);
            continue;
        }
    }
    // Remove trailing zeros
    while result.last().is_some_and(|&x| x.abs() < f32::EPSILON) {
        result.pop();
    }
    result
}

/// Set up a recorder for testing
fn set_up_recorder(port_names: Vec<String>, port_labels: Vec<String>, dir: &PathBuf) -> AppData {
    assert_eq!(port_names.len(), port_labels.len());
    let mut inputs = JackPipes::new();
    let outputs = JackPipes::new();
    for p in port_names.iter().zip(port_labels.iter()) {
        inputs.add_name(p.0, p.1).unwrap();
    }
    let (_audio_tx, _audio_rx) = mpsc::channel::<f32>();
    let (_command_tx, _command_rx) = mpsc::channel::<Command>();

    let mut app = App;
    match app.initialise(_audio_tx, _command_rx, inputs, outputs, dir) {
        Ok(a) => a,
        Err(err) => panic!("Cannot initalise AppData: {err}"),
    }
}

/// Output some audio through a new Jack client.  Return the async
/// Jack client and flag `play_audio_f` that controls the audio
/// playing. Audio plays when `play_audio_f` is true and is stopped
/// when false.
fn play_test_audio(
    client_name: &str,
    port_names: Vec<&str>,
    audio_data: Vec<&[f32]>,
) -> (
    AsyncClient<Notifications, TestAudioOutProcess>,
    Arc<AtomicBool>,
) {
    let play_audio_f = Arc::new(AtomicBool::new(false));
    let ac = make_jack_client_port(
        client_name,
        port_names,
        audio_data
            .iter()
            .map(|b| b.to_vec())
            .collect::<Vec<Vec<f32>>>(),
        play_audio_f.clone(),
    );
    (ac, play_audio_f)
}

/// The destination directory
fn dst_dir() -> PathBuf {
    PathBuf::from("/home/worik/Documents/2025/qzn3t/recorder/tests/data")
    //temp_dir()
}
