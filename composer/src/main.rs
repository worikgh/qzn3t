// Copyright (c) 2025 Worik Turei Stanton
// License: GPL-3.0

use anyhow::{anyhow, Context, Result};
use chrono::prelude::*;
use clap::Parser;
use nix::sys::signal::Signal;
use nix::sys::signal::{self};
use nix::unistd::Pid;
use serde_json::Value;
use std::fs::{self, create_dir_all};
use std::io::{self};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
mod structs;

use structs::Args;
use structs::Config;
use structs::State;

static RUNNING: AtomicBool = AtomicBool::new(true);

#[allow(dead_code)]
struct CompositionApp {
    config: Config,
    state: State,
    pids: Vec<u32>,
    dub_counter: u32,
    fn_counter: u32,
    fn_rec: Option<String>,
    fn_dub: Option<String>,
    fn_dir: Option<PathBuf>,
    backing_track: Option<PathBuf>,
    dub_track: Option<PathBuf>,
    pfx: String,
}

impl CompositionApp {
    fn new(config: Config) -> Result<Self> {
        let state = if config.backing_track.is_some() {
            State::Dubing
        } else {
            State::Recording
        };

        let pfx = if let Some(backing_track) = config.backing_track.clone() {
            if let Some(stem) = backing_track.file_stem().and_then(|s| s.to_str()) {
                let id = if let Some(caps) = regex::Regex::new(r"(\d{14})(\S+)\.wav$")
                    .ok()
                    .and_then(|re| re.captures(stem))
                {
                    format!("_{}_", &caps[2])
                } else if let Some(caps) = regex::Regex::new(r"([a-zA-Z_\\-\\d:\\.]+)\.wav$")
                    .ok()
                    .and_then(|re| re.captures(stem))
                {
                    format!("_{}_", &caps[1])
                } else {
                    "".to_string()
                };
                config.file_prefix.clone() + &id
            } else {
                config.file_prefix.clone()
            }
        } else {
            config.file_prefix.clone()
        };

        Ok(Self {
            config,
            state,
            pids: Vec::new(),
            dub_counter: 1,
            fn_counter: 1,
            fn_rec: None,
            fn_dub: None,
            fn_dir: None,
            backing_track: None,
            dub_track: None,
            pfx,
        })
    }

    fn run(&mut self) -> Result<()> {
        let running = Arc::new(AtomicBool::new(true));
        let r = running.clone();
        eprintln!("DBG compose: CompositionApp::run");

        if let Some(ref backing_track) = self.config.backing_track {
            eprintln!("DBG compose: CompositionApp::run  Have backing track: {backing_track:?}");
            if !backing_track.exists() {
                return Err(anyhow!("Unreadable backing track: {:?}", backing_track));
            }
            self.backing_track = Some(backing_track.clone());

            if let Some(file_name) = backing_track.file_name().and_then(|s| s.to_str()) {
                self.fn_rec = Some(file_name.to_string());
            }
        }

        create_dir_all(&self.config.audio_dir)?;

        while running.load(Ordering::SeqCst) {
            eprintln!(
                "DBG compose: State: {:?}\n{}",
                self.state,
                self.state.message()
            );

            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            let input = input.trim();

            if input == "q" {
                break;
            }

            match self.state {
                State::Recording => self.handle_recording()?,
                State::RecordingReview => self.handle_recording_review(input)?,
                State::Dubing => self.handle_dubing()?,
                State::DubReview => self.handle_dub_review(input)?,
                State::DubAccept => self.handle_dub_accept(input)?,
            }
        }

        self.cleanup()?;
        Ok(())
    }

