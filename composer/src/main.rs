// Copyright (c) 2025 Worik Turei Stanton
// License: GPL-3.0

use anyhow::{Context, Result, anyhow};
use clap::Parser;
use compose::audio_to_flac;
use compose::get_sample_rate;
use jack_rec::run_port;
use mixer::AudioMixer;
use send_audio_to_jack::create_out_port;
use std::error::Error;
use std::fs;
use std::process::Command as ProcessCommand;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::sync::mpsc::TryRecvError;
use std::thread;
use std::thread::JoinHandle;
use std::thread::spawn;
use structs::Args;
use structs::Command;
use structs::ThisError;
use ui::{UI, UIError};
mod mixer;
mod send_audio_to_jack;
mod structs;
mod ui;

#[allow(dead_code)]
#[derive(Clone)]
struct ComopositionApp {
    selected: Option<usize>,
}
impl ComopositionApp {}
struct ConfigApp {
    recorded_audio: Vec<f32>,
    recorded_dub: Vec<f32>,
    record_handle: Option<JoinHandle<Vec<f32>>>,
    audio_tx: mpsc::Sender<f32>,
    command_rx: mpsc::Receiver<Command>,
    ok_to_run: Arc<AtomicBool>,
    port_name: String,
    file_name: String,
}

impl ConfigApp {
    fn quit(&mut self) {
        self.handle_stop().unwrap();
    }
    fn get_audio_from_jack(&mut self) -> Result<thread::JoinHandle<Vec<f32>>, ThisError> {
        let port = self.port_name.clone();
        let run_flag = self.ok_to_run.clone();
        Ok(thread::spawn(move || -> Vec<f32> {
            // Buffer and channel to get data on
            let mut audio_data: Vec<f32> = Vec::new();
            let (sender, receiver) = mpsc::channel::<f32>();

            let async_jack_client = match run_port(port.clone(), sender) {
                Ok(p) => p,
                Err(err) => {
                    eprintln!("Error composer: {err}: get audio from {port}");
                    return vec![];
                }
            };
            loop {
                if !run_flag.load(Ordering::Relaxed) {
                    break;
                }
                loop {
                    match receiver.try_recv() {
                        Ok(b) => audio_data.push(b),
                        Err(e) => match e {
                            TryRecvError::Empty =>
                            // Got all data available
                            {
                                break;
                            }
                            TryRecvError::Disconnected =>
                            // Broken channel
                            {
                                eprintln!(
                                    "Error compose: audio data channel has become disconnected"
                                );
                                run_flag.store(true, Ordering::Relaxed)
                            }
                        },
                    };
                }
                thread::sleep(std::time::Duration::from_millis(100));
            }
            async_jack_client.deactivate().unwrap();
            eprintln!(
                "DBG composer: get_audio_from_jack 2  Got {} samples, {} non-zero",
                audio_data.len(),
                audio_data
                    .iter()
                    .filter(|a| a.abs() > 0.0001)
                    .collect::<Vec<_>>()
                    .len(),
            );
            eprintln!(
                "DBG composer: Got {} samples of audio data",
                audio_data.len()
            );
            audio_data
        }))
    }
    fn handle_recording(&mut self) -> Result<(), Box<dyn Error>> {
        self.recorded_audio.truncate(0);
        match self.get_audio_from_jack() {
            Ok(handle) => self.record_handle = Some(handle),
            Err(err) => {
                eprintln!("Error composer: Error from get_audio_from_jack");
                return Err(err.into());
            }
        };
        Ok(())
    }

    fn handle_stop(&mut self) -> Result<(), Box<dyn Error>> {
        self.ok_to_run.store(false, Ordering::Relaxed);
        if let Some(handle) = self.record_handle.take() {
            self.ok_to_run.store(false, Ordering::Relaxed);
            if let Ok(audio_data) = handle.join() {
                self.recorded_audio.extend(audio_data.iter());
            }
        }
        Ok(())
    }

    /// Play back the audio data
    fn handle_review_record(&mut self) -> Result<(), Box<dyn Error>> {
        self.ok_to_run.store(true, Ordering::Relaxed);
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

    fn handle_dub_accept(&mut self) -> Result<()> {
        Ok(())
    }

    /// Save the audio from the `recorded_audio` to a FLAC file
    fn handle_save(&mut self) -> Result<()> {
        eprintln!(
            "DBG composer: Save to file name {}: {} samples",
            self.file_name,
            self.recorded_audio.len()
        );
        let flac_data = audio_to_flac(&self.recorded_audio)?;
        fs::write(self.file_name.as_str(), &flac_data)?;
        Ok(())
    }

    /// Send `recorded_audio` to the backend to play.
    fn play_audio(&self, data: &[f32]) -> Result<(), Box<dyn Error>> {
        let tx = self.audio_tx.clone();

        // Flag to shut down playback from the UI
        let ok_to_run = self.ok_to_run.clone();

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
                if !ok_to_run.load(Ordering::Relaxed) {
                    break;
                }

                k += 1;
                if k == blk_sz {
                    thread::sleep(std::time::Duration::from_millis(100));
                    k = 0;
                }

                if let Err(e) = tx.send(*i) {
                    eprintln!("Error composer: Playing audio: {e}");
                    break;
                }
                sent += 1;
            }
            eprintln!("DBG composer: Sent {sent}/{} samples", data.len());
        });
        Ok(())
    }
}
impl ComopositionApp {
    fn new() -> Result<Self> {
        Ok(Self { selected: None })
    }

