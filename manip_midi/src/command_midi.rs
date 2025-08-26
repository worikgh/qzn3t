// Copyright (c) 2025 Worik Turei Stanton
// License: GPL-3.0

//! Read MIDI on standin.
//! Respond to ControlChange messages by running commands
use std::collections::HashMap;
use std::env;
use std::error::Error;
use std::fs::File;
use std::io;
use std::io::Read;
use std::process::Command;
mod midi_status;
use crate::midi_status::MidiStatus;
fn run_command(command: &str) -> Result<(), Box<dyn Error>> {
    Command::new(command).status()?;
    Ok(())
}
fn make_table(description: &str) -> Result<(HashMap<u8, String>, u8), Box<dyn Error>> {
    let mut r1 = HashMap::new();
    let lines: Vec<&str> = description.lines().collect();
    for s in lines.iter() {
        if !s.starts_with("x ") {
            continue;
        }
        let (byte, command) = s[2..]
            .split_once(' ')
            .ok_or(format!("Line '{}' has invalid format", s))?;
        let byte: u8 = byte.parse()?;
        r1.insert(byte, command.to_string());
    }
    let channel = description
        .lines()
        .rev() // If more than one, use last
        .find(|s| s.starts_with("c "))
        .unwrap_or("0");
    let channel: u8 = channel.parse()?;
    Ok((r1, channel))
}
fn main() -> Result<(), Box<dyn Error>> {
    let cfg_file_name = env::args()
        .nth(1)
        .expect("Configuration file on command line");
    let mut s: String = "".to_string();
    let mut file = File::open(&cfg_file_name)
        .unwrap_or_else(|e| panic!("{e:?}: Could not open file: {cfg_file_name}"));
    file.read_to_string(&mut s)
        .expect("Could not read file contents");
    let (command_table, channel): (HashMap<u8, String>, u8) = make_table(&s)?;
    // Track MIDI status
    let mut status: Option<MidiStatus> = None;
    // Read stdin a byte at a time
    let mut buffer = [0u8; 1];
    let stdin = io::stdin();
    let mut handle = stdin.lock(); // Lock the stdin handle for efficient reading
    loop {
        match handle.read(&mut buffer) {
            Ok(0) =>
            // EOF
            {
                break
            }
            Err(e) => return Err(Box::new(e)),
            Ok(2..) => panic!("Cannot happen"),
            Ok(1) => {
                let byte = buffer[0];
                if byte & 0x80 == 0x80 {
                    // status
                    if byte & 0x0f == channel {
                        // Status byte on this channel:
                        status = MidiStatus::from_byte(byte);
                    } else {
                        status = None;
                    }
                } else {
                    // Data byte
                    if let Some(MidiStatus::ProgramChange(_)) = status.as_ref() {
                        // Expecting a possible command
                        if let Some(command) = command_table.get(&byte) {
                            run_command(command)?;
                        }
                    }
                }
            }
        }
    }
    Ok(())
}
