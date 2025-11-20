// Copyright (c) 2025 Worik Turei Stanton
// License: GPL-3.0

use jack::{AsyncClient, AudioOut, Client, Control, Port, ProcessHandler, ProcessScope};
use qzn3t_pitch_detection::note_detection_result;
use qzn3t_pitch_detection::runner;
use qzn3t_recorder::{
    app::{App, AppData},
    structs::Command,
    utils::get_sample_rate,
};
use std::time::Instant;
use std::{
    env::temp_dir,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};

/// Generate `duration_ms` milli-seconds of a wave form
#[derive(Debug)]
#[allow(dead_code)]
enum WaveForm {
    Sin,
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
        WaveForm::Sin => {
            // sin(2πft)
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
// fn generate_test_audio(
//     frequency: u16,
//     volume: f32,
//     sample_rate: u32,
//     duration_ms: u32,
//     wave_form: WaveForm,
// ) -> Vec<f32> {
//     let duration_samples = (duration_ms as u64 * sample_rate as u64) / 1_000;
//     match wave_form {
//         WaveForm::Sin => {
//             let angular_frequency =
//                 2.0 * std::f32::consts::PI * frequency as f32 / sample_rate as f32;

//             (0..duration_samples)
//                 .map(|i| (angular_frequency * i as f32).sin() * volume)
//                 .collect()
//         }
//         wave_form => panic!("Unimplemented waveform: {wave_form:?}"),
//     }
// }

/// A test client that plays an audio buffer when the `run_flag` is
/// set.  It resets `run_flag` when it is finished.  It sets `active`
/// on the first `process` invocation
///
struct TestAudioOutProcess {
    audio_buffer: Vec<f32>,
    output: Port<AudioOut>,
    position: usize,
    play_audio_flag: Arc<AtomicBool>,
    active: Arc<AtomicBool>, // Set this when the first `process` invocation
}
impl ProcessHandler for TestAudioOutProcess {
    fn process(&mut self, _c: &Client, ps: &ProcessScope) -> Control {
        self.active.store(true, Ordering::SeqCst);
        let output = self.output.as_mut_slice(ps);
        let frames = output.len();
        let samples: &Vec<f32> = &self.audio_buffer;

        for (i, _) in (0..frames).enumerate() {
            if self.position >= samples.len() {
                self.position = 0;
                self.play_audio_flag.store(false, Ordering::SeqCst);
            }
            if self.play_audio_flag.load(Ordering::SeqCst) {
                let sample = samples[self.position];
                output[i] = sample;
                self.position += 1;
            } else {
                output[i] = 0_f32;
            }
        }
        Control::Continue
    }
}
pub struct Notifications;
impl jack::NotificationHandler for Notifications {}

/// Create a source for testing.  Creates a client `client_name` with
/// output port `port_name` and when the flag `run_flag` is set it
/// sends the contents of `audio_buffer` to the pipe.
fn make_jack_client_port(
    client_name: &str,
    port_name: &str,
    audio_buffer: Vec<f32>,
    play_audio_flag: Arc<AtomicBool>,
) -> AsyncClient<Notifications, TestAudioOutProcess> {
    // Do not return the active client until it has started
    let active_flag = Arc::new(AtomicBool::new(false));

    let (client, _status) =
        match jack::Client::new(client_name, jack::ClientOptions::NO_START_SERVER) {
            Ok(cs) => cs,
            Err(err) => panic!("Failed creating test client {client_name}: {err}"),
        };
    let output = match client.register_port(port_name, AudioOut::default()) {
        Ok(p) => p,
        Err(err) => panic!("Cannot create output port {port_name} for client {client_name}. {err}"),
    };

    let out_process = TestAudioOutProcess {
        audio_buffer,
        output,
        position: 0,
        play_audio_flag,
        active: active_flag.clone(),
    };
    match client.activate_async(Notifications, out_process) {
        Ok(ac) => {
            let mut activate_wait = 0;
            loop {
                if active_flag.load(Ordering::SeqCst) {
                    dbg!(activate_wait);
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

/// Generate a sin wave in a buffer
/// Send it to the recorder from a Jackd client
/// Check the recorded audio is essentially the same
/// TODO: Why is it not exact?
#[test]
fn record_audio() {
    let sample_rate = get_sample_rate();

    // The test audio
    let audio_buffer = generate_test_audio(220, 0.25, 1_000, WaveForm::Triangle);
    let audio_buffer = trim_audio(&audio_buffer);

    // A pitch detector to get the pitch of the original and recorded audio
    let (note_rx, audio_tx) = start_pitch_detection();

    // Get frequency and RMS volume
    let frequency = get_mean_pitch(&audio_buffer, &note_rx, &audio_tx);
    let rms = (audio_buffer.iter().fold(0.0, |a, b| a + b * b) / audio_buffer.len() as f32).sqrt();

    // // Set this to start the test audio
    // let play_audio_flag = Arc::new(AtomicBool::new(false));

    let port_name = "output";
    let client_name = "integration_test";
    let (ac, play_audio_flag) = play_test_audio(client_name, port_name, &audio_buffer);
    let port_name = format!("{}:{port_name}", ac.as_client().name());

    // Set up recorder
    let mut app_data = set_up_recorder(&port_name);

    // Record data from `port_name`
    if let Err(err) = app_data.handle_recording() {
        panic!("Called handle_recording(): {err}");
    }

    // FIXME: This should be a test (or `jack_rec` should be fixed to
    // not return a handle until the client is ready)
    // thread::sleep(Duration::from_secs(1));

    // Start the test signal
    play_audio_flag.store(true, Ordering::SeqCst);

    // Wait for audio to stop
    let duration_ms = 1_000 * audio_buffer.len() / sample_rate;
    thread::sleep(Duration::from_millis(duration_ms as u64));

    loop {
        if !play_audio_flag.load(Ordering::SeqCst) {
            break;
        }
        thread::sleep(Duration::from_millis(100));
    }

    if let Err(err) = app_data.handle_audio_stop() {
        panic!("Could not stop audio: {err}");
    }

    // Get data out of the recorder
    let new_buffer = trim_audio(&app_data.recorded_audio);

    let new_duration_ms = (1_000 * new_buffer.len()) / sample_rate;
    let duration_ms = (1_000 * audio_buffer.len()) / sample_rate;
    assert_eq!(new_duration_ms, duration_ms);

    // Get pitch of the recorded data
    let mean_freq_new = get_mean_pitch(&new_buffer, &note_rx, &audio_tx);

    // Allow 1hz difference.  Pitch detection is not perfect
    assert!((frequency - mean_freq_new).abs() < 1.0);

    let rms_new = (new_buffer.iter().fold(0.0, |a, b| a + b * b) / new_buffer.len() as f32).sqrt();

    // Test to two decimal points
    let rms = (rms * 100.0).round() as u32;
    let rms_new = (rms_new * 100.0).round() as u32;
    assert_eq!(rms, rms_new);

    // Always fail so output can be examined.  Remove this when the
    // test is implemented properly
}

// If the programme `jack-scope` is available use it to display an original wave and a recorded wave, for visual confirmation
// #[test]
#[allow(dead_code)]
fn display_audio() {
    // Set up the scope
    use std::process::Command;
    use std::process::Stdio;

    let test_audio = generate_test_audio(110, 0.42, 10_500, WaveForm::Triangle);
    let port_name = "output";
    let client_name = "integration_test";
    let (ac, play_audio_flag) = play_test_audio(client_name, port_name, &test_audio);
    let port_name = format!("{}:{port_name}", ac.as_client().name());

    // Set up the recorder to test and start recording
    let mut app_data = set_up_recorder(&port_name);
    if let Err(err) = app_data.handle_recording() {
        panic!("Called handle_recording(): {err}");
    }

    // Start audio
    play_audio_flag.store(true, Ordering::SeqCst);

    // Wait for audio to finish, and a bit
    let duration_ms = 1_100 * test_audio.len() / get_sample_rate();
    thread::sleep(Duration::from_millis(duration_ms as u64));
    loop {
        if !play_audio_flag.load(Ordering::SeqCst) {
            break;
        }
        dbg!("Did not wait long enough");
        thread::sleep(Duration::from_millis(100));
    }
    if let Err(err) = app_data.handle_audio_stop() {
        panic!("Could not stop audio: {err}");
    }

    // Now the audio is in two places:
    // 1. `test_audio`
    // 2. `app_data.recorded_audio`
    // Play both into `jack-scope`
    let port_name = "output";
    let client_name_1 = "origanal_audio";
    let client_name_2 = "recorded_audio";
    let (ac_1, play_audio_flag_1) = play_test_audio(client_name_1, port_name, &test_audio);
    let port_name_1 = format!("{}:{port_name}", ac_1.as_client().name());
    let (ac_2, play_audio_flag_2) =
        play_test_audio(client_name_2, port_name, &app_data.recorded_audio);
    let port_name_2 = format!("{}:{port_name}", ac_2.as_client().name());

    // Start the scope
    let mut jack_scope_child = Command::new("jack-scope")
        .args(["-n", "2"])
        // .args(["-n", "2", "-w", "1024", "-b", "4096"])
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let pid = jack_scope_child.id();
    let scope_client_name = format!("jack-scope-{pid}");
    let pipe_1 = format!("{scope_client_name}:in_1");
    let pipe_2 = format!("{scope_client_name}:in_2");
    dbg!(&pipe_1, &pipe_2);

    // Check all the pipes are ready (Jack you plonker!!) This could
    // just as easilly be a little sleep
    let fail_limit = 4;
    let mut fail_cnt = 0;
    loop {
        let jack_lsp = Command::new("jack_lsp").output().unwrap();
        let ports = String::from_utf8(jack_lsp.stdout)
            .unwrap()
            .split("\n")
            .map(|p| p.to_string())
            .collect::<Vec<String>>();
        let error = String::from_utf8(jack_lsp.stderr).unwrap();
        let pipe_1_ready = ports.contains(&pipe_1);
        let pipe_2_ready = ports.contains(&pipe_2);

        if !pipe_1_ready || !pipe_2_ready {
            dbg!(&ports);
            dbg!(&error);
            dbg!(pipe_1_ready, pipe_2_ready);
            thread::sleep(Duration::from_millis(1000));
            fail_cnt += 1;
            if fail_cnt == fail_limit {
                panic!("Cannot connect ports to scope as they do not exist");
            }
            continue;
        }
        break;
    }
    // Connect the pipes
    if let Err(err) = ac_1
        .as_client()
        .connect_ports_by_name(&port_name_1, &pipe_1)
    {
        panic!("Failed to connect test_audio to scope: {err}");
    }
    if let Err(err) = ac_2
        .as_client()
        .connect_ports_by_name(&port_name_2, &pipe_2)
    {
        panic!("Failed to connect recorded audio to scope: {err}");
    }

    // Start audio
    play_audio_flag_1.store(true, Ordering::SeqCst);
    play_audio_flag_2.store(true, Ordering::SeqCst);

    // Wait for audio to finish, and a bit, to display it in the scope
    let duration_ms = 1_100 * test_audio.len() / get_sample_rate();
    thread::sleep(Duration::from_millis(duration_ms as u64));
    loop {
        if !play_audio_flag.load(Ordering::SeqCst) {
            break;
        }
        dbg!("Did not wait long enough");
        thread::sleep(Duration::from_millis(100));
    }

    jack_scope_child.kill().unwrap();

    // If there was stderr, display it
    if let Ok(output) = jack_scope_child.wait_with_output()
        && !output.stderr.is_empty()
    {
        panic!("Stderr: {}", String::from_utf8_lossy(&output.stderr));
    }
    // Fail the test so the output is dislayed.
    panic![];
}

fn start_pitch_detection() -> (
    mpsc::Receiver<note_detection_result::NoteDetectionResult>,
    mpsc::Sender<f32>,
) {
    // A pitch detector to get the pitch of the sin wave
    let (note_data_tx, note_data_rx) =
        mpsc::channel::<note_detection_result::NoteDetectionResult>();
    let (audio_data_tx, audio_data_rx) = mpsc::channel::<f32>();
    let sample_rate = get_sample_rate() as u32;
    let detector_cfg = runner::DetectorCfg {
        sample_rate,
        size: 16384,
        padding: 1024,
        power_threshold: 0.1,
        clarity_threshold: 0.5,
        detector: runner::Detector::McLeod,
    };
    let _jh = runner::pitch_detection_run(note_data_tx, audio_data_rx, &detector_cfg, None);
    (note_data_rx, audio_data_tx)
}

fn get_mean_pitch(
    audio_buffer: &[f32],
    note_rx: &mpsc::Receiver<note_detection_result::NoteDetectionResult>,
    audio_tx: &mpsc::Sender<f32>,
) -> f32 {
    let sample_rate = get_sample_rate();
    // Clear the channel sending note results
    while note_rx.try_recv().is_ok() {}

    // Calculate the pitch
    for a in audio_buffer.iter() {
        if let Err(err) = audio_tx.send(*a) {
            panic!("{err}");
        }
    }

    // For the number of milli seconds the audio buffer is, loop detecting notes
    let ms = audio_buffer.len() * 1_000 / sample_rate;
    // Store the frequency readings.  this will be averaged.
    let mut freqs = Vec::new();
    let now = Instant::now();
    loop {
        if now.elapsed().as_millis() >= ms as u128 {
            break;
        }
        let ndr = match note_rx.recv_timeout(Duration::from_millis((12 * ms / 10) as u64)) {
            Ok(ndr) => ndr,
            Err(mpsc::RecvTimeoutError::Timeout) => break,
            Err(err) => panic!("{err}"),
        };
        freqs.push(ndr.actual_freq);
    }
    if freqs.is_empty() {
        0_f32
    } else {
        freqs.iter().fold(0.0, |a, &b| a + b) / freqs.len() as f32
    }
}

/// Remove leading and trailing zeros frm an audio buffer
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

/// Set up the recorder for use
fn set_up_recorder(port_name: &str) -> AppData {
    // Set up recorder
    // These two channels will not be used but they are required to build `AppData`
    let (_audio_tx, _audio_rx) = mpsc::channel::<f32>();
    let (_command_tx, _command_rx) = mpsc::channel::<Command>();

    // Set up output file to save audio in Not used in this test but
    // there must be an output file for a `recorder`
    let mut dir = temp_dir();
    dir.push("sinwave.raw");
    let file_name = match dir.as_path().to_str() {
        Some(f) => f,
        None => panic!("Cannot convert {dir:?} to string"),
    };

    let mut app = App::new();

    match app.initialise(_audio_tx, _command_rx, port_name, file_name.into(), true) {
        Ok(a) => a,
        Err(err) => panic!("Cannot initalise AppData: {err}"),
    }
}

/// Output some audio through a new Jack client.  Return the async
/// Jack client and flag that starts the audio playing.  The audio
/// player will reset the flag when it is finished (resetting the flag
/// will stop the audio playing)
fn play_test_audio(
    client_name: &str,
    port_name: &str,
    audio_buffer: &[f32],
) -> (
    AsyncClient<Notifications, TestAudioOutProcess>,
    Arc<AtomicBool>,
) {
    // Set this to start the test audio
    let play_audio_flag = Arc::new(AtomicBool::new(false));

    let ac = make_jack_client_port(
        client_name,
        port_name,
        audio_buffer.to_vec(),
        play_audio_flag.clone(),
    );
    (ac, play_audio_flag)
}
