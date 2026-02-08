// Copyright (c) 2025 Worik Turei Stanton
// License: GPL-3.0

use clap::Parser;
use clap::ValueEnum;
use std::fmt;
use std::path::PathBuf;

#[derive(Parser, Debug, Default)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// Input Jack pipe to record from with an optional name
    #[arg(short = 'i', long, action = clap::ArgAction::Append, long_help = "Input Jack pipe to record from with an optional name\n\n\
		    The Jack pipes are specified as: `<client>:<port>`.  Multiple ports can be\n\
		    specified for multi-channel recording:\n\
		    `-i <client_a>:<port_a> -i <client_b>:<port_b>`")]
    pub inputs: Vec<String>,

    #[arg(short = 'o', long, action = clap::ArgAction::Append, long_help = "Output Jack pipe to playaudio to with an optional name\n\n\
		    The Jack pipes are specified as: `<client>:<port>`.  Multiple ports can be\n\
		    specified for multi-channel recording:\n\
		    `-o <client_a>:<port_a> -o <client_b>:<port_b>`.")]
    pub outputs: Vec<String>,

    /// Backing track for immediate overdubbing
    #[arg(short = 'b', long)]
    pub backing_track: Option<PathBuf>,

    /// Directory to write files to
    #[arg(
        short = 'd',
        long,
        long_help = "Directory to write files to\n\n\
		     If a directory is specified audio files for each input will be written to the\n\
		     directory. "
    )]
    pub directory: Option<String>,

    /// Name for audio and metadata files
    #[arg(
        short = 'f',
        long,
        default_value = "qzn3t",
        long_help = "File name touse for files"
    )]
    pub file_name: String,

    /// If this is not None run a command directly.  Only some
    /// commands make sense
    #[arg(short = 'k', long)]
    pub kommand: Option<Command>,

    /// If set all stdout will be suppressed.
    #[arg(
        short = 's',
        long,
        default_value_t = false,
        long_help = "Suppress all stdout"
    )]
    pub silent: bool,
}

#[derive(Debug, Clone, ValueEnum, PartialEq, Hash, Eq)]
pub enum Command {
    Continue, // Used if no menu item selected
    DubAccept,
    DubReview,
    Dubing,
    Quit,
    Play,
    Record,
    ReviewRecord,
    Stop,
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
                Command::Quit => "Quit",
                _ => "Unknown command {self:?}",
            }
        )
    }
}