    /// Set up the environment to run in
    fn initialise(
        &mut self,
        audio_tx: mpsc::Sender<f32>,
        command_rx: mpsc::Receiver<Command>,
        port: String,
        file_name: String,
    ) -> Result<ConfigApp, Box<dyn Error>> {
        Ok(ConfigApp {
            recorded_audio: Vec::new(),
            record_handle: None,
            recorded_dub: Vec::new(),
            audio_tx,
            command_rx,
            ok_to_run: Arc::new(AtomicBool::new(true)),
            port_name: port,
            file_name,
        })
    }

    fn run(
        &mut self,
        mut config_app: ConfigApp,
    ) -> Result<JoinHandle<Result<(), ThisError>>, Box<dyn Error>> {
        // The version of `self` used inside the loop

        let app_handle = spawn(move || -> Result<(), ThisError> {
            loop {
                let command = match config_app.command_rx.recv() {
                    Ok(s) => s,
                    Err(err) => {
                        eprintln!("Error composer: Getting state: {err}");
                        break;
                    }
                };
                eprintln!("DBG compose: App::run command: {command:?}");
                match command {
                    Command::Record => config_app.handle_recording()?,
                    Command::Stop => config_app.handle_stop()?,
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
            eprintln!("DBG composer: Broken from main loop");
            Ok(())
        }); // Closure
        Ok(app_handle)
    }
}

fn validate_jack_pipe(pipe: &str) -> Result<()> {
    let output = ProcessCommand::new("jack_lsp")
        .output()
        .context("Failed to run jack_lsp")?;

    let pipes = String::from_utf8_lossy(&output.stdout);
    if !pipes.lines().any(|line| line == pipe) {
        return Err(anyhow!("Invalid Jack pipe: {}", pipe));
    }

    let type_output = ProcessCommand::new("jack_lsp")
        .arg("-t")
        .output()
        .context("Failed to run jack_lsp -t")?;

    let type_info = String::from_utf8_lossy(&type_output.stdout);
    let lines: Vec<&str> = type_info.lines().collect();

    if let Some(pos) = lines.iter().position(|&line| line == pipe) {
        if pos + 1 < lines.len() && lines[pos + 1].ends_with("audio") {
            Ok(())
        } else {
            Err(anyhow!("Pipe {} is not an audio type", pipe))
        }
    } else {
        Err(anyhow!("Pipe {} not found in type listing", pipe))
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    // Validate Jack pipe input
    validate_jack_pipe(&args.input)?;

    // Channel to send audio data to Jackd
    let (audio_tx, audio_rx) = mpsc::channel::<f32>();

    // The app is controlled through a channel with the front end UI
    let (command_tx, command_rx) = mpsc::channel::<Command>();

    // The main programme runs in `CompositionApp`
    let mut app = ComopositionApp::new()?;
    let file_name = format!("{}/{}", args.directory, args.file_name);
    let config: ConfigApp = app.initialise(audio_tx, command_rx, args.input, file_name)?;

    // The audio output.  Stays valid so long as `_out_port` exists.
    let _out_port = create_out_port("output", audio_rx, config.ok_to_run.clone())?;

    // Start the application.  Runs in its own thread, the handle is in `app_handle`
    let t = app.run(config)?;
    let mut app_handle = Some(t);

    // The user interface...
    let mut ui = UI::new();
    let _ = UI::set_up_screen();
    loop {
        ui.display(None);
        if app_handle.as_ref().unwrap().is_finished()
            && let Some(t) = app_handle.take()
        {
            match t.join() {
                Ok(result) => match result {
                    Ok(()) => {
                        eprintln!("Thread finished successfully");
                        break;
                    }
                    Err(err) => {
                        eprintln!("Thread finished with error: {err}",);
                        break;
                    }
                },
                Err(_) => {
                    // Thread has finished before this call
                    unreachable!()
                }
            };
        }

        // Simple UI
        let command = match ui.get_command() {
            Ok(c) => c,
            Err(uierr) => match uierr {
                UIError::BadChoice(_) => {
                    eprintln!("{uierr}");
                    continue;
                }
                UIError::Fatal(err) => return Err(err.into()),
            },
        };
        command_tx.send(command.clone())?;
        if command == Command::Quit {
            break;
        }
        eprintln!("DBG compose: After send command: {command:?}");
    }
    let _ = UI::cleanup_screen();
    eprintln!("DBG composer: Leaving main");
    Ok(())
}
