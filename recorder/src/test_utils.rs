// Copyright (c) 2026 Worik Turei Stanton
// License: GPL-3.0

pub mod common {

    use crate::{
        app::{App, AppData},
        errors::RecorderError,
        io::JackPipes,
        utils::get_sample_rate,
    };
    use jack::{
        AsyncClient, AudioIn, AudioOut, Client, Control, NotificationHandler, Port, ProcessHandler,
        ProcessScope,
    };
    use tempfile::TempDir;

    use std::{
        path::{Path, PathBuf},
        process::Command,
        sync::{
            Arc, Mutex,
            atomic::{AtomicBool, Ordering},
            mpsc,
        },
        thread,
        time::Duration,
    };

    #[derive(Debug)]
    #[allow(dead_code)]
    pub enum WaveForm {
        Sine,
        Triangle,
        Square,
    }

    /// Generate a buffer of mono audio samples.
    ///
    /// * `frequency` – Frequency of the tone in Hz.
    /// * `volume`    – Linear gain (0.0‑1.0).
    /// * `duration_ms` – Length of the buffer in milliseconds.
    /// * `wave_form` – Desired waveform.
    pub fn generate_test_audio(
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

    /// Remove leading and trailing zeros from an audio buffer
    pub fn trim_audio(audio_buffer: &[f32]) -> Vec<f32> {
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

    /// Create a Jack client to sink audio to test playing.  A port
    /// and a buffer for each audio channel.  The client simulates
    /// playing audio by writing it to the buffer.  The buffers are
    /// shared with an Arc<Mutex<_>> so they can then be examined to
    /// check the audio captured and compare with the original.
    pub struct TestPlayNotificationHandler;
    impl NotificationHandler for TestPlayNotificationHandler {}

    pub struct TestPlayProcessHandler {
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
    pub fn make_test_play_client(
        name: &str,
        port_names: Vec<String>,
        buffers: Vec<Arc<Mutex<Vec<f32>>>>,
    ) -> Result<AsyncClient<TestPlayNotificationHandler, TestPlayProcessHandler>, RecorderError>
    {
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

    #[derive(Debug)]
    pub struct TestAudioOutProcess {
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
                // Loop audio if necessary
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

    #[allow(dead_code)]
    pub struct Notifications;
    impl jack::NotificationHandler for Notifications {}

    /// Create a source for testing.  Creates a client `client_name` with
    /// output ports from `port_names` and when the flag `play_audio_f` is
    /// set it sends the contents of `audio_buffer` to the pipe.  When the
    ///  audio is played `play_audio_f` is reset
    #[allow(dead_code)]
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

        dbg!(&outputs);
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

    /// Set up a recorder for testing
    #[allow(dead_code)]
    pub fn set_up_recorder(port_names: Vec<String>, dir: &Path) -> AppData {
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
    #[allow(dead_code)]
    pub fn play_test_audio(
        client_name: &str,
        port_names: Vec<&str>,
        audio_data: Vec<&[f32]>,
    ) -> (
        AsyncClient<Notifications, TestAudioOutProcess>,
        Arc<AtomicBool>,
    ) {
        // Exactly one port for each buffer
        assert_eq!(port_names.len(), audio_data.len());
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

    /// The destination directory.  Hard coded into repository/crate structure
    #[allow(dead_code)]
    pub fn dst_dir() -> PathBuf {
        std::env::current_dir().unwrap().join("tests/data")
    }

    // Helper function to create test directory
    pub fn setup_test_dir() -> TempDir {
        tempfile::tempdir().expect("Failed to create temp dir")
    }
}