    fn handle_recording(&mut self) -> Result<()> {
        self.backing_track = None;
        self.fn_dir = Some(self.config.audio_dir.join(self.fn_counter.to_string()));
        create_dir_all(self.fn_dir.as_ref().unwrap())?;

        // File name to record to
        self.fn_rec = Some(
            self.fn_dir
                .as_ref()
                .unwrap()
                .join(&self.config.file_prefix)
                .to_string_lossy()
                .to_string(),
        );

        println!("Press <enter> to stop recording");

        eprintln!("DBG compose: CompositionApp::handle_recording 1 {cmd}");
        let result = self.jack_rec_cmd(self.fn_rec.as_ref().unwrap())?;
        eprintln!("DBG compose: CompositionApp::handle_recording 2 {result}");

        println!("Processing...");
        let out_file_stats = self.process_jackrec(&result)?;

        if out_file_stats.is_empty() {
            println!("No matching file has audio in it. Cannot make a backing track for dubbing");
            self.state = State::Recording;
        } else {
            self.backing_track = Some(self.get_peakiest_file(&out_file_stats)?);
            self.state = State::RecordingReview;

            if let Some(ref bt) = self.backing_track
                && let Ok(relative_path) = bt.strip_prefix(self.config.data_dir.join("audio"))
            {
                println!("{}", relative_path.display());
            }
        }

        Ok(())
    }

    fn handle_recording_review(&mut self, input: &str) -> Result<()> {
        match input.to_lowercase().as_str() {
            "r" => self.state = State::Recording,
            "d" => self.state = State::Dubing,
            _ => {
                if let Some(ref backing_track) = self.backing_track {
                    self.play_audio(backing_track)?;
                }
            }
        }
        Ok(())
    }

    fn handle_dubing(&mut self) -> Result<()> {
        self.fn_dir = Some(self.config.audio_dir.join(self.fn_counter.to_string()));
        create_dir_all(self.fn_dir.as_ref().unwrap())?;

        self.fn_dub = Some(format!(
            "{}-{}",
            self.fn_rec.as_ref().unwrap(),
            self.dub_counter
        ));
        self.dub_counter += 1;

        println!("Press <enter> to stop overdubbing");

        if let Some(ref backing_track) = self.backing_track {
            let pid = self.run_daemon(
                &format!(
                    "{} -ao jack {:?}",
                    self.config.play_path.display(),
                    backing_track
                ),
                false,
            )?;
            self.pids.push(pid);
        }

        let cmd = self.jack_rec_cmd(self.fn_dub.as_ref().unwrap())?;
        let result = Self::run_command(&cmd)?;

        self.kill_play_processes()?;

        println!("Processing\nResult: {}", result);
        let out_file_stats = self.process_jackrec(&result)?;
        self.dub_track = Some(self.get_peakiest_file(&out_file_stats)?);

        self.state = State::DubReview;
        Ok(())
    }

    fn handle_dub_review(&mut self, input: &str) -> Result<()> {
        match input.to_lowercase().as_str() {
            "d" => self.state = State::Dubing,
            "r" => self.state = State::Recording,
            _ => {
                if let (Some(backing_track), Some(dub_track)) =
                    (&self.backing_track, &self.dub_track)
                {
                    let p1 = self.run_daemon(
                        &format!(
                            "{} -ao jack {:?}",
                            self.config.play_path.display(),
                            backing_track
                        ),
                        false,
                    )?;
                    let p2 = self.run_daemon(
                        &format!(
                            "{} -ao jack {:?}",
                            self.config.play_path.display(),
                            dub_track
                        ),
                        false,
                    )?;
                    self.pids.extend_from_slice(&[p1, p2]);
                    self.state = State::DubAccept;
                }
            }
        }
        Ok(())
    }

    fn handle_dub_accept(&mut self, input: &str) -> Result<()> {
        self.kill_play_processes()?;
        self.pids.clear();

        match input {
            "r" => self.state = State::Recording,
            "d" => self.state = State::Dubing,
            "g" => self.state = State::DubReview,
            _ => {}
        }
        Ok(())
    }

    fn jack_rec_cmd(&self, prefix: &str) -> Result<String> {
        let inputs: Vec<String> = self
            .config
            .inputs
            .iter()
            .map(|input| format!("-i \"{}\"", input))
            .collect();

        let cmd = format!(
            "{} -p \"{}\" {}",
            self.config.jack_rec_path.display(),
            prefix,
            inputs.join(" ")
        );
        Ok(cmd)
    }

