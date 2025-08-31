// Copyright (c) 2025 Worik Turei Stanton
// License: GPL-3.0

//! Use a Pad (like the Novation LPX) as a musical instrument
//! Control the colours on the display
//! Translate the MIDI signals from the raw PAD number into
//! noteon/noteoff signals
//! On start up connect directly to the pad device (it must exist and be
//! available) then set up a virtual connection for the MIDI consumer
//! and connect to it later
//! This only works for the Novation LPX
//! ["Programmer's Manual for the Novation LPX" ](https://fael-downloads-prod.focusrite.com/customer/prod/s3fs-public/downloads/Launchpad%20X%20-%20Programmers%20Reference%20Manual.pdf)
extern crate midir;
mod section;

use crate::midir::os::unix::VirtualOutput;
use crate::section::Section;
use crate::section::default_sections;
use clap::{Arg, Command};
use midir::MidiInputPort;
use midir::MidiOutputPort;
use midir::{MidiInput, MidiInputConnection, MidiOutput, MidiOutputConnection};
use std::collections::HashSet;
use std::error::Error;
use std::fs::File;
use std::io::prelude::*;
use std::process::exit;
use std::result::Result;
use std::sync::mpsc::{self, Receiver, Sender};

/// Initialise a vector of `Section` from a file.
/// It is either a list of `Section` in JSON or it is in "linear"
/// style.
/// One record (for a `Section`) per line
/// Comma separated
/// Fields:
/// 1. Sapce separated list of pads in the section
/// 2. Main colour in hex: #RRGGBB
/// 3. Active colour in hex: #RRGGBB
/// 4. MIDI not in decimal or 0x hex
fn load_sections(filename: &str) -> Option<Vec<Section>> {
    let mut file = match File::open(filename) {
        Ok(f) => f,
        Err(err) => panic!("{err}"),
    };
    let mut content = String::new();
    match file.read_to_string(&mut content) {
        Ok(_) => (),
        Err(err) => panic!("{err}"),
    };

    // Create the sections from the file
    let lines: Vec<String> = content.lines().map(|line| line.to_string()).collect();
    let mut sections: Vec<Section> = lines
	.iter()
	.filter(|&l| !l.trim().starts_with('#'))
	.map(|l| {
		let records: Vec<String> = l.split(',').map(|s| s.trim().to_string()).collect();
		// Error checking
		if records.len() != 4 {
		    panic!("Error pad: Invalid line in configuration record.  Wrong number of fields: {}  Line: {l}", records.len());
		}
		let pads = records[0].split_whitespace().map(|s| {
		    let valid_pad = |p:u8| -> bool {
			let row = p % 10 ;
			let col = p / 10;
			row > 0 && row < 9 && col > 0 && col < 9
		    };
		    if let Ok(s) = s.parse::<u8>() && valid_pad(s){
			s
		    }else{
			panic!("Error pad: Invalid line {l}. Pad is invalid: {s}")
		    }
		}).collect();
		let make_colour = |colour_str:&str, name:&str| -> [u8;3] {
		    if colour_str.len() != 7 {
			panic!("Error pad: Invalid line. Colour: {name} is invalid: {}", colour_str);
		    }
		    let hex_colour = &colour_str[1..];

		    let rgb: Result<Vec<u8>, _> = hex_colour.as_bytes().chunks(2).map(|chunk| {
			let hex = std::str::from_utf8(chunk).unwrap();
			u8::from_str_radix(hex, 16)
		    }).collect();

		    match rgb {
			Ok(values) if values.len() == 3 => [values[0], values[1], values[2]],
			_ => panic!("Error pad: Invalid line. Colour: {name} is invalid: {}", colour_str),
		    }
		};
		let main_colour: [u8; 3] = make_colour(&records[1], "main_colour");
		let active_colour: [u8; 3] = make_colour(&records[2], "active_colour");
		let midi_note  =  &records[3];
		let midi_note:u8 = match midi_note.parse::<u8>() {
		    Ok(u) => u,
		    Err(e) => panic!("Error pad: Invalid line. Midid note: {midi_note} is invalid: {e}",),
		};

		Section::new(pads, main_colour, active_colour, midi_note)
	    })
	.collect();

    // If there is a default section with no pads put all un-included pads in it
    if sections.iter().filter(|s| s.pads.is_empty()).count() > 1 {
        panic!(
            "Error pad: Invalid sections in {filename}.  There must be at most one default section"
        );
    }

    // Check all colours are made of tripples in 0..127
    for s in sections.iter() {
        let mc = &s.main_colour;
        for i in mc.iter() {
            if *i > 127 {
                panic!(
                    "Error pad: Invalid main_colour: {mc:?}.  Each component must be in 0..127  Component: {i}"
                );
            }
        }
        let ac = &s.active_colour;
        for i in ac.iter() {
            if *i > 127 {
                panic!(
                    "Error pad: Invalid active_colour: {ac:?}.  Each component must be in 0..127  Component: {i}"
                );
            }
        }
    }

    // Check each pad occurs at most once
    let mut pad_check: HashSet<u8> = HashSet::new();
    for s in sections.iter() {
        for p in s.pads.iter() {
            if pad_check.contains(p) {
                panic!["Error pad: Invalid sections in {filename}. Repeated pad: {p}"];
            }
            pad_check.insert(*p);
        }
    }
    if let Some(index) = sections.iter().position(|x| x.pads.is_empty()) {
        // Collect all pads mentioned so far
        let mut pads_here: Vec<u8> = sections.iter().flat_map(|x| x.pads.clone()).collect();
        if pads_here.len() < 64 {
            // Need the default
            pads_here.sort();
            // Check each row for missing pads and add them to default
            for r in 1..=8 {
                let pads: Vec<&u8> = pads_here.iter().filter(|x| *x / 10 == r).collect();
                for c in 1..=8 {
                    let pad = r * 10 + c;
                    if !pads.iter().any(|x| x == &&pad) {
                        sections[index].pads.push(pad);
                    }
                }
            }
        }
    }
    Some(sections)
}

