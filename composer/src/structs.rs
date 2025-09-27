// Copyright (c) 2025 Worik Turei Stanton
// License: GPL-3.0

use clap::Parser;
use std::error::Error;
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

#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub enum Command {
    Stop,
    Record,
    ReviewRecord,
    Dubing,
    DubReview,
    DubAccept,
    Quit,
}

impl Command {}

#[derive(Debug)]
pub struct ThisError;
impl Error for ThisError {}
use std::fmt;
impl fmt::Display for ThisError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "This is a custom error")
    }
}
impl From<anyhow::Error> for ThisError {
    fn from(_: anyhow::Error) -> Self {
        ThisError
    }
}
impl From<Box<dyn Error>> for ThisError {
    fn from(err: Box<dyn Error>) -> Self {
        eprintln!("Error composer: `From<Box<dyn Error>> for ThisError` err: {err}");
        ThisError
    }
}