    fn process_jackrec(&self, result: &str) -> Result<Vec<(f64, PathBuf)>> {
        eprintln!("DBG compose: CompositionApp::process_jackrec {result}");
        let start = std::time::Instant::now();
        let mut file_stats = Vec::new();

        // Parse JSON output from jack_rec
        let json: Value = serde_json::from_str(result)
            .map_err(|e| anyhow!("Failed to parse jack_rec output: {}", e))?;

        let output_files = json["output_files"]
            .as_array()
            .ok_or_else(|| anyhow!("Invalid jack_rec output format"))?;

        for file_value in output_files {
            if let Some(raw_file) = file_value.as_str() {
                let raw_path = PathBuf::from(raw_file);

                // Check amplitude
                let amplitude_output = Self::run_command(&format!(
                    "{} {:?}",
                    self.config.amplitude_path.display(),
                    raw_path
                ))?;
                let amplitude: f64 = amplitude_output.trim().parse().unwrap_or(0.0);

                if amplitude > 0.01 {
                    let wav_file = raw_path.with_extension("wav");

                    // Convert raw to wav using sox
                    let sox_cmd = format!(
                        "{} -q -t raw -b 32 -e float -c 1 -r 48k {:?} -e signed-integer -b 16 {:?}",
                        self.config.sox_path.display(),
                        raw_path,
                        wav_file
                    );

                    Self::run_command(&sox_cmd)?;

                    let peaks = self.peaks(&wav_file)?;
                    file_stats.push((peaks, wav_file.clone()));

                    if let Some(file_name) = wav_file.file_name().and_then(|s| s.to_str()) {
                        println!("Peaks {:.4} Output {}", peaks, file_name);
                    }
                }

                // Clean up raw file
                let _ = fs::remove_file(&raw_path);
            }
        }

        println!(
            "Processing files took: {} seconds",
            start.elapsed().as_secs()
        );
        Ok(file_stats)
    }

    fn peaks(&self, filename: &Path) -> Result<f64> {
        let cmd = format!(
	    "ffmpeg -loglevel quiet -i {:?} -af astats=metadata=1:reset=1,ametadata=print:key=lavfi.astats.Overall.RMS_level:file=- -f null -",
	    filename
	);

        let output = Self::run_command(&cmd)?;
        let lines: Vec<&str> = output.lines().collect();

        let data: Vec<f64> = lines
            .iter()
            .filter(|line| line.contains("lavfi.astats.Overall.RMS_level="))
            .filter_map(|line| {
                line.strip_prefix("lavfi.astats.Overall.RMS_level=")
                    .and_then(|s| s.parse().ok())
            })
            .filter(|&x| x > f64::NEG_INFINITY)
            .collect();

        if data.is_empty() {
            Ok(0.0)
        } else {
            let sum: f64 = data.iter().sum();
            Ok(sum / data.len() as f64)
        }
    }

    fn get_peakiest_file(&self, file_stats: &[(f64, PathBuf)]) -> Result<PathBuf> {
        file_stats
            .iter()
            .max_by(|a, b| a.0.partial_cmp(&b.0).unwrap())
            .map(|(_, path)| path.clone())
            .ok_or_else(|| anyhow!("No valid audio files found"))
    }

    fn play_audio(&self, file: &Path) -> Result<()> {
        Self::run_command(&format!(
            "{} -ao jack {:?}",
            self.config.play_path.display(),
            file
        ))?;
        Ok(())
    }

    fn kill_play_processes(&self) -> Result<()> {
        let _ = Command::new("pkill")
            .arg("-f")
            .arg(self.config.play_path.to_string_lossy().as_ref())
            .output();
        Ok(())
    }

    fn run_daemon(&self, cmd: &str, wait: bool) -> Result<u32> {
        let mut parts = cmd.split_whitespace();
        let program = parts.next().ok_or_else(|| anyhow!("Empty command"))?;

        let mut command = Command::new(program);
        command.args(parts);

        if wait {
            let status = command.status()?;
            if !status.success() {
                return Err(anyhow!("Command failed: {}", cmd));
            }
            Ok(0)
        } else {
            let child = command.spawn()?;
            Ok(child.id())
        }
    }

