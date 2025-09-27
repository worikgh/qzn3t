// Copyright (c) 2025 Worik Turei Stanton
// License: GPL-3.0

use crate::send_audio_to_jack::create_out_port;
use anyhow::{Context, Result, anyhow};
use clap::Parser;
use jack_rec::run_port;
use mixer::AudioMixer;
use std::error::Error;
use std::io::{self, Write};
use std::process::Command as ProcessCommand;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::sync::mpsc::TryRecvError;
use std::thread;
use std::thread::JoinHandle;
use std::thread::spawn;
use std::time::Duration;
use structs::Args;
use structs::Command;
use structs::ThisError;
mod mixer;
mod send_audio_to_jack;
mod structs;

#[allow(dead_code)]
#[derive(Clone)]
struct CompositionApp {}

struct ConfigApp {
    recorded_audio: Vec<f32>,
    recorded_dub: Vec<f32>,
    record_handle: Option<JoinHandle<Vec<f32>>>,
    audio_out: mpsc::Sender<f32>,
    cmd_rx: mpsc::Receiver<Command>,
    ok_to_run: Arc<AtomicBool>,
    port_name: String,
}

impl ConfigApp {
    fn quit(&mut self) {
        self.stop_recording().unwrap();
    }
    fn get_audio_from_jack(&mut self) -> Result<thread::JoinHandle<Vec<f32>>, ThisError> {
        let port = self.port_name.clone();
        let run_flag = self.ok_to_run.clone();
        Ok(thread::spawn(move || -> Vec<f32> {
            eprintln!("DBG composer: get_audio_from_jack 1");
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
            eprintln!("DBG composer: get_audio_from_jack 1.5");
            let mut k = 0;
            loop {
                if !run_flag.load(Ordering::Relaxed) {
                    break;
                }
                k += 1;
                if k % 10 == 0 {
                    eprintln!(
                        "DBG composer: Record {k} Audio data: {} bytes",
                        audio_data.len()
                    );
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
                thread::sleep(Duration::from_millis(100));
            }
            eprintln!("DBG composer: get_audio_from_jack 1.9");
            async_jack_client.deactivate().unwrap();
            eprintln!(
                "DBG composer: get_audio_from_jack 2  Got {} bytes, {} non-zero",
                audio_data.len(),
                audio_data
                    .iter()
                    .filter(|a| a.abs() > 0.0001)
                    .collect::<Vec<_>>()
                    .len(),
            );
            audio_data
        }))
    }
    fn handle_recording(&mut self) -> Result<(), Box<dyn Error>> {
        match self.get_audio_from_jack() {
            Ok(handle) => self.record_handle = Some(handle),
            Err(err) => {
                eprintln!("Error composer: Error from get_audio_from_jack");
                return Err(err.into());
            }
        };
        Ok(())
    }

    fn stop_recording(&mut self) -> Result<(), Box<dyn Error>> {
        if let Some(handle) = self.record_handle.take() {
            self.ok_to_run.store(false, Ordering::Relaxed);
            if let Ok(audio_data) = handle.join() {
                eprintln!(
                    "DBG composer: Stop recording  Got {} bytes, {} non-zero",
                    audio_data.len(),
                    audio_data
                        .iter()
                        .filter(|a| a.abs() > 0.0001)
                        .collect::<Vec<_>>()
                        .len(),
                );
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

    /// Send `recorded_audio` to the backend to play.
    fn play_audio(&self, data: &[f32]) -> Result<(), Box<dyn Error>> {
        eprintln!(
            "DBG composer: play_audio  Got {} bytes, {} non-zero",
            data.len(),
            data.iter()
                .filter(|a| a.abs() > 0.0001)
                .collect::<Vec<_>>()
                .len(),
        );
        let mut k = 0;
        for i in data.iter() {
            // let _ = data.iter().map(|&b| {
            k += 1;
            if let Err(e) = self.audio_out.send(*i) {
                eprintln!("Error composer: Playing audio: {e}");
            }
        }
        eprintln!("DBG composer: Sent {k} bytes");
        Ok(())
    }
}

impl CompositionApp {
    fn new() -> Result<Self> {
        Ok(Self {})
    }

    /// Set up the environment to run in
    fn initialise(
        &mut self,
        sender: mpsc::Sender<f32>,
        commands: mpsc::Receiver<Command>,
        port: String,
    ) -> Result<ConfigApp, Box<dyn Error>> {
        Ok(ConfigApp {
            recorded_audio: Vec::new(),
            record_handle: None,
            recorded_dub: Vec::new(),
            audio_out: sender,
            cmd_rx: commands,
            ok_to_run: Arc::new(AtomicBool::new(true)),
            port_name: port,
        })
    }

    fn run(
        &mut self,
        mut config_app: ConfigApp,
    ) -> Result<JoinHandle<Result<(), ThisError>>, Box<dyn Error>> {
        // The version of `self` used inside the loop

        let t = spawn(move || -> Result<(), ThisError> {
            loop {
                let command = match config_app.cmd_rx.recv() {
                    Ok(s) => s,
                    Err(err) => {
                        eprintln!("Error composer: Getting state: {err}");
                        break;
                    }
                };
                eprintln!("DBG compose: App::run command: {command:?}");
                match command {
                    Command::Record => config_app.handle_recording()?,
                    Command::Stop => config_app.stop_recording()?,
                    Command::ReviewRecord => config_app.handle_review_record()?,
                    Command::Dubing => config_app.handle_dubing()?,
                    Command::DubReview => config_app.handle_dub_review()?,
                    Command::DubAccept => config_app.handle_dub_accept()?,
                    Command::Quit => {
                        config_app.quit();
                        break;
                    }
                }
            }
            eprintln!("DBG composer: Broken from main loop");
            Ok(())
        }); // Closure
        Ok(t)
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
    let (audio_out_send, audio_out_receive) = mpsc::channel::<f32>();
    let args = Args::parse();

    // Validate Jack pipe input
    validate_jack_pipe(&args.input)?;

    // The app is controlled through a channel with the front end UI
    let (app_send, app_rec) = mpsc::channel::<Command>();

    let mut app = CompositionApp::new()?;
    let config: ConfigApp = app.initialise(audio_out_send, app_rec, args.input)?;
    let _out_port = create_out_port("output", audio_out_receive, config.ok_to_run.clone())?;
    let t = app.run(config)?;
    let mut t = Some(t);

    loop {
        if t.as_ref().unwrap().is_finished()
            && let Some(t) = t.take()
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

        println!(" Qzn3t Composer\n");
        io::stdout().lock().write_all("input > ".as_bytes())?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();

        let command: Command = match input {
            "r" => Command::Record,
            "s" => Command::Stop,
            "q" => Command::Quit,
            "v" => Command::ReviewRecord,
            _ => continue,
        };
        match command {
            Command::Record =>
            // Recording audio
            {
                io::stdout()
                    .lock()
                    .write_all("Press s <ENTER> to stop".as_bytes())
                    .unwrap();
            }
            _ => {
                let msg = format!("No prompt for {command:?}");
                io::stdout().lock().write_all(msg.as_bytes()).unwrap();
            }
        };
        app_send.send(command.clone())?;
        if command == Command::Quit {
            break;
        }
        eprintln!("DBG compose: After send command: {command:?}");
    }
    eprintln!("DBG composer: Leaving main");
    Ok(())
}
