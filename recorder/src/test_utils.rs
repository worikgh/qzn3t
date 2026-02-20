// Copyright (c) 2026 Worik Turei Stanton
// License: GPL-3.0

pub mod common {

    use crate::{
        app::{App, AppData},
        errors::RecorderError,
        io::{AudioBuffers, JackPipes},
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
        Geometric,
        Linear,
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
            // The samples go from 1.0 and go linearly to -1.0 over
            // the duration.
            WaveForm::Linear => vec![volume; duration_samples as usize],
            WaveForm::Geometric => (0..duration_samples)
                .map(|n| 1.0 - 2.0 * n as f32 / duration_samples as f32)
                .collect(),
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
        zeros_sent: Arc<Mutex<Vec<u32>>>,
        non_zeros_sent: Arc<Mutex<Vec<u32>>>,
        audio_buffers: AudioBuffers,
    }
    impl Drop for TestPlayProcessHandler {
        fn drop(&mut self) {
            for c in 0..self.audio_buffers.channels() {
                let dbg = format!(
                    "TestPlayProcessHandler buf: {c}: {}",
                    describe_linear_buffer(
                        self.audio_buffers
                            .get_buffer_idx(c.try_into().unwrap())
                            .unwrap()
                    )
                );
                dbg!(dbg);
            }
        }
    }
    impl ProcessHandler for TestPlayProcessHandler {
        fn process(&mut self, _: &Client, ps: &ProcessScope) -> Control {
            for (idx, p) in self.ports.iter().enumerate() {
                let samples = p.as_slice(ps);
                for s in samples.iter() {
                    if s.abs() <= f32::EPSILON {
                        self.zeros_sent.lock().unwrap()[idx] += 1;
                    } else {
                        self.non_zeros_sent.lock().unwrap()[idx] += 1;
                    }
                }
                self.audio_buffers
                    .get_buffer_mut(idx.try_into().unwrap())
                    .unwrap()
                    .extend_from_slice(samples);
                self.buffers[idx].lock().unwrap().extend_from_slice(samples);
            }
            Control::Continue
        }
    }

    /// Set up a client (named `name`) with ports (names in
    /// `port_names`) that reads audio data form the ports and write
    /// it into shared buffers (`buffers`).  This is for testing,
    /// writing the buffers in lieu of sending to audio hardware.  The
    /// buffers can be examined to check what data would have been
    /// sent to audio hardware.
    #[allow(clippy::type_complexity)]
    pub fn make_test_play_client(
        name: &str,
        port_names: Vec<String>,
        buffers: Vec<Arc<Mutex<Vec<f32>>>>,
    ) -> Result<
        (
            AsyncClient<TestPlayNotificationHandler, TestPlayProcessHandler>,
            // This counts zeros received per audio channel
            Arc<Mutex<Vec<u32>>>,
            // This counts non-zeros received per audio channel
            Arc<Mutex<Vec<u32>>>,
        ),
        RecorderError,
    > {
        assert_eq!(port_names.len(), buffers.len());
        let (client, _) = Client::new(name, jack::ClientOptions::NO_START_SERVER)
            .expect("make_test_play_client: Cannot make Jack client");

        let mut ports = vec![];
        for p in port_names.iter() {
            let port = client
                .register_port(p, AudioIn::default())
                .expect("Creating port");
            ports.push(port);
        }

        let notification_handler = TestPlayNotificationHandler;
        let channels_count = port_names.len();
        let zeros_sent = Arc::new(Mutex::new(vec![0u32; channels_count]));
        let non_zeros_sent = Arc::new(Mutex::new(vec![0u32; channels_count]));
        let channel_count = ports.len();
        let process_handler = TestPlayProcessHandler {
            buffers,
            ports,
            zeros_sent: zeros_sent.clone(),
            non_zeros_sent: non_zeros_sent.clone(),
            audio_buffers: AudioBuffers::new_channels(channel_count),
        };
        let ac = client
            .activate_async(notification_handler, process_handler)
            .unwrap();
        Ok((ac, zeros_sent, non_zeros_sent))
    }

    #[derive(Debug)]
    pub struct TestAudioOutProcess {
        audio_buffers: Vec<Vec<f32>>,
        outputs: Vec<Port<AudioOut>>,
        position: usize,
        play_audio_f: Arc<AtomicBool>,
        active: Arc<AtomicBool>,

        /// Debugging the bug where runs of zeros are inserted, mostly
        /// to one channel.  COuld the zeros and non zeros sent to
        /// each channel
        zeros_sent: Arc<Mutex<Vec<u32>>>,
        non_zeros_sent: Arc<Mutex<Vec<u32>>>,
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
            assert!(frames.windows(2).all(|w| w[0] == w[1]) || frames.len() <= 1);
            let frames: usize = *frames.first().unwrap();