// Get a MIDI port that has a name containing `keyword`
fn get_midi_port<T: midir::MidiIO>(midi_io: &T, keyword: &str) -> Option<T::Port> {
    for port in midi_io.ports() {
        let name = match midi_io.port_name(&port) {
            Ok(name) => name,
            Err(_) => continue,
        };
        // eprintln!("DBG pad: get_midi_port(midi_io, {keyword}) name: {name}");

        if name.contains(keyword) {
            eprintln!(
                "DBG pad: get_midi_port(midi_io, {keyword}) from keyword: {keyword} name: {name}"
            );
            return Some(port);
        }
    }

    None
}

/// Create an output MIDI port to the pad device.
/// It uses the passed parameter `name` to create a port: `PadCtl:<name>`
fn get_midi_out(name: &str, p_name: &str) -> Result<MidiOutputConnection, Box<dyn Error>> {
    let midi_output = MidiOutput::new("PadCtl")?;
    let port = get_midi_port(&midi_output, p_name)
        .ok_or(format!["Failed to get MIDI port {p_name} -> PAD"])?;
    Ok(midi_output.connect(&port, name)?)
}

/// Create a MIDI input port, connected from the pad device MIDI port.
/// `name` is the port name for the created port
/// `f` is the function that takes a channel and sends the MIDI that
/// it wants to handle down that channel
/// `tx` is the channel
fn get_midi_in(
    name: &str,
    p_name: &str,
    f: impl FnMut(u64, &[u8], &mut Sender<[u8; 3]>) + Send + 'static,
    tx: Sender<[u8; 3]>,
) -> Result<MidiInputConnection<Sender<[u8; 3]>>, Box<dyn Error>> {
    let midi_input = MidiInput::new(p_name)?;
    let port = match get_midi_port(&midi_input, p_name) {
        Some(p) => p,
        None => panic!("Failed to find port: {p_name} in get_midi_in"),
    };
    //.ok_or(Err("Failed guess port".into())?);
    let result = midi_input.connect(&port, name, f, tx)?;
    Ok(result)
}

/// Get all MIDI Ports.
fn get_all_midi_input_ports() -> Result<Vec<String>, Box<dyn Error>> {
    let input = MidiInput::new("foo")?;
    let ports: Vec<MidiInputPort> = input.ports();
    let mut result: Vec<String> = vec![];
    for p in ports {
        result.push(input.port_name(&p)?);
    }
    Ok(result)
}
fn get_all_midi_output_ports() -> Result<Vec<String>, Box<dyn Error>> {
    let output = MidiOutput::new("foo")?;
    let ports: Vec<MidiOutputPort> = output.ports();
    let mut result: Vec<String> = vec![];
    for p in ports {
        result.push(output.port_name(&p)?);
    }
    Ok(result)
}

