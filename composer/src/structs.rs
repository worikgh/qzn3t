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

#[derive(Debug)]
pub struct ThisError;
impl Error for ThisError {}
impl fmt::Display for ThisError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ThisError")
    }
}
impl From<anyhow::Error> for ThisError {
    fn from(err: anyhow::Error) -> Self {
        eprintln!("Error composer: `From<anyhow::Error> for ThisError` err: {err}");
        ThisError
    }
}
impl From<Box<dyn Error>> for ThisError {
    fn from(err: Box<dyn Error>) -> Self {
        eprintln!("Error composer: `From<Box<dyn Error>> for ThisError` err: {err}");
        ThisError
    }
}