    fn run_command(cmd: &str) -> Result<String> {
        let output = if cfg!(target_os = "windows") {
            Command::new("cmd").args(["/C", cmd]).output()?
        } else {
            eprintln!("DBG compose: CompositionApp::run_cmd 1: {cmd}");
            let res = Command::new("sh").args(["-c", cmd]).output()?;
            eprintln!("DBG compose: CompositionApp::run_cmd 2 {}", res.status);
            res
        };

        if !output.status.success() {
            return Err(anyhow!("Command failed: {}", cmd));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    fn cleanup(&mut self) -> Result<()> {
        for &pid in &self.pids {
            let _ = signal::kill(Pid::from_raw(pid as i32), Signal::SIGTERM);
        }
        self.pids.clear();
        Ok(())
    }
}

fn setup_signal_handler() -> Result<()> {
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    ctrlc::set_handler(move || {
        println!("\nShutting down gracefully...");
        r.store(false, Ordering::SeqCst);
        RUNNING.store(false, Ordering::SeqCst);
    })?;

    Ok(())
}

fn std_prefix() -> String {
    "Compose".to_string()
}

fn std_directory() -> String {
    let now = Local::now();
    let wday = match now.weekday() {
        chrono::Weekday::Sun => "Sun",
        chrono::Weekday::Mon => "Mon",
        chrono::Weekday::Tue => "Tue",
        chrono::Weekday::Wed => "Wed",
        chrono::Weekday::Thu => "Thu",
        chrono::Weekday::Fri => "Fri",
        chrono::Weekday::Sat => "Sat",
    };

    format!(
        "{}T{}_{}",
        now.format("%Y-%m-%d"),
        now.format("%H:%M:%S"),
        wday
    )
}

fn validate_jack_pipe(pipe: &str) -> Result<()> {
    let output = Command::new("jack_lsp")
        .output()
        .context("Failed to run jack_lsp")?;

    let pipes = String::from_utf8_lossy(&output.stdout);
    if !pipes.lines().any(|line| line == pipe) {
        return Err(anyhow!("Invalid Jack pipe: {}", pipe));
    }

    let type_output = Command::new("jack_lsp")
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

fn find_executable(name: &str) -> Result<PathBuf> {
    if let Ok(path) = which::which(name) {
        Ok(path)
    } else {
        Err(anyhow!("Executable not found: {}", name))
    }
}

fn main() -> Result<()> {
    setup_signal_handler()?;

    let args = Args::parse();

    // Validate Jack pipes
    for pipe in &args.inputs {
        validate_jack_pipe(pipe)?;
    }

    // Get current executable directory
    let exe_path = std::env::current_exe()?;
    let dir = exe_path
        .parent()
        .ok_or_else(|| anyhow!("Cannot get executable directory"))?;

    // FIXME: Get a better way to get this directory.  Make
    // `peak_volume` accessible as a Rust library
    let qzn3t_root = dir
        .parent()
        .ok_or_else(|| anyhow!("Cannot get QZN3T root directory"))?
        .parent()
        .ok_or_else(|| anyhow!("Cannot get QZN3T root directory"))?
        .parent()
        .ok_or_else(|| anyhow!("Cannot get QZN3T root directory"))?
        .parent()
        .ok_or_else(|| anyhow!("Cannot get QZN3T root directory"))?
        .to_path_buf();

    let data_dir = dir.join("compositions");
    if !data_dir.exists() {
        create_dir_all(&data_dir)?;
    }

    // Find required executables
    let amplitude_path = qzn3t_root.join("peak_volume/target/release/peak_volume");
    if !amplitude_path.exists() {
        return Err(anyhow!("Amplitude tool not found: {:?}", amplitude_path));
    }

    let jack_rec_path = qzn3t_root.join("jack_rec/target/release/jack_rec");
    if !jack_rec_path.exists() {
        return Err(anyhow!("jack_rec not found: {:?}", jack_rec_path));
    }

    let sox_path = find_executable("sox")?;
    let play_path = find_executable("mplayer")?;

    let config = Config {
        qzn3t_root,
        data_dir: data_dir.clone(),
        audio_dir: data_dir
            .join("audio")
            .join(args.directory.as_ref().unwrap_or(&std_directory())),
        sox_path,
        play_path,
        amplitude_path,
        jack_rec_path,
        file_prefix: args.prefix.unwrap_or_else(std_prefix),
        directory: args.directory.unwrap_or_else(std_directory),
        backing_track: args.backing_track,
        inputs: args.inputs,
    };

    create_dir_all(&config.audio_dir)?;

    let mut app = CompositionApp::new(config)?;
    app.run()?;

    Ok(())
}
