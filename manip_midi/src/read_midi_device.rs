// Copyright (c) 2025 Worik Turei Stanton
// License: GPL-3.0

//! Passed a string that is used to identify a MIDI port by name this
//! opens that port. Any MIDI sent to that port is output on the
//! standard out of this
use midir::MidiIO;
use midir::MidiInputPort;
use std::error::Error;
use std::io::{self, Write};
use std::thread;
use std::time::Duration;
const THIS_MIDI_NAME: &str = "120Pedal";
fn main() {
    if let Err(err) = inner_main() {
        eprintln!("Error read_midi: inner_main failed: {err:?}");
    }
}
fn inner_main() -> Result<(), Box<dyn Error>> {
    // The name of the MIDI port.  The first port found that contains
    // this string will be used
    let matches = clap::Command::new("MyApp")
        .version("0.0")
        .about("Reads a MIDI device and outputs any data from that device to stdout")
        .arg(
            clap::Arg::new("list")
                .short('l')
                .long("list")
                .help("List devices")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            clap::Arg::new("port")
                .help("The input MIDI port)")
                .index(1) // Positional argument at index 1
                .value_parser(clap::value_parser!(String)),
        )
        .get_matches();
    let list: bool = *matches.get_one::<bool>("list").unwrap_or(&false);
    // Create the port for MIDI input
    let this_name = THIS_MIDI_NAME.to_string();
    let midi_in = match midir::MidiInput::new(THIS_MIDI_NAME) {
        Ok(m) => m,
        Err(err) => {
            eprintln!("Error: read_midi Failed initialising MIDI input: {err}");
            return Err(Box::new(err));
        }
    };
    if list {
        for mp in midi_in.ports().iter() {
            eprintln!(
                "midi_read.rs: {}",
                midi_in
                    .port_name(mp)
                    .unwrap_or("Error midi_read --list: Failed to get a port's name".to_string())
            );
        }
        return Ok(());
    }
    let name = matches
        .get_one::<String>("port")
        .expect("Must pass port name");
    let this_port: MidiInputPort = match get_midi_port(name, &midi_in) {
        Ok(p) => p,
        Err(err) => {
            eprintln!("Error read_midi: Failed to get MIDI port: {err}");
            return Err(err);
        }
    };
    let _c = match midi_in.connect(
        &this_port,
        format!("{}-in", this_name).as_str(),
        move |_a, b, _| {
            // The meat of this programme.  Simply write all data from
            // MIDI to stdout
            io::stdout()
                .write_all(b)
                .unwrap_or_else(|e| panic!("Cannot write to stdout: {}", e));
            io::stdout()
                .flush()
                .expect("read_midi: Failed to flush stdout");
        },
        (),
    ) {
        Ok(c) => c,
        Err(err) => {
            eprintln!("Error read_midi: Failed to connect to MIDI: {err}");
            return Err(Box::new(err));
        }
    };
    loop {
        thread::sleep(Duration::from_secs(1));
    }
}
/// Get the first MDII port that has `name` as part of its name
fn get_midi_port<T>(name: &str, midi_in: &T) -> Result<MidiInputPort, Box<dyn Error>>
where
    T: MidiIO<Port = MidiInputPort>,
{
    midi_in
        .ports()
        .iter()
        .find(|&port| {
            midi_in
                .port_name(port)
                .map(|port_name| port_name.contains(name))
                .unwrap_or(false)
        })
        .ok_or_else(|| format!("No MIDI port found containing '{}'", name).into())
        .cloned()
}