            // All the audio buffers must be the same length
            assert!(
                self.audio_buffers
                    .first()
                    .is_some_and(|first| self.audio_buffers.iter().all(|x| x.len() == first.len()))
            );
            let slen = self.audio_buffers[0].len();
            for j in 0..frames {
                // Loop audio if necessary
                if self.position >= slen {
                    self.position = 0;
                    self.play_audio_f.store(false, Ordering::SeqCst);
                }

                if self.play_audio_f.load(Ordering::SeqCst) {
                    for (i, out) in outputs.iter_mut().enumerate().take(olen) {
                        let sample = self.audio_buffers[i][self.position];
                        out[j] = sample;
                        let mut zeros_sent = self.zeros_sent.lock().unwrap();
                        let mut non_zeros_sent = self.non_zeros_sent.lock().unwrap();
                        if sample.abs() <= f32::EPSILON {
                            zeros_sent[i] += 1;
                        } else {
                            non_zeros_sent[i] += 1;
                        }
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

        let out_process = TestAudioOutProcess {
            audio_buffers,
            outputs,
            position: 0,
            play_audio_f,
            active: active_flag.clone(),
            zeros_sent: Arc::new(Mutex::new(vec![])),
            non_zeros_sent: Arc::new(Mutex::new(vec![])),
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

        match App::initialise(inputs, outputs, dir, true) {
            Ok(a) => a,
            Err(err) => panic!("Cannot initalise AppData: {err}"),
        }
    }

    /// Output some audio through a new Jack client.  Return the async
    /// Jack client and flag `play_audio_f` that controls the audio
    /// playing. Audio plays when `play_audio_f` is true and is stopped
    /// when false.
    #[allow(dead_code, clippy::type_complexity)]
    pub fn play_test_audio(
        client_name: &str,
        port_names: Vec<&str>,
        audio_data: Vec<&[f32]>,
    ) -> (
        AsyncClient<Notifications, TestAudioOutProcess>,
        Arc<AtomicBool>,
        Arc<Mutex<Vec<u32>>>,
        Arc<Mutex<Vec<u32>>>,
    ) {
        // Exactly one port for each buffer
        assert_eq!(port_names.len(), audio_data.len());
        let play_audio_f = Arc::new(AtomicBool::new(false));
        let audio_buffers = audio_data
            .iter()
            .map(|b| b.to_vec())
            .collect::<Vec<Vec<f32>>>();

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
        let channel_count = audio_buffers.len();
        let zeros_sent = Arc::new(Mutex::new(vec![0_u32; channel_count]));
        let non_zeros_sent = Arc::new(Mutex::new(vec![0_u32; channel_count]));
        let out_process = TestAudioOutProcess {
            audio_buffers,
            outputs,
            position: 0,
            play_audio_f: play_audio_f.clone(),
            active: active_flag.clone(),
            zeros_sent: zeros_sent.clone(),
            non_zeros_sent: non_zeros_sent.clone(),
        };
        let ac = match client.activate_async(Notifications, out_process) {
            Ok(ac) => {
                let mut activate_wait = 0;
                loop {
                    if !active_flag.load(Ordering::SeqCst) {
                        break;
                    }
                    thread::sleep(Duration::from_millis(1));
                    activate_wait += 1;
                    if activate_wait == 100 {
                        panic!("Could not activate client");
                    }
                }
                ac
            }
            Err(err) => panic!("Cannot create async for client {client_name}. {err}"),
        };
        (ac, play_audio_f, zeros_sent, non_zeros_sent)
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

    /// Find runs of zero in a buffer.  There is a bug where series of
    /// zeros are being added to the buffer written to by the client
    /// (created by `make_test_play_client` that sinks audio in shared
    /// buffers).  This function returns an analysis of the buffer
    /// returning the start index and length of series of zeros in the
    /// supplied buffer
    pub fn find_zeros(input: &[f32]) -> Vec<(usize, usize)> {
        let mut ret = vec![];
        let mut i = 0;

        while i < input.len() {
            if input[i].abs() < f32::EPSILON {
                let start = i;
                while i < input.len() && input[i].abs() < f32::EPSILON {
                    i += 1;
                }
                let length = i - start;
                if length > 1 {
                    ret.push((start, length));
                }
            } else {
                i += 1;
            }
        }

        ret
    }
    /// Testing zeros bug with constant (linear) buffers.  This function
    /// reports each run of numbers in the buffer, where it starts, and
    /// how long it is.  A "good" buffer will have one entry, starting at
    /// 0, and continuing for the entire length of the buffer
    pub fn describe_linear_buffer(buffer: &[f32]) -> String {
        let mut ret = "".to_string();
        if buffer.is_empty() {
            return ret;
        }
        let mut idx: usize = 0;
        let mut start = idx;
        let mut len = 0;
        let mut v = buffer[0];

        while idx < buffer.len() {
            let t = buffer[idx];
            if (t - v).abs() <= f32::EPSILON {
                len += 1;
            } else {
                // Value changed
                ret = format!("{ret}{v}:{start}:{len} ");
                start = idx;
                len = 1;
                v = t;
            }
            idx += 1;
        }
        ret = format!("{ret}{v}:{start}:{len} ");
        ret
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_zeros_detects_zero_runs_correctly() {
        let input: Vec<f32> = vec![
            1.0,
            0.0,
            0.0,  // zero run at index 1 length 2
            -0.0, // treated as zero (IEEE -0.0)
            2.5,
            3.0,
            0.0, // zero run at index 6 length 1
            0.0,
            0.0,                // run continues -> index 6 length 3
            -0.0,               // continues (index 6 length 4)
            4.0,                // non-zero
            f32::EPSILON / 2.0, // smaller than EPSILON => treated as zero
            0.0,
            5.0,
            0.0,
            1.0,
        ];

        // Expected runs:
        // - start 1, length 3 (indices 1..4 include 0.0,0.0,-0.0)
        // - start 6, length 4 (indices 6..10 include 0.0,0.0,-0.0,-0.0)
        // - start 11, length 2 (EPSILON/2 treated as zero)
        // - start 14, length 1, not counted
        let expected = vec![(1usize, 3usize), (6usize, 4usize), (11usize, 2usize)];

        let got = common::find_zeros(&input);
        assert_eq!(got, expected);
    }
}
