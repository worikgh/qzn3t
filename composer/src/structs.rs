// Copyright (c) 2025 Worik Turei Stanton
// License: GPL-3.0

use clap::Parser;
use std::error::Error;
use std::fmt;
use std::path::PathBuf;
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
#[allow(dead_code)]
pub struct Args {
    /// Input Jack pipe to record (e.g., -i yoshimi:left).  Only one
    /// at a tme (for now)
    #[arg(short = 'i', long)]
    pub input: String,

    /// Backing track for immediate overdubbing
    #[arg(short = 'b', long)]
    pub backing_track: Option<PathBuf>,

    /// Directory to write files to
    #[arg(short = 'd', long, default_value = ".")]
    pub directory: String,

    /// Output file name
    #[arg(short = 'f', long, default_value = "test.flac")]
    pub file_name: String,
}

#[derive(Debug, Clone, PartialEq, Hash, Eq)]
#[allow(dead_code)]
pub enum Command {
    Stop,
    Record,
    ReviewRecord,
    Dubing,
    DubReview,
    DubAccept,
    Save,
    Continue, // Used if no menu item selected
    Quit,
}

impl Command {}
impl fmt::Display for Command {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Command::Record => "Record",
                Command::Stop => "Stop",
                Command::ReviewRecord => "Review recording",
                Command::Dubing => "Dub",
                Command::Save => "Save",
                Command::Quit => "Quit",
                _ => "Unknown command {self:?}",
            }
        )
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum ThisError {
    BadCommand(Command),
    Generic,
}
impl Error for ThisError {}
impl fmt::Display for ThisError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl From<anyhow::Error> for ThisError {
    fn from(err: anyhow::Error) -> Self {
        eprintln!("Error composer: `From<anyhow::Error> for ThisError` err: {err}");
        ThisError::Generic
    }
}
impl From<Box<dyn Error>> for ThisError {
    fn from(err: Box<dyn Error>) -> Self {
        eprintln!("Error composer: `From<Box<dyn Error>> for ThisError` err: {err}");
        ThisError::Generic
    }
}
