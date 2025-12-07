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
		    `-i <client_a>:<port_a> -i <client_b>:<port_b>`. The port can be given a name\n\
		    using format `-i <client>:<port>:<name>` If no name is specified then it is\n\
		    `<client>:<port>`")]
    pub input: Vec<String>,

    /// Backing track for immediate overdubbing
    #[arg(short = 'b', long)]
    pub backing_track: Option<PathBuf>,

    /// Directory to write files to
    #[arg(
        short = 'd',
        long,
        long_help = "Directory to write files to\n\n\
		     If a directory is specified audio files for each input will be written to the\n\
		     directory. (TODO: Implement writing these files as recording is underway)"
    )]
    pub directory: Option<String>,

    /// If this is not None run a command directly.  Only some
    /// commands make sense
    #[arg(short = 'k', long)]
    pub kommand: Option<Command>,
}

#[derive(Debug, Clone, ValueEnum, PartialEq, Hash, Eq)]
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
