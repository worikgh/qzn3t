// Copyright (c) 2025 Worik Turei Stanton
// License: GPL-3.0

use jack::{AsyncClient, AudioOut, Client, Control, Port, ProcessHandler, ProcessScope};
use qzn3t_recorder::{
    app::{App, AppData},
    io::Inputs,
    structs::Command,
    utils::get_sample_rate,
};

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
///
struct TestAudioOutProcess {
    audio_buffers: Vec<Vec<f32>>,
    outputs: Vec<Port<AudioOut>>,
    position: usize,
    play_audio_flag: Arc<AtomicBool>,
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

        // All the audi buffers must be the same length
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
                self.play_audio_flag.store(false, Ordering::SeqCst);
            }

            if self.play_audio_flag.load(Ordering::SeqCst) {
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
/// output port `port_name` and when the flag `play_audio_flag` is set
/// it sends the contents of `audio_buffer` to the pipe.
fn make_jack_client_port(
    client_name: &str,
    port_names: Vec<&str>,
    audio_buffers: Vec<Vec<f32>>,
    play_audio_flag: Arc<AtomicBool>,
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

/// Generate a sine wave in a buffer
/// Send it to the recorder from a Jackd client
/// Check the recorded audio is essentially the same

#[test]
/// Multi-channel processing
fn record_two_channels() {
    // The test audio
    let audio_duration_ms = 1;
    let audio_buffer_sine = generate_test_audio(220, 0.25, audio_duration_ms, WaveForm::Sine);
    let audio_buffer_sine = trim_audio(&audio_buffer_sine);
    let audio_buffer_tri = generate_test_audio(220, 0.25, audio_duration_ms, WaveForm::Triangle);
    let audio_buffer_tri = trim_audio(&audio_buffer_tri);

    // Client to play the output
    let port_sine = "sine-wave";
    let port_sine_label = "Sine wave";
    let port_tri = "tri-wave";
    let port_tri_label = "Triangle wave";
    let client_name = "integration_test";
    let (ac, play_audio_flag) = play_test_audio(
        client_name,
        vec![port_sine, port_tri],
        vec![&audio_buffer_sine, &audio_buffer_tri],
        // vec![port_tri, port_sine],
        // vec![&audio_buffer_tri, &audio_buffer_sine],
    );

    // Set up the recorder
    // The inputs (Jack pipes to record) first
    let mut inputs = Inputs::new();
    let port_tri = format!("{}:{port_tri}", ac.as_client().name());
    if let Err(err) = inputs.add_name(&port_tri, port_tri_label) {
        panic!("{err}");
    }
    let port_sine = format!("{}:{port_sine}", ac.as_client().name());
    if let Err(err) = inputs.add_name(&port_sine, port_sine_label) {
        panic!("{err}");
    }
    let mut app_data = set_up_recorder_inputs(inputs);

    // Start the recorder.
    if let Err(err) = app_data.handle_recording() {
        panic!("Called handle_recording(): {err}");
    }

    // Start the test signal
    play_audio_flag.store(true, Ordering::SeqCst);

    // Wait for audio to stop
    thread::sleep(Duration::from_millis(audio_duration_ms as u64));
    loop {
        if !play_audio_flag.load(Ordering::SeqCst) {
            break;
        }
        eprintln!("Waiting for audio to stop");
        thread::sleep(Duration::from_millis(100));
    }

    if let Err(err) = app_data.handle_audio_stop() {
        panic!("Could not stop audio: {err}");
    }

    // Get two recorded buffers
    dbg!(app_data.recorded_audio.keys().collect::<Vec<&String>>());
    let rec_tri = app_data.recorded_audio.get(port_tri_label).unwrap();
    eprintln!("Trim tri:");
    let rec_tri = trim_audio(rec_tri);

    let rec_sine = app_data.recorded_audio.get(port_sine_label).unwrap();
    eprintln!("Trim sine:");
    let rec_sine = trim_audio(rec_sine);

    dbg!(rec_sine.len());
    dbg!(audio_buffer_sine.len());
    dbg!(rec_tri.len());
    dbg!(audio_buffer_tri.len());

    let mut result = true;

    // The two buffers must be identical
    if rec_sine.len() != audio_buffer_sine.len() {
        eprintln!("Sine lengths differ");
        result = false;
    }
    dbg!(&rec_sine[0..7]);
    dbg!(&rec_tri[0..7]);
    for i in 0..audio_buffer_sine.len().min(rec_sine.len()) {
        eprintln!(
            "Sine at {i} {:0.4}/{:0.4}",
            rec_sine[i], audio_buffer_sine[i]
        );
        if (rec_sine[i] - audio_buffer_sine[i]).abs() > f32::EPSILON {
            eprintln!("Sine differs at {i}");
            result = false;
            break;
        }
    }
    if rec_tri.len() != audio_buffer_tri.len().min(rec_tri.len()) {
        eprintln!("Triangle lengths differ");
        result = false;
    }

    for i in 0..audio_buffer_tri.len().min(rec_tri.len()) {
        if (rec_tri[i] - audio_buffer_tri[i]).abs() > f32::EPSILON {
            eprintln!("Triangle differs at {i}");
            result = false;
            break;
        }
    }
    assert!(result);
}

#[test]
fn record_audio() {
    let sample_rate = get_sample_rate();

    // The test audio
    let audio_buffer = generate_test_audio(220, 0.25, 1_000, WaveForm::Triangle);
    let audio_buffer = trim_audio(&audio_buffer);

    let port_name = "out put"; // Spaces are allowed in pipe names.
    let client_name = "integration_test";
    let (ac, play_audio_flag) = play_test_audio(client_name, vec![port_name], vec![&audio_buffer]);
    let port_name = format!("{}:{port_name}", ac.as_client().name());
    // Set up recorder
    let mut app_data = set_up_recorder(vec![port_name.clone()]);

    // Record data from `port_name`
    if let Err(err) = app_data.handle_recording() {
        panic!("Called handle_recording(): {err}");
    }

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
    let new_buffer = trim_audio(app_data.recorded_audio.get(port_name.as_str()).unwrap());

    // The buffers should be the same length
    assert_eq!(audio_buffer.len(), new_buffer.len());

    // The buffers should be the same exactly
    for i in 0..audio_buffer.len() {
        assert!((audio_buffer[i] - new_buffer[i]).abs() < f32::EPSILON);
    }
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
    let (ac, play_audio_flag) = play_test_audio(client_name, vec![port_name], vec![&test_audio]);
    let port_name = format!("{}:{port_name}", ac.as_client().name());

    // Set up the recorder to test and start recording
    let mut app_data = set_up_recorder(vec![port_name]);
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
    let (ac_1, play_audio_flag_1) =
        play_test_audio(client_name_1, vec![port_name], vec![&test_audio]);
    let port_name_1 = format!("{}:{port_name}", ac_1.as_client().name());
    let (ac_2, play_audio_flag_2) = play_test_audio(
        client_name_2,
        vec![port_name],
        vec![app_data.recorded_audio.get(&port_name_1).unwrap()],
    );
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
    let mut dbg = 0;
    while result.last().is_some_and(|&x| x.abs() < f32::EPSILON) {
        result.pop();
        dbg += 1;
    }
    eprintln!("Trimmed {dbg} trailing zeros");
    result
}

/// Seting up the recorder for testing
fn set_up_recorder_inputs(inputs: Inputs) -> AppData {
    // Set up recorder
    // These two channels will not be used but they are required to build `AppData`
    let (_audio_tx, _audio_rx) = mpsc::channel::<f32>();
    let (_command_tx, _command_rx) = mpsc::channel::<Command>();

    // Set up output directory to save audio in Not used in this test but
    // there must be an output file for a `recorder`
    let mut dir = temp_dir();
    dir.push("sinewave.raw");
    let directory = match dir.as_path().to_str() {
        Some(f) => f,
        None => panic!("Cannot convert {dir:?} to string"),
    };

    let mut app = App;
    match app.initialise(_audio_tx, _command_rx, inputs, Some(directory.into()), true) {
        Ok(a) => a,
        Err(err) => panic!("Cannot initalise AppData: {err}"),
    }
}

fn set_up_recorder(port_names: Vec<String>) -> AppData {
    let mut inputs = Inputs::new();
    for p in port_names.iter() {
        inputs.add(p).unwrap();
    }
    set_up_recorder_inputs(inputs)
}

/// Output some audio through a new Jack client.  Return the async
/// Jack client and flag that starts the audio playing.  The audio
/// player will reset the flag when it is finished (resetting the flag
/// will stop the audio playing)
fn play_test_audio(
    client_name: &str,
    port_names: Vec<&str>,
    audio_data: Vec<&[f32]>,
) -> (
    AsyncClient<Notifications, TestAudioOutProcess>,
    Arc<AtomicBool>,
) {
    // Set this to start the test audio
    let play_audio_flag = Arc::new(AtomicBool::new(false));

    let ac = make_jack_client_port(
        client_name,
        port_names,
        audio_data
            .iter()
            .map(|b| b.to_vec())
            .collect::<Vec<Vec<f32>>>(),
        play_audio_flag.clone(),
    );
    (ac, play_audio_flag)
}
