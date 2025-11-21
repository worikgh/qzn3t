// Copyright (c) 2025 Worik Turei Stanton
// License: GPL-3.0

//! The main container for the programme
use crate::errors::RecorderError;
use crate::mixer::AudioMixer;
use crate::structs::{AudioFormat, Command};
use crate::utils::{audio_to_flac, get_sample_rate};
use jack_rec;
use std::error::Error;
use std::fs;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::Duration;
#[allow(dead_code)]
#[derive(Clone)]

/// Hold the code that runs the programme
pub struct App;
impl App {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self
    }

    /// Set up the environment to run in
    pub fn initialise(
        &mut self,
        audio_tx: mpsc::Sender<f32>,
        command_rx: mpsc::Receiver<Command>,
        port: &str,
        file_name: String,
        use_raw: bool,
    ) -> Result<AppData, Box<dyn Error>> {
        let recorder_run = Arc::new(AtomicBool::new(true));
        let ui_run = Arc::new(AtomicBool::new(true));
        let r = recorder_run.clone();
        ctrlc::set_handler(move || {
            eprintln!("DBG qzn3t/recorder: Received Ctrl-C, shutting down gracefully...");
            r.store(false, Ordering::Relaxed);
        })
        .expect("Error qzn3t/recorder: setting Ctrl-C handler");
        Ok(AppData {
            recorded_audio: Vec::new(),
            audio_handle: None,
            recorded_dub: Vec::new(),
            audio_tx,
            command_rx,
            recorder_run,
            ui_run,
            jack_input: port.to_string(),
            file_name,
            format: if use_raw {
                AudioFormat::Raw
            } else {
                AudioFormat::Flac
            },
        })
    }

    /// Run the main loop that receives commands on a channel
    pub fn run(
        &mut self,
        mut config_app: AppData,
    ) -> Result<thread::JoinHandle<Result<(), RecorderError>>, Box<dyn Error>> {
        // The version of `self` used inside the loop

        let app_handle = thread::spawn(move || -> Result<(), RecorderError> {
            loop {
                let command = match config_app.command_rx.recv() {
                    Ok(s) => s,
                    Err(err) => {
                        eprintln!("Error recorder: Getting state: {err}");
                        break;
                    }
                };
                match command {
                    Command::Record => config_app.handle_recording()?,
                    Command::Stop => config_app.handle_audio_stop()?,
                    Command::ReviewRecord => config_app.handle_review_record()?,
                    Command::Dubing => config_app.handle_dubing()?,
                    Command::DubReview => config_app.handle_dub_review()?,
                    Command::DubAccept => config_app.handle_dub_accept()?,
                    Command::Save => config_app.handle_save()?,
                    Command::Continue => (),
                    Command::Quit => {
                        config_app.quit();
                        break;
                    }
                }
            }
            eprintln!("DBG recorder: Broken from main loop");
            Ok(())
        }); // Closure
        Ok(app_handle)
    }
}

/// Hold the data for the programme.
pub struct AppData {
    pub recorded_audio: Vec<f32>,
    recorded_dub: Vec<f32>,
    // TODO: Multi channel.  Need to return several channels at once
    // in JoinHandle, or join a thread per channel and have a
    // collection of handles.
    pub audio_handle: Option<thread::JoinHandle<Result<Vec<f32>, RecorderError>>>,
    // TODO: Multi channel.  Need a collection of channels.
    audio_tx: mpsc::Sender<f32>,
    command_rx: mpsc::Receiver<Command>,
    pub recorder_run: Arc<AtomicBool>,
    pub ui_run: Arc<AtomicBool>,
    file_name: String,
    format: AudioFormat,
    // Identify the Jackd source to record from
    // TODO: Multi-channel. There must be a collection of these
    jack_input: String,
}
impl AppData {
    /// Stop all the processes
    fn quit(&mut self) {
        _ = self.handle_audio_stop();
        self.ui_run.store(false, Ordering::SeqCst);
    }

    /// Spawn a thread to get audio data from a Jack portdata
    fn get_audio_from_jack(
        &mut self,
        // TODO: Multi-channel.  This should be a collection of
        // client/port pairs for input
        jack_input: String,
    ) -> Result<thread::JoinHandle<Result<Vec<f32>, RecorderError>>, RecorderError> {
        // Copy of the switch to turn the recorder off
        let recorder_run = self.recorder_run.clone();

        // Local switch that is set when recordr is ready and recording
        let active_1 = Arc::new(AtomicBool::new(false));
        let active_2 = active_1.clone();
        let result = Ok(thread::spawn(move || -> Result<Vec<f32>, RecorderError> {
            // Buffer and channel to get data on
            let mut audio_data: Vec<f32> = Vec::new();

            let (sender, receiver) = mpsc::channel::<f32>();

            let client = "qzn3t/recorder".to_string();
            let ac = match jack_rec::run_port(client, jack_input, sender, recorder_run.clone()) {
                Ok(p) => p,
                Err(err) => {
                    eprintln!("Error recorder: {err}: get audio");
                    return Err(err.into());
                }
            };

            loop {
                let itr = receiver.try_iter();
                active_2.store(true, Ordering::SeqCst);
                for b in itr {
                    audio_data.push(b);
                }
                if !recorder_run.load(Ordering::Relaxed) {
                    break;
                }
                thread::sleep(Duration::from_millis(10));
            }

            if let Err(err) = ac.deactivate() {
                Err(RecorderError::DeactivateClientFailed(format!("{err}")))
            } else {
                Ok(audio_data)
            }
        }));
        // Do not return until Jack client is active.  About 5ms in testing
        let mut stuck_guard = 0;
        const STUCK_LIMIT: u32 = 100;
        while !active_1.load(Ordering::SeqCst) {
            thread::sleep(Duration::from_millis(10));
            stuck_guard += 1;
            if stuck_guard == STUCK_LIMIT {
                panic!("qzn3t/recorder Timed out creating Jack client");
            }
        }
        result
    }

