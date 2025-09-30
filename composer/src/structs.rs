// Copyright (c) 2025 Worik Turei Stanton
// License: GPL-3.0

use clap::Parser;
use std::{path::PathBuf, sync::mpsc};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
#[allow(dead_code)]
pub struct Args {
    /// Input Jack pipe to record (e.g., -i yoshimi:left).  Only one
    /// at a tme (for now)
    #[arg(short = 'i', long)]
    pub input: String,

    /// Prefix for track file names (defaults to YYYYMMDDhhmmss)
    #[arg(short = 'p', long)]
    pub prefix: Option<String>,

    /// Backing track for immediate overdubbing
    #[arg(short = 'b', long)]
    pub backing_track: Option<PathBuf>,

    /// Directory to write files to (used in audio/)
    #[arg(short = 'd', long)]
    pub directory: Option<String>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Config {
    pub qzn3t_root: PathBuf,
    pub data_dir: PathBuf,
    pub audio_dir: PathBuf,
    pub sox_path: PathBuf,
    pub play_path: PathBuf,
    pub amplitude_path: PathBuf,
    pub jack_rec_path: PathBuf,
    pub file_prefix: String,
    pub directory: String,
    pub backing_track: Option<PathBuf>,
    pub input: String,
    pub audio_out: mpsc::Sender<f32>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum State {
    Recording,
    Dubing,
    RecordingReview,
    DubReview,
    DubAccept,
}

impl State {
    pub fn message(&self) -> &'static str {
        match self {
            State::Recording => "Press \n<enter> to start recording",
            State::Dubing => "Press \n<enter> to start overdubbing",
            State::RecordingReview => {
                "Press \n<enter> to review recording \nr <enter> to record again \nd <enter> to overdub"
            }
            State::DubReview => {
                "Press \n<enter> to review dub \nd <enter> to dub again \nr <enter> to record again"
            }
            State::DubAccept => {
                "Press \nd <enter> to dub again \nr <enter> to record again\ng <enter> Review again"
            }
        }
    }
}
