// Copyright (c) 2025 Worik Turei Stanton
// License: GPL-3.0

//! The main container for the programme
use crate::errors::RecorderError;
use crate::io::{AudioBuffers, Inputs};
use crate::structs::Command;
use crate::utils::get_sample_rate;
use jack_rec;
use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::path::PathBuf;
use std::sync::Once;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::Duration;

static SET_HANDLER: Once = Once::new();

/// Hold the code that runs the programme
pub struct App;
impl App {
    /// Set up the environment to run in
    pub fn initialise(
        &mut self,
        audio_tx: mpsc::Sender<f32>,
        command_rx: mpsc::Receiver<Command>,
        inputs: Inputs,
        directory: Option<PathBuf>,
    ) -> Result<AppData, Box<dyn Error>> {
        let recorder_run = Arc::new(AtomicBool::new(true));
        let ui_run = Arc::new(AtomicBool::new(true));
        let r = recorder_run.clone();

        // This has to be done so this function can be tested in parallel
        SET_HANDLER.call_once(|| {
            if let Err(err) = ctrlc::set_handler(move || {
                eprintln!("DBG qzn3t/recorder: Received Ctrl-C, shutting down gracefully...");
                r.store(false, Ordering::Relaxed);
            }) {
                panic!("Error qzn3t/recorder: setting Ctrl-C handler: {err}");
            }
        });

        Ok(AppData {
            recorded_audio: AudioBuffers::new(),
            audio_handle: None,
            recorded_dub: Vec::new(),
            audio_tx: vec![audio_tx],
            command_rx,
            recorder_run,
            ui_run,
            inputs,
            save_dir: directory,
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
                    Command::Record => config_app.handle_record()?,
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
#[allow(dead_code)]
pub struct AppData {
    pub recorded_audio: AudioBuffers,
    recorded_dub: Vec<f32>,
    pub audio_handle: Option<thread::JoinHandle<Result<AudioBuffers, RecorderError>>>,
    // TODO: Multi channel.  Need a collection of channels.
    audio_tx: Vec<mpsc::Sender<f32>>,
    command_rx: mpsc::Receiver<Command>,
    pub recorder_run: Arc<AtomicBool>,
    pub ui_run: Arc<AtomicBool>,
    save_dir: Option<PathBuf>,
    inputs: Inputs,
}
impl AppData {
    /// Stop all the processes
    fn quit(&mut self) {
        _ = self.handle_audio_stop();
        self.ui_run.store(false, Ordering::SeqCst);
    }

    /// Spawn a thread to get audio data from a Jack port
    fn get_audio_from_jack(
        &mut self,
    ) -> Result<thread::JoinHandle<Result<AudioBuffers, RecorderError>>, RecorderError> {
        // Copy of the switch to turn the recorder off
        let recorder_run = self.recorder_run.clone();

        // Local switch that is set when recorder is ready and recording
        let active_1 = Arc::new(AtomicBool::new(false));
        let active_2 = active_1.clone();

        let jack_ports = self.inputs.named_ports();
        let result = Ok(thread::spawn(
            move || -> Result<AudioBuffers, RecorderError> {
                // Buffer and channel to get data on
                let mut audio_data = AudioBuffers::new();
                let mut audio_input_pipe = HashMap::new();
                let mut senders = Vec::new();
                let mut inputs = Vec::new();
                for np in jack_ports.iter() {
                    let name = np.0.as_str();
                    let input = np.1.clone();
                    audio_data.add_buffer(name, Vec::new())?;

                    let (sender, receiver) = mpsc::channel::<f32>();
                    audio_input_pipe.insert(name.to_string(), receiver);
                    senders.push(sender);
                    inputs.push(input);
                }

                let client = "qzn3t/recorder".to_string();
                let ac = match jack_rec::run_port(client, inputs, senders, recorder_run.clone()) {
                    Ok(p) => p,
                    Err(err) => {
                        eprintln!("Error recorder: {err}: get audio");
                        return Err(err.into());
                    }
                };

                // Signal that this is running to caller (parent)
                active_2.store(true, Ordering::SeqCst);

                loop {
                    for (name, receiver) in audio_input_pipe.iter() {
                        // let receiver = pk.1;
                        let buffer: &mut Vec<f32> = audio_data.get_mut(name).unwrap();
                        let itr = receiver.try_iter();
                        for b in itr {
                            buffer.push(b);
                        }
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
            },
        ));
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

    pub fn handle_record(&mut self) -> Result<(), Box<dyn Error>> {
        self.recorded_audio.reset();

        self.recorder_run.store(true, Ordering::SeqCst);

        match self.get_audio_from_jack() {
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
        // JoinHandle holds a collection of buffers
        if let Some(handle) = self.audio_handle.take() {
            let j = handle.join();
            match j {
                Ok(Ok(mut audio_data)) => {
                    for (n, b) in audio_data.iter_mut() {
                        self.recorded_audio.add_buffer(n, b.to_vec())?;
                    }
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
        // This is possibly quite a big copy.  FIXME: There must be a
        // better way
        self.play_audio(self.recorded_audio.clone())
    }

    /// Play the contents of `recorded_audio` while recording separately
    fn handle_dubing(&mut self) -> Result<(), Box<dyn Error>> {
        unimplemented!();
    }

    /// Mix `recorded_audio` and `recorded_dub` and play it back
    fn handle_dub_review(&mut self) -> Result<(), Box<dyn Error>> {
        // Mix together `recorded_audio` and `recorded_dub` and play it back
        unimplemented!();
    }

    fn handle_dub_accept(&mut self) -> Result<(), RecorderError> {
        unimplemented!();
    }

    /// Save the audio from the `recorded_audio`
    pub fn handle_save(&mut self) -> Result<(), RecorderError> {
        assert!(self.save_dir.is_some());
        for (name, audio_data) in self.recorded_audio.iter() {
            eprintln!(
                "DBG recorder: Save to file name {:?}/{}: {} samples",
                self.save_dir,
                name,
                audio_data.len(),
            );
            let data = unsafe {
                std::slice::from_raw_parts(
                    audio_data.as_ptr() as *const u8,
                    audio_data.len() * std::mem::size_of::<f32>(),
                )
            }
            .to_vec();

            // `self.save_dir` is not `None`
            let dest: PathBuf = self.save_dir.as_ref().unwrap().join(name);
            fs::write(dest, &data).map_err(|err| RecorderError::Generic(err.to_string()))?;
        }
        Ok(())
    }

    /// When a command is passed into the programme by `-k`
    /// TODO: Move this into `App`
    pub fn handle_kommand(&mut self, k: Command) -> Result<(), Box<dyn Error>> {
        match k {
            Command::Record => {
                println!("Recording.  C-c to stop");
                self.handle_record()?;
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
    fn play_audio(&self, audio: AudioBuffers) -> Result<(), Box<dyn Error>> {
        // FIXME: Multi-channel.  A sender for each channel?
        let tx = self.audio_tx.clone();

        // Flag to shut down playback from the UI
        let audio_run = self.recorder_run.clone();

        // Must copy the data so the playback is independant of the
        // original buffer remaining
        // let data = data.to_vec();
        thread::spawn(move || {
            let sample_rate = get_sample_rate();

            // Send a block of data every 100ms
            let blk_sz = sample_rate / 10;
            let mut k = 0;

            // Record how much data sent
            let mut sent = 0_usize;

            for i in audio.iter() {
                if !audio_run.load(Ordering::Relaxed) {
                    break;
                }
                for (j, d) in i.1.iter().enumerate() {
                    if let Err(e) = tx[j].send(*d) {
                        eprintln!("Error recorder: Playing audio: {e}");
                        break;
                    }
                    sent += 1;
                }

                k += 1;
                if k == blk_sz {
                    thread::sleep(std::time::Duration::from_millis(100));
                    k = 0;
                }
                eprintln!("DBG recorder: Sent {sent}/{} samples", i.1.len());
            }
        });
        Ok(())
    }
}