    pub fn handle_recording(&mut self) -> Result<(), Box<dyn Error>> {
        self.recorded_audio.truncate(0);
        self.recorder_run.store(true, Ordering::SeqCst);
        let port = self.jack_input.clone();
        // let client = "qzn3t".to_string();

        match self.get_audio_from_jack(port) {
            Ok(handle) => {
                self.audio_handle = Some(handle);
            }
            Err(err) => {
                eprintln!("Error recorder: Error from get_audio_from_jack");
                return Err(err.into());
            }
        };
        Ok(())
    }

    /// The command: stop
    pub fn handle_audio_stop(&mut self) -> Result<(), Box<dyn Error>> {
        // This ends the main loop

        self.recorder_run.store(false, Ordering::Relaxed);
        // TODO: Multi-track.  Either JoinHandle that holds a
        // collection of buffers or join a collectiion of JoinHandles
        // one per channel
        if let Some(handle) = self.audio_handle.take() {
            let j = handle.join();
            match j {
                Ok(Ok(audio_data)) => {
                    self.recorded_audio.extend(audio_data.iter());
                    Ok(())
                }
                Ok(Err(recorder_error)) => Err(recorder_error.into()),
                Err(err) => Err(format!("Error {err:?}").into()),
            }
        } else {
            Ok(())
        }
    }

    /// Play back the audio data
    fn handle_review_record(&mut self) -> Result<(), Box<dyn Error>> {
        self.recorder_run.store(true, Ordering::Relaxed);
        self.play_audio(&self.recorded_audio)
    }

    /// Play the contents of `recorded_audio` while recording separately
    fn handle_dubing(&mut self) -> Result<(), Box<dyn Error>> {
        Ok(())
    }

    /// Mix `recorded_audio` and `recorded_dub` and play it back
    fn handle_dub_review(&mut self) -> Result<(), Box<dyn Error>> {
        // Mix together `recorded_audio` and `recorded_dub` and play it back
        let mixer = AudioMixer::new();
        let mixed = mixer.mix_buffers(&self.recorded_audio, &self.recorded_dub);
        self.play_audio(&mixed)?;
        Ok(())
    }

    fn handle_dub_accept(&mut self) -> Result<(), RecorderError> {
        Ok(())
    }

    /// Save the audio from the `recorded_audio` to a FLAC file
    pub fn handle_save(&mut self) -> Result<(), RecorderError> {
        eprintln!(
            "DBG recorder: Save to file name {}: {} samples.  AudioFormat: {:?}",
            self.file_name,
            self.recorded_audio.len(),
            self.format,
        );
        let data = match self.format {
            AudioFormat::Flac => audio_to_flac(&self.recorded_audio)?,
            AudioFormat::Raw => {
                // The data as received from Jack.
                unsafe {
                    std::slice::from_raw_parts(
                        self.recorded_audio.as_ptr() as *const u8,
                        self.recorded_audio.len() * std::mem::size_of::<f32>(),
                    )
                }
                .to_vec()
            }
        };
        fs::write(self.file_name.as_str(), &data)
            .map_err(|err| RecorderError::Generic(err.to_string()))?;
        Ok(())
    }

    /// When a command is passed into the programme by `-k`
    /// TODO: Move this into `App`
    pub fn handle_kommand(&mut self, k: Command) -> Result<(), Box<dyn Error>> {
        match k {
            Command::Record => {
                println!("Recording.  C-c to stop");
                self.handle_recording()?;
                if let Some(h) = self.audio_handle.take() {
                    match h.join() {
                        Ok(Ok(data)) => {
                            self.recorded_audio = data;
                            self.handle_save()?;
                            Ok(())
                        }
                        Ok(Err(recorder_error)) => Err(recorder_error.into()),
                        Err(err) => Err(format!(
                            "Error qzn3t/recorder: Failed getting data: {err:?}"
                        )
                        .into()),
                    }
                } else {
                    Err("Error qzn3t/recorder: Failed to take record_handle".into())
                }
            }
            _ => panic!("Error recorder: -k {k:?} is not handled"),
        }
    }

    /// Send `recorded_audio` to the backend to play.
    fn play_audio(&self, data: &[f32]) -> Result<(), Box<dyn Error>> {
        let tx = self.audio_tx.clone();

        // Flag to shut down playback from the UI
        let audio_run = self.recorder_run.clone();

        // Must copy the data so the playback is independant of the
        // original buffer remaining
        let data = data.to_vec();
        thread::spawn(move || {
            let sample_rate = get_sample_rate();

            // Send a block of data every 100ms
            let blk_sz = sample_rate / 10;
            let mut k = 0;

            // Record how much data sent
            let mut sent = 0_usize;
            for i in data.iter() {
                if !audio_run.load(Ordering::Relaxed) {
                    break;
                }

                k += 1;
                if k == blk_sz {
                    thread::sleep(std::time::Duration::from_millis(100));
                    k = 0;
                }

                if let Err(e) = tx.send(*i) {
                    eprintln!("Error recorder: Playing audio: {e}");
                    break;
                }
                sent += 1;
            }
            eprintln!("DBG recorder: Sent {sent}/{} samples", data.len());
        });
        Ok(())
    }
}
