// Copyright (c) 2025 Worik Turei Stanton
// License: GPL-3.0

use clap::Parser;
use clap::ValueEnum;
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

    /// Write audio as raw.  Defaults to using FLAC
    #[arg(short = 'r', long, default_value_t = false)]
    pub raw: bool,

    /// If this is not None run a command directly.  Only some
    /// commands make sense
    #[arg(short = 'k', long)]
    pub kommand: Option<Command>,
}

#[derive(Debug, Clone, ValueEnum, PartialEq, Hash, Eq)]
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

/// The audio format to use
#[allow(dead_code)] // Not dead code.  Bug in linter
#[derive(Debug)]
pub enum AudioFormat {
    Flac,
    Raw,
}