fn main() -> Result<(), Box<dyn Error>> {
    let matches = Command::new("pad")
        .arg(
            Arg::new("list")
                .short('l')
                .long("list")
                .help("List available configurations")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("config")
                .short('c')
                .long("config")
                .help("Path to configuration file")
                .value_name("FILE")
                .required(false),
        )
        .arg(
            Arg::new("pad_midi_out")
                .short('o')
                .long("pad_midi_out")
                .help("Send MIDI to pad on this port")
                .value_name("MIDI_OUT")
                .required(false),
        )
        .arg(
            Arg::new("pad_midi_in")
                .short('i')
                .long("pad_midi_in")
                .help("Get MIDI from pad on this port")
                .value_name("MIDI_IN")
                .required(false),
        )
        .get_matches();

    if *matches.get_one::<bool>("list").unwrap() {
        eprintln!("DBG pad: Input ports:");
        let ports = get_all_midi_input_ports()?;
        for port_name in ports {
            eprintln!("\t{port_name}");
        }
        eprintln!("DBG pad: Output ports:");
        let ports = get_all_midi_output_ports()?;
        for port_name in ports {
            eprintln!("\t{port_name}");
        }
        exit(0);
    }

    let midi_input = matches.get_one::<String>("pad_midi_in").unwrap();
    let midi_output = matches.get_one::<String>("pad_midi_out").unwrap();

    eprintln!("DBG pad: MIDI  input: {}", midi_input);
    eprintln!("DBG pad: MIDI output: {}", midi_output);

    // Initialise the collection of `Section` from the file. (See `section.rs`)
    let sections: Vec<Section> = if let Some(cfg_file_name) = matches.get_one::<String>("config") {
        load_sections(cfg_file_name).expect("Failed to load sections")
    } else {
        default_sections()
    };

    // The channel to send MIDI messages, received from the pad device in the
    // MidiInputConnection, here to the main thread
    let (tx, rx): (Sender<[u8; 3]>, Receiver<[u8; 3]>) = mpsc::channel::<[u8; 3]>();

    // Connect to the pad device to receive pad press events.  `f` is the
    // function that handles input MIDI and sends them back to themain
    // thread
    let f = move |_stamp, message: &[u8], tx: &mut Sender<[u8; 3]>| {
        // let message = MidiMessage::from_bytes(message.to_vec());
        if message.len() == 3 {
            let m3: [u8; 3] = message.try_into().unwrap();
            tx.send(m3).unwrap();
        }
    };
    // The port stays open as long as `_in` is in scope
    let _in = get_midi_in("read_input", midi_output.as_str(), f, tx.clone())?;

    // Create an output port to the pad device for sending it colour.
    let mut colour_port: MidiOutputConnection = get_midi_out("colour_port", midi_input.as_str())?;

    // Selecting Layouts (page 7 programmers manual).  127 => "Programmer Mode"
    let msg: [u8; 9] = [240, 0, 32, 41, 2, 12, 0, 127, 247];
    match colour_port.send(&msg) {
        Ok(()) => (),
        Err(err) => eprintln!("Error pad: {err}: Failed to send msg to pad device: {msg:?}"),
    };

    let make_colour = |section: &Section, colour: [u8; 3]| -> Vec<u8> {
        // Buid the MIDI command that sets the colours of all the pads
        // in a section (they are all the same colour - part of what
        // defines a section).  One long MIDI sysex message that sets
        // many pads in one command

        // "LED lighting SysEx message" programmer's mabual page 15
        let mut colour_message: Vec<u8> = vec![240, 0, 32, 41, 2, 12, 3];
        let pads: Vec<u8> = section.pads().to_vec();
        for pad in pads.iter() {
            colour_message.push(3); // RGB colour
            colour_message.push(*pad); // Pad index
            colour_message.extend(colour.to_vec()); // RGB tripple
        }
        colour_message.push(247); // End message
        colour_message
    };

    // Initialise the colours
    for section in sections.iter() {
        let colour = make_colour(section, section.main_colour);
        eprintln!("DBG qzn3t: Send colour: {colour:?}");
        match colour_port.send(&colour) {
            Ok(()) => (),
            Err(err) => eprintln!("Error pad: {err}: Cannot send colour: {colour:?}"),
        };
    }

    // Establish the output that sends MIDI to whatever software will
    // interpret the MIDI to create sound and MIDI controls to
    // whatever interprets them.  An external programme will have to
    // conmplete these setups as this programme does not know what
    // they will be
    let midi_out: MidiOutput = MidiOutput::new("PadCtlNote")?;
    let port_name = "port";
    let mut midi_note_out_port: MidiOutputConnection = midi_out.create_virtual(port_name)?;

    let midi_out: MidiOutput = MidiOutput::new("PadCtlCtl")?;
    let port_name = "port";
    let mut midi_ctl_out_port: MidiOutputConnection = midi_out.create_virtual(port_name)?;
    eprintln!("DBG pad: Virtual MIDI Output port 'PadCtlNote:{port_name}' is open");
    eprintln!("DBG pad: Virtual MIDI Output port 'PadCtlCtl:{port_name}' is open");

    // Main loop.
    loop {
        let message: [u8; 3] = match rx.recv() {
            Ok(m) => m,
            Err(err) => panic!("{}", err),
        };
        if message[0] == 144 {
            // MIDI noteon

            // Find the section the pad is in
            let pad: u8 = message[1];

            if let Some(section) = sections.iter().find(|x| x.pad_in(pad)) {
                // got the section for a pad

                // Send out the note
                let velocity = message[2];
                let message: [u8; 3] = [message[0], section.midi_note, velocity];
                eprintln!(
                    "DBG pad: SEND NoteOn: Type: {:2x} Note: {:2x} Velocity: {:2x}",
                    message[0], message[1], message[2]
                );
                midi_note_out_port.send(&message)?;

                if velocity > 0 {
                    // Note on
                    // Set colour of section to "active_colour"
                    let active_colour = make_colour(section, section.active_colour);
                    colour_port.send(&active_colour).unwrap();
                } else {
                    // Note off
                    // Restore the colour
                    let main_colour = make_colour(section, section.main_colour);
                    colour_port.send(&main_colour).unwrap();
                }
                continue;
            }
        } else if message[0] == 176 {
            // A control signal
            eprintln!(
                "DBG pad: SEND Ctl: {:2x} {:2x} {:2x}",
                message[0], message[1], message[2]
            );
            midi_ctl_out_port.send(&message).unwrap();
        }
    }
    // Ok(())
}
//
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_load_sections_from_json() {
        let json_content = r#"
	[
	    {
		"pads": [11, 12, 13],
		"main_colour": [127, 0, 0],
		"active_colour": [0, 127, 0],
		"midi_note": 60
	    },
	    {
		"pads": [],
		"main_colour": [0, 0, 0],
		"active_colour": [127, 127, 127],
		"midi_note": 0
	    }
	]
	"#;

        let mut file = NamedTempFile::new().unwrap();
        write!(file, "{}", json_content).unwrap();
        let path = file.path().to_str().unwrap();

        let sections = load_sections(path).unwrap();
        assert_eq!(sections.len(), 2);
        assert_eq!(sections[0].pads, vec![11, 12, 13]);
        assert_eq!(sections[1].pads.len(), 64 - 3);
    }

    #[test]
    fn test_load_sections_from_csv_like() {
        let csv_content = "11 12 13, #7f0000, #007f00, 60\n21 22 23, #00007f, #7f7f00, 61";

        let mut file = NamedTempFile::new().unwrap();
        write!(file, "{}", csv_content).unwrap();
        let path = file.path().to_str().unwrap();

        let sections = load_sections(path).unwrap();
        assert_eq!(sections.len(), 2);
        assert_eq!(sections[0].pads, vec![11, 12, 13]);
        assert_eq!(sections[0].main_colour, [127, 0, 0]);
        assert_eq!(sections[1].pads, vec![21, 22, 23]);
    }
    #[test]
    fn test_load_sections_from_csv_like_with_default() {
        let csv_content = "11 12 13, #7f0000, #007f00, 60\n21 22 23, #00007f, #7f7f00, 61\n, #7f007f, #7f7f7f, 62";

        let mut file = NamedTempFile::new().unwrap();
        write!(file, "{}", csv_content).unwrap();
        let path = file.path().to_str().unwrap();

        let sections = load_sections(path).unwrap();
        assert_eq!(sections.len(), 3);
        assert_eq!(sections[0].pads, vec![11, 12, 13]);
        assert_eq!(sections[0].main_colour, [127, 0, 0]);
        assert_eq!(sections[1].pads, vec![21, 22, 23]);
        assert_eq!(sections[2].pads.len(), 64 - 6);
        assert_eq!(sections[2].main_colour, [0x7f, 0, 0x7f]);
    }

    #[test]
    fn test_load_sections_csv_comments() {
        let csv_content = "11  12 13, #7f0000, #007f00, 60\n # This is a comment line\n21 22 23, #00007f, #7f7f00, 61";

        let mut file = NamedTempFile::new().unwrap();
        write!(file, "{}", csv_content).unwrap();
        let path = file.path().to_str().unwrap();

        let sections = load_sections(path).unwrap();
        assert_eq!(sections.len(), 2);
        assert_eq!(sections[0].pads, vec![11, 12, 13]);
        assert_eq!(sections[0].main_colour, [127, 0, 0]);
        assert_eq!(sections[1].pads, vec![21, 22, 23]);
    }

    #[test]
    fn test_load_sections_with_default_section() {
        let content = r#"
	[
	    {
		"pads": [11, 12],
		"main_colour": [127, 0, 0],
		"active_colour": [0, 127, 0],
		"midi_note": 60
	    },
	    {
		"pads": [],
		"main_colour": [0, 0, 0],
		"active_colour": [127, 127, 127],
		"midi_note": 0
	    }
	]
	"#;

        let mut file = NamedTempFile::new().unwrap();
        write!(file, "{}", content).unwrap();
        let path = file.path().to_str().unwrap();

        let sections = load_sections(path).unwrap();
        assert!(sections[1].pads.len() > 2); // Default section should have been filled
    }

    #[test]
    #[should_panic(expected = "Invalid line in configuration record")]
    fn test_invalid_csv_line() {
        let content = "11 12, #7f0000, #007f00"; // Missing midi_note

        let mut file = NamedTempFile::new().unwrap();
        write!(file, "{}", content).unwrap();
        let path = file.path().to_str().unwrap();

        load_sections(path).unwrap();
    }
    #[test]
    #[should_panic(expected = "There must be at most one default section")]
    fn test_invalid_csv_line_more_default() {
        let content = "11 12, #7f0000, #007f00, 60\n, #7f0000, #007f7f, 61\n, #7f4e00, #e07f7f, 62";

        let mut file = NamedTempFile::new().unwrap();
        write!(file, "{}", content).unwrap();
        let path = file.path().to_str().unwrap();

        load_sections(path).unwrap();
    }

    #[test]
    #[should_panic(expected = "Repeated pad")]
    fn test_invalid_csv_line_repeat_pads() {
        let content =
            "11 12, #7f0000, #007f00, 60\n11, #7f0000, #007f7f, 61\n, #7f4e00, #707f7f, 62";

        let mut file = NamedTempFile::new().unwrap();
        write!(file, "{}", content).unwrap();
        let path = file.path().to_str().unwrap();

        load_sections(path).unwrap();
    }

    #[test]
    #[should_panic(expected = "Invalid section")]
    fn test_invalid_csv_line_repeat_pads_in_section() {
        let content = "11 12, #7f0000, #007f00, 60\n31 41 51 31, #7f0000, #007f7f, 61\n, #7f4e00, #e07f7f, 62";

        let mut file = NamedTempFile::new().unwrap();
        write!(file, "{}", content).unwrap();
        let path = file.path().to_str().unwrap();

        load_sections(path).unwrap();
    }

    #[test]
    #[should_panic(expected = "Colour: main_colour is invalid")]
    fn test_invalid_color_format() {
        let content = "11 12, #7f00, #007f00, 60"; // Invalid color format

        let mut file = NamedTempFile::new().unwrap();
        write!(file, "{}", content).unwrap();
        let path = file.path().to_str().unwrap();

        load_sections(path).unwrap();
    }

    #[test]
    #[should_panic(expected = "Pad is invalid")]
    fn test_invalid_pad_number() {
        let content = "11 99, #7f0000, #007f00, 60"; // Pad 99 is invalid

        let mut file = NamedTempFile::new().unwrap();
        write!(file, "{}", content).unwrap();
        let path = file.path().to_str().unwrap();

        load_sections(path).unwrap();
    }
}
