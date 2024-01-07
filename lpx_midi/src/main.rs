//! Send arbitrary MIDI to a Novation LPX Pad
//! ["Programmer's Manual" ](https://fael-downloads-prod.focusrite.com/customer/prod/s3fs-public/downloads/Launchpad%20X%20-%20Programmers%20Reference%20Manual.pdf)
extern crate midir;
extern crate serde;

use midir::{MidiOutput, MidiOutputConnection};
use std::env;
use std::error::Error;
use std::result::Result;

// Get a MIDI port that has a name containing `keyword`
fn get_midi_port<T: midir::MidiIO>(
    midi_io: &T,
    keyword: &str,
) -> Option<T::Port> {
    for port in midi_io.ports() {
        let name = match midi_io.port_name(&port) {
            Ok(name) => name,
            Err(_) => continue,
        };

        if name.contains(keyword) {
            return Some(port);
        }
    }

    None
}

/// Create an output MIDI port to the LPX.
/// It uses the passed parameter `name` to create a prort: LpxCtl:<name>
fn get_midi_out(name: &str) -> Result<MidiOutputConnection, Box<dyn Error>> {
    let midi_output = MidiOutput::new("LpxCtl")?;
    let port = get_midi_port(&midi_output, "Launchpad X LPX MIDI In").unwrap(); //.ok_or(Err("Failed guess port".into())?);
    Ok(midi_output.connect(&port, name)?)
}

fn testable(midi: Vec<u8>) -> Result<(), Box<dyn Error>> {
    // Collect the MIDI to send

    // Create an output port to the LPX for sending it midi.
    let mut midi_port: MidiOutputConnection = get_midi_out("midi_port")?;

    match midi_port.send(&midi) {
        Ok(()) => (),
        Err(err) => eprintln!("{err}: Failed to send msg to LPX: {midi:?}"),
    };
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    // The only argument is a configuration file
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        panic!("Pass some MIDI as arguments");
    }

    let args: Vec<u8> = args
        .iter()
        .skip(1)
        .map(|x| match x.parse::<u8>() {
            Ok(x) => x,
            Err(err) => panic!("{err}: Invald input: {x}"),
        })
        .collect();
    testable(args)
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    /// LED lighting SysEx message
    /// Not a proper test.  More of a demo
    fn half_red() {
        testable(vec![240, 0, 32, 41, 2, 12, 14, 1, 247]).unwrap();
        testable(vec![
            240, 0, 32, 41, 2, 12, 3, 0, 11, 5, 0, 12, 5, 0, 13, 5, 0, 14, 5,
            0, 15, 5, 0, 16, 5, 0, 17, 5, 0, 18, 5, 0, 21, 5, 0, 22, 5, 0, 23,
            5, 0, 24, 5, 0, 25, 5, 0, 26, 5, 0, 27, 5, 0, 28, 5, 0, 31, 5, 0,
            32, 5, 0, 33, 5, 0, 34, 5, 0, 35, 5, 0, 36, 5, 0, 37, 5, 0, 38, 5,
            0, 41, 5, 0, 42, 5, 0, 43, 5, 0, 44, 5, 0, 45, 5, 0, 46, 5, 0, 47,
            5, 0, 48, 5, 247,
        ])
        .unwrap();
    }
}
