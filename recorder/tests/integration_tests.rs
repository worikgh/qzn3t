// Copyright (c) 2025 Worik Turei Stanton
// License: GPL-3.0

use jack::{
    AsyncClient, AudioIn, AudioOut, Client, Control, NotificationHandler, Port, PortFlags,
    ProcessHandler, ProcessScope,
};
use qzn3t_recorder::{
    app::{App, AppData},
    errors::RecorderError,
    io::{JackPipes, read_f32_vec_from_file},
    send_audio_to_jack::send_audo_to_jack,
    structs::Command,
    utils::get_sample_rate,
};

use std::{
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};

#[derive(Debug)]
enum WaveForm {
    Sine,
    Triangle,
    Square,
}

// There is a timing bug
// Using `cargo test play_three_channels`
// `play_three_channels` fails when AUDIO_DURATION <=  95
// `play_three_channels` pass  when AUDIO_DURATION >=  99
// Using `cargo test`
// AUDIO_DURATION must be approximately at least 1_000 to pass all
// tests.  But it is very sensitive, non-linear and intermittent

/// The length of the test audio buffers in ms
const AUDIO_DURATION: u32 = 1_900;

/// Generate a buffer of mono audio samples.
///
/// * `frequency` – Frequency of the tone in Hz.
/// * `volume`    – Linear gain (0.0‑1.0).
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
    let angular_frequency = 2.0 * PI * frequency as f32 / sample_rate as f32;
    match wave_form {
        WaveForm::Sine => {
            // sine(2πft)
            (0..duration_samples)
                .map(|i| (angular_frequency * i as f32).sin() * volume)
                .collect()
        }

        WaveForm::Square => {
            // square wave: sign(sin(2πft))
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
    active: Arc<AtomicBool>,
}

impl ProcessHandler for TestAudioOutProcess {
    fn process(&mut self, _c: &Client, ps: &ProcessScope) -> Control {
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

/// Create a Jack client to sink audio to test playing.  A port and
/// a buffer for each audio channel.  The client simulates playing
/// audio by writing it to the buffer.  The buffers are shared with
/// an Arc<Mutex<_>> so they can then be examined to check the
/// audio made it
fn make_test_play_client(
    name: &str,
    port_names: Vec<String>,
    buffers: Vec<Arc<Mutex<Vec<f32>>>>,
) -> Result<AsyncClient<TestPlayNotificationHandler, TestPlayProcessHandler>, RecorderError> {
    let (_client, _) =
        Client::new(name, jack::ClientOptions::NO_START_SERVER).expect("Cannot make Jack sink");

    let mut ports = vec![];
    for p in port_names.iter() {
        let port = _client
            .register_port(p, AudioIn::default())
            .expect("Creating port");
        ports.push(port);
    }

    let notification_handler = TestPlayNotificationHandler;
    let process_handler = TestPlayProcessHandler { buffers, ports };
    let ac = _client
        .activate_async(notification_handler, process_handler)
        .unwrap();
    Ok(ac)
}

/// Create a source for testing.  Creates a client `client_name` with
/// output ports from `port_names` and when the flag `play_audio_f` is
/// set it sends the contents of `audio_buffer` to the pipe.  When the
///  audio is played `play_audio_f` is reset
fn make_jack_client_port(
    client_name: &str,
    port_names: Vec<&str>,
    audio_buffers: Vec<Vec<f32>>,
    play_audio_f: Arc<AtomicBool>,
) -> AsyncClient<Notifications, TestAudioOutProcess> {
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

/// Test playing back audio.  Generate three channels of audio: a
/// square, sine and triangle wave.  Create a Jack
/// client with three inputs to act as the sink.  Generally in normal
/// use this would be system:playback_1, system:playback_2 and
/// system:playback_3 (or whateverpipes lead to sound hardware)  But in this case the audio needs to be
/// captured and compared with the original.
struct TestPlayNotificationHandler;
impl NotificationHandler for TestPlayNotificationHandler {}
struct TestPlayProcessHandler {
    ports: Vec<Port<AudioIn>>,
    /// A buffer for each port, shared with caller for verifying test
    buffers: Vec<Arc<Mutex<Vec<f32>>>>,
}
impl ProcessHandler for TestPlayProcessHandler {
    fn process(&mut self, _: &Client, ps: &ProcessScope) -> Control {
        for (idx, p) in self.ports.iter().enumerate() {
            let t = p.as_slice(ps);
            self.buffers[idx].lock().unwrap().extend_from_slice(t);
        }
        Control::Continue
    }
}
#[test]
fn play_three_channels() {
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
    let _ac = make_test_play_client(client_name, port_names, buffers).unwrap();
    let port_names = _ac
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

/// Generate two audio tracks: sine and triangle waves.  Record both
/// simultaneously and save them both to disc files.  Use the
/// generated audio file to test the command line interface for
/// playing back files
#[test]
fn record_two_channels_and_play_back() {
    // The test audio

    let audio_duration_ms = AUDIO_DURATION;

    let audio_buffer_sine = generate_test_audio(220, 0.25, AUDIO_DURATION, WaveForm::Sine);
    let audio_buffer_sine = trim_audio(&audio_buffer_sine);

    let audio_buffer_tri = generate_test_audio(220, 0.25, AUDIO_DURATION, WaveForm::Triangle);
    let audio_buffer_tri = trim_audio(&audio_buffer_tri);

    // Directory recorded audio is sent to
    let output_path = dst_dir().join("two_channels");

    // Client to play the output:
    let port_sine = "sine-wave";
    let port_tri = "tri-wave";

    let client_name = "integration_test";
    let (ac, play_audio_f) = play_test_audio(
        client_name,
        vec![port_sine, port_tri],
        vec![&audio_buffer_sine, &audio_buffer_tri],
    );

    // Set up the recorder
    // The inputs (Jack pipes to record) first
    let mut inputs = JackPipes::new(true);
    let port_tri = format!("{}:{port_tri}", ac.as_client().name());
    if let Err(err) = inputs.add(&port_tri) {
        panic!("{err}");
    }
    let port_sine_complete = format!("{}:{port_sine}", ac.as_client().name());
    if let Err(err) = inputs.add(&port_sine_complete) {
        panic!("{err}");
    }
    let mut recorder = set_up_recorder(vec![port_sine_complete, port_tri], &output_path);

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
    let rec_sine = recorder.recorded_audio.get_buffer(0).unwrap();
    let rec_sine = trim_audio(&rec_sine);
    let rec_tri = recorder.recorded_audio.get_buffer(1).unwrap();
    let rec_tri = trim_audio(&rec_tri);

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
                eprintln!(
                    "Fail: Sine differs at {i}, {:0.4}, {:0.4}",
                    rec_sine[i], audio_buffer_sine[i]
                );
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
        let audio_path = output_path.with_extension("raw");
        match read_f32_vec_from_file(&audio_path, 2) {
            Ok(audio_buffer) => {
                let recovered_sine = audio_buffer.get_buffer(0).unwrap();
                let recovered_sine = trim_audio(&recovered_sine);
                let recovered_tri = trim_audio(&audio_buffer.get_buffer(1).unwrap());
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
                eprintln!("Fail: Cannot read data from {audio_path:?}.  Error: {err}");
                result = false;
            }
        };
    }
    if result {
        // Test playing back the audio files from the previous test

        let buffers = vec![
            Arc::new(Mutex::new(Vec::<f32>::new())),
            Arc::new(Mutex::new(Vec::new())),
        ];
        let buffers_new = buffers.to_vec();
        let port_names = vec!["playback_1".to_string(), "playback_2".to_string()];

        let client_name = "test_play_client";
        let _ac = make_test_play_client(client_name, port_names, buffers).unwrap();
        let port_names = _ac
            .as_client()
            .ports(Some(client_name), None, PortFlags::IS_INPUT);

        let mut outputs = JackPipes::new(false);
        for p in port_names.iter() {
            outputs.add(p).unwrap();
        }
        let (_audio_tx, _audio_rx) = mpsc::channel::<f32>();
        let (_command_tx, _command_rx) = mpsc::channel::<Command>();

        let mut app = App;
        let mut app_data =
            match app.initialise_ui(_command_rx, JackPipes::new(true), outputs, &output_path) {
                Ok(a) => a,
                Err(err) => panic!("Cannot initalise AppData: {err}"),
            };
        app_data.handle_kommand(Command::Play).unwrap();

        // Check the buffers are the same
        let audio_buffers = read_f32_vec_from_file(&output_path.with_extension("raw"), 2).unwrap();
        let ab_0 = trim_audio(audio_buffers.get_buffer_idx(0).unwrap());
        {
            // let bf_0: std::sync::MutexGuard<'_, Vec<f32>> =
            let bf_0 = trim_audio(&buffers_new[0].lock().unwrap());
            if ab_0.len() != bf_0.len() {
                result = false;
            } else {
                for idx in 0..ab_0.len() {
                    let a = ab_0[idx];
                    let b = bf_0[idx];
                    if (a - b).abs() >= f32::EPSILON {
                        result = false;
                        eprintln!("channel 0: Failed match @ {idx}: Saved: {a}  Buffered: {b}");
                        break;
                    }
                }
            }
        }
        let ab_1 = trim_audio(audio_buffers.get_buffer_idx(1).unwrap());
        {
            let bf_1 = trim_audio(&buffers_new[1].lock().unwrap());
            if ab_1.len() != bf_1.len() {
                result = false;
            } else {
                for idx in 0..ab_1.len() {
                    let a = ab_1[idx];
                    let b = bf_1[idx];
                    if (a - b).abs() >= f32::EPSILON {
                        eprintln!("channel 1: Failed match @ {idx}: Saved: {a}  Buffered: {b}");
                    }
                }
            }
        }
    }
    assert!(result);
}

#[test]
/// Record one channel of audio.
/// Save it to disc.
fn record_audio() {
    let duration_ms = AUDIO_DURATION;

    // The test audio
    let audio_buffer = generate_test_audio(220, 0.25, duration_ms, WaveForm::Sine);
    let audio_buffer = trim_audio(&audio_buffer);

    // The Jack client playing the test audio to be recorded
    let port_name = "record_audio";
    let client_name = "integration_test";
    let (ac, play_audio_flag) = play_test_audio(client_name, vec![port_name], vec![&audio_buffer]);

    // The port to record audio data from
    let port_name_complete = format!("{}:{port_name}", ac.as_client().name());

    // File path for recorded audio.  There will be two files with
    // suffixes "raw" and "json" for audio data and metadata
    // respectively
    let output_path = dst_dir().join("record_audio");

    // Set up recorder
    let mut recorder = set_up_recorder(vec![port_name_complete.clone()], &output_path);

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
    let new_buffer = trim_audio(&recorder.recorded_audio.get_buffer(0).unwrap());
    // The buffers should be the same length
    assert_eq!(audio_buffer.len(), new_buffer.len());

    // The buffers should be the same exactly
    for i in 0..audio_buffer.len() {
        assert!((audio_buffer[i] - new_buffer[i]).abs() < f32::EPSILON);
    }
    // Check the saved data
    let mut result = true;
    match read_f32_vec_from_file(&output_path.with_extension("raw"), 1) {
        Ok(d) => {
            let imported_data = trim_audio(&d.get_buffer(0).unwrap());
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
        Err(err) => panic!("Failed to read data from {output_path:?}.  Error: {err}"),
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
fn set_up_recorder(port_names: Vec<String>, dir: &Path) -> AppData {
    let mut inputs = JackPipes::new(true);
    let outputs = JackPipes::new(false);
    for p in port_names.iter() {
        inputs.add(p).unwrap();
    }
    let (_audio_tx, _audio_rx) = mpsc::channel::<f32>();
    let (_command_tx, _command_rx) = mpsc::channel::<Command>();

    let mut app = App;
    match app.initialise(inputs, outputs, dir) {
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
    std::env::current_dir().unwrap().join("tests/data")
}
