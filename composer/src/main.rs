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
use std::sync::mpsc::Sender;
use std::sync::mpsc::TryRecvError;
use std::thread;
use std::thread::JoinHandle;
use std::thread::spawn;
use structs::Args;
use structs::AudioFormat;
use structs::Command;
use structs::ThisError;
use ui::{UI, UIError};
mod mixer;
mod send_audio_to_jack;
mod structs;
mod ui;

#[allow(dead_code)]
#[derive(Clone)]
struct ComopositionApp;
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
    format: AudioFormat,
}

impl ConfigApp {
    fn quit(&mut self) {
        self.handle_stop().unwrap();
    }
    fn get_audio_from_jack(
        &mut self,
        port: String,
    ) -> Result<thread::JoinHandle<Vec<f32>>, ThisError> {
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
            audio_data
        }))
    }
    fn handle_recording(&mut self) -> Result<(), Box<dyn Error>> {
        self.recorded_audio.truncate(0);
        let port = self.port_name.clone();
        match self.get_audio_from_jack(port) {
            Ok(handle) => self.record_handle = Some(handle),
            Err(err) => {
                eprintln!("Error composer: Error from get_audio_from_jack");
                return Err(err.into());
            }
        };
        Ok(())
    }

    /// The command: stop
    fn handle_stop(&mut self) -> Result<(), Box<dyn Error>> {
        // This ends the main loop
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
            "DBG composer: Save to file name {}: {} samples.  AudioFormat: {:?}",
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
        fs::write(self.file_name.as_str(), &data)?;
        Ok(())
    }

    /// When a command is passed into the programme by `-k`
    fn handle_kommand(&mut self, k: Command) -> Result<(), Box<dyn Error>> {
        match k {
            Command::Record => {
                println!("Recording.  C-c to stop");
                self.handle_recording()?;
                if let Some(h) = self.record_handle.take() {
                    match h.join() {
                        Ok(data) => {
                            self.recorded_audio = data;
                            self.handle_save()?;
                            Ok(())
                        }
                        Err(err) => Err(format!(
                            "Error qzn3t/composer: Failed getting data: {err:?}"
                        )
                        .into()),
                    }
                } else {
                    Err("Error qzn3t/composer: Failed to take record_handle".into())
                }
            }
            _ => panic!("Error composer: -k {k:?} is not handled"),
        }
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
        Ok(Self)
    }

    /// Set up the environment to run in
    fn initialise(
        &mut self,
        audio_tx: mpsc::Sender<f32>,
        command_rx: mpsc::Receiver<Command>,
        port: String,
        file_name: String,
        use_raw: bool,
    ) -> Result<ConfigApp, Box<dyn Error>> {
        let ok_to_run = Arc::new(AtomicBool::new(true));
        let r = ok_to_run.clone();
        ctrlc::set_handler(move || {
            eprintln!("DBG qzn3t/composer: Received Ctrl-C, shutting down gracefully...");
            r.store(false, Ordering::Relaxed);
        })
        .expect("Error qzn3t/composer: setting Ctrl-C handler");
        Ok(ConfigApp {
            recorded_audio: Vec::new(),
            record_handle: None,
            recorded_dub: Vec::new(),
            audio_tx,
            command_rx,
            ok_to_run,
            port_name: port,
            file_name,
            format: if use_raw {
                AudioFormat::Raw
            } else {
                AudioFormat::Flac
            },
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

/// The UI loop
fn ui_loop(
    mut app_handle: Option<JoinHandle<Result<(), ThisError>>>,
    command_tx: &Sender<Command>,
) -> Result<(), Box<dyn Error>> {
    // The user interface...
    let mut ui = UI::new();
    let _ = UI::set_up_screen();
    loop {
        ui.display(None);
        if !test_main_loop(&mut app_handle) {
            break;
        }

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
    Ok(())
}

/// Check if the main loop is over
fn test_main_loop(app_handle: &mut Option<JoinHandle<Result<(), ThisError>>>) -> bool {
    // Test `is_finished`.  Set --> main loop finished
    if app_handle.as_ref().unwrap().is_finished()
	// Get the handle of the thread
	&& let Some(t) = app_handle.take()
    {
        match t.join() {
            Ok(result) => match result {
                Ok(()) => {
                    eprintln!("Thread finished successfully");
                    false
                }
                Err(err) => {
                    eprintln!("Thread finished with error: {err}",);
                    false
                }
            },
            Err(_) => {
                // Thread has finished before this call
                unreachable!()
            }
        }
    } else {
        true
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

    // Start the application.  Runs in its own thread, the handle is in `app_handle`
    match args.kommand {
        None => {
            let config: ConfigApp =
                app.initialise(audio_tx, command_rx, args.input, file_name, args.raw)?;
            let _out_port = create_out_port("output", audio_rx, config.ok_to_run.clone())?;
            let t = app.run(config)?;
            let app_handle = Some(t);
            // The audio output.  Stays valid so long as `_out_port` exists.
            ui_loop(app_handle, &command_tx)
        }
        Some(k) => {
            let mut cfg: ConfigApp =
                app.initialise(audio_tx, command_rx, args.input, file_name, args.raw)?;
            cfg.handle_kommand(k)?;
            Ok(())
        }
    }
}
