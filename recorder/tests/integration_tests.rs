// Copyright (c) 2025 Worik Turei Stanton
// License: GPL-3.0

use jack::{AsyncClient, AudioOut, Client, Control, Port, ProcessHandler, ProcessScope};
use qzn3t_recorder::{
    app::{App, AppData},
    structs::Command,
};
use std::{
    env::temp_dir,
    fs::File,
    io::Read,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, channel},
    },
    thread::{self, sleep},
    time::Duration,
};

/// Generate one second of sin wave
fn generate_sin_wave(frequency: u16, volume: f32, sample_rate: u32) -> Vec<f32> {
    let duration_samples = sample_rate as usize;
    let angular_frequency = 2.0 * std::f32::consts::PI * frequency as f32 / sample_rate as f32;

    (0..duration_samples)
        .map(|i| (angular_frequency * i as f32).sin() * volume)
        .collect()
}

/// A test client that plays an audio buffer when the `run_flag` is
/// set.  It resets `run_flag` when it is finished
/// Usage:
/// ```
/// let audio_buffer = generate_sin_wave(220, 0.75, 48_000);
/// let run_flag = Arc::new(AtomicBool::new(false));
/// let ac = make_jack_client_port("client_name", "port_name", audio_buffer, fun_flag.clone());
/// <Connect the port up to the recorder to test>
///     run_flag.store(true, Ordering::SeqCst);
/// sleep(Duration::from_secs(1)); // Wait for sin wave to play
/// assert!(!run_flag.load(Ordering::SeqCst));
/// ```
///
struct OutProcess {
    audio_buffer: Vec<f32>,
    output: Port<AudioOut>,
    position: usize,
    run_flag: Arc<AtomicBool>,
}
impl ProcessHandler for OutProcess {
    fn process(&mut self, _c: &Client, ps: &ProcessScope) -> Control {
        {
            if !self.run_flag.load(Ordering::SeqCst) {
                // Not playing
                return Control::Continue;
            }
        }
        let output = self.output.as_mut_slice(ps);
        let frames = output.len();
        let samples: &Vec<f32> = &self.audio_buffer;

        for (i, _) in (0..frames).enumerate() {
            if self.position >= samples.len() {
                self.run_flag.store(false, Ordering::SeqCst);
                self.position = 0;
                println!("DBG Return from OutProcess: Position {}", self.position);
                return Control::Continue;
            }
            let sample = samples[self.position];
            output[i] = sample;
            self.position += 1;
        }
        Control::Continue
    }
}
pub struct Notifications;
impl jack::NotificationHandler for Notifications {}
fn make_jack_client_port(
    client_name: &str,
    port_name: &str,
    audio_buffer: Vec<f32>,
    run_flag: Arc<AtomicBool>,
) -> AsyncClient<Notifications, OutProcess> {
    let (client, _status) =
        match jack::Client::new(client_name, jack::ClientOptions::NO_START_SERVER) {
            Ok(cs) => cs,
            Err(err) => panic!("Failed creating test client {client_name}: {err}"),
        };
    let output = match client.register_port("outout", AudioOut::default()) {
        Ok(p) => p,
        Err(err) => panic!("Cannot create output port {port_name} for client {client_name}. {err}"),
    };

    let out_process = OutProcess {
        audio_buffer,
        output,
        position: 0,
        run_flag: run_flag.clone(),
    };
    match client.activate_async(Notifications, out_process) {
        Ok(ac) => ac,
        Err(err) => panic!("Cannot create async for client {client_name}. {err}"),
    }
}
#[allow(dead_code)]
//#[test]
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

/// Generate a sin vave in a buffer
/// Send it to the recorder with a file and directory specified.
/// Read the recorded buffer from the file system
/// Compare it to original sin wave
#[test]
fn record_audio_to_file() {
    let audio_buffer = generate_sin_wave(220, 0.75, 48_000);
    let run_flag = Arc::new(AtomicBool::new(false));
    let port = "output";
    let client_name = "integration_test";
    let ac = make_jack_client_port(client_name, port, audio_buffer.clone(), run_flag.clone());
    let port_name = format!("{}:{port}", ac.as_client().name());

    // Set up recorder
    // These two channels will not be used but they are required to build `AppData`
    // Channel to send audio data to Jackd
    let (audio_tx, _audio_rx) = mpsc::channel::<f32>();
    // The app is controlled through a channel with the front end UI
    let (_command_tx, command_rx) = mpsc::channel::<Command>();
    // The main programme runs in `App`
    let mut dir = temp_dir();
    dir.push("sinwave.raw");
    let file_name = match dir.as_path().to_str() {
        Some(f) => f,
        None => panic!("Cannot convert {dir:?} to string"),
    };
    let mut app = App::new();
    let mut app_data: AppData =
        match app.initialise(audio_tx, command_rx, &port_name, file_name.into(), true) {
            Ok(a) => a,
            Err(err) => panic!("Cannot initalise AppData: {err}"),
        };
    if let Err(err) = app_data.handle_recording() {
        panic!("Called handle_recording(): {err}");
    }
    // <Connect the port up to the recorder to test>
    run_flag.store(true, Ordering::SeqCst);
    sleep(Duration::from_secs(1)); // Wait for sin wave to play
    // Test that Jack Client has shut down.
    assert!(!run_flag.load(Ordering::SeqCst));
    if let Err(err) = app_data.handle_audio_stop() {
        panic!("Could not stop audio: {err}");
    }
    let new_buffer = app_data.recorded_audio.clone();

    // Check recorded data is same as submitted data
    assert_eq!(new_buffer.len(), audio_buffer.len());
    for i in 0..audio_buffer.len() {
        assert!((audio_buffer[i] - new_buffer[i]).abs() < f32::EPSILON);
    }

    // Save the data
    if let Err(err) = app_data.handle_save() {
        panic!("Could not save audio data: {err}");
    }

    // Read it back and compare it
    let mut file = match File::open(file_name) {
        Ok(f) => f,
        Err(err) => panic!("Could not open file: {file_name}: {err}"),
    };

    let mut buffer = Vec::new();
    _ = file.read_to_end(&mut buffer);
    let f32_vec: Vec<f32> = buffer
        .chunks_exact(std::mem::size_of::<f32>())
        .map(|chunk| {
            let bytes: [u8; 4] = chunk.try_into().unwrap();
            f32::from_ne_bytes(bytes)
        })
        .collect();
    assert!(f32_vec.len() == audio_buffer.len());
    for i in 0..audio_buffer.len() {
        assert!((audio_buffer[i] - f32_vec[i]).abs() < f32::EPSILON);
    }
}
