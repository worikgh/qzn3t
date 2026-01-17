// Copyright (c) 2025 Worik Turei Stanton
// License: GPL-3.0

//! Open a virtual MIDI port and output the MIDI messages on the
//! standard output.  This was designed to consume messages from
//! `aplaymidi` that sends data as <u64, [u8]> which is the timestamp
//! followed by the MIDI message
use chrono::Utc;
use midir::MidiInput;
use midir::os::unix::VirtualInput;
use std::error::Error;
use std::io::{self, Write};
use std::sync::mpsc::{Receiver, channel};
use std::thread;
use std::time::Duration as StdDuration;
const PORT_NAME: &str = "read_midi_virtual_port";
struct MidiOutput {
    rx: Option<Receiver<(u64, Vec<u8>)>>,
}

impl MidiOutput {
    fn new(rx: Receiver<(u64, Vec<u8>)>) -> Self {
        Self { rx: Some(rx) }
    }
    fn run(&mut self) {
        // Take the receiver from the Option, leaving None behind
        let rx = self.rx.take().expect("Receiver should be present");
        let mut ts_last_o: Option<u64> = None;
        let mut now = Utc::now();
        let handle: thread::JoinHandle<_> = thread::spawn(move || {
            loop {
                let msg = match rx.recv() {
                    Ok(msg) => msg,
                    Err(err) => {
                        eprintln!(
                            "Error read_midi_virtual: MidiOutput.run: Reading message: {err}"
                        );
                        continue;
                    }
                };

                // When `run` is called the time stamp on the first
                // message is unknown
                if ts_last_o.is_none() {
                    ts_last_o = Some(msg.0);
                }
                // Now `ts_last_o` holds data

                // Decode the data intop timestamp and MIDI message
                let ts = msg.0;
                let midi_msg = msg.1;

                // `td1` is the time since the last message received
                let td1 = (Utc::now() - now).num_microseconds().unwrap();
                now = Utc::now();

                // `td2` is the difference, in ms, between this mesage
                // time stamp and the last one
                let td2 = (ts - ts_last_o.as_ref().unwrap()) as i64;
                ts_last_o = Some(ts);

                let sleep_micro = td2 - td1;
                let sleep_tolerance = 10_000;
                if sleep_micro > sleep_tolerance {
                    // The message is not ready to send yet.  It came
                    // early by more than `sleep_tolerance`
                    // micro-seconds

                    thread::sleep(StdDuration::from_micros(sleep_micro as u64));
                } else if sleep_micro < -sleep_tolerance {
                    eprintln!("Error read_midi_virtual run: Behind {} μs", -sleep_micro);
                }

                io::stdout()
                    .write_all(&midi_msg)
                    .unwrap_or_else(|e| panic!("Cannot write to stdout: {}", e));
                io::stdout()
                    .flush()
                    .expect("read_midi: Failed to flush stdout");
            }
        });
        let join_result = handle.join();
        let Err(err) = join_result;
        eprintln!("Error read_midi_virtual run: Join error: {err:?}");
    }
}
fn create_virtual_midi_port_with_logging(port_name: &str) -> Result<(), Box<dyn Error>> {
    // Create a virtual MIDI input port to receive messages
    let midi_in = MidiInput::new(port_name)?;

    // Create the virtual port with a callback to handle incoming messages
    let (sx, rx) = channel::<(u64, Vec<u8>)>();
    let mut midi_output = MidiOutput::new(rx);
    let _conn_in = midi_in.create_virtual(
        port_name,
        move |timestamp_ms, message, _| {
            if let Err(err) = sx.send((timestamp_ms, message.to_vec())) {
                panic!("{err}");
            }
        },
        (),
    )?;

    eprintln!("Created virtual MIDI port: {}", port_name);
    eprintln!("Use with: aplaymidi -p \"{}\" your_file.mid", port_name);
    eprintln!("Waiting for MIDI messages...");
    eprintln!("{}", "=".repeat(60));

    // Keep the program running
    midi_output.run();
    loop {
        thread::sleep(StdDuration::from_secs(1));
    }
}
#[allow(dead_code)]
fn get_message_type(status_byte: u8) -> String {
    match status_byte {
        0x80..=0x8F => "Note Off".to_string(),
        0x90..=0x9F => "Note On".to_string(),
        0xA0..=0xAF => "Polyphonic Aftertouch".to_string(),
        0xB0..=0xBF => "Control Change".to_string(),
        0xC0..=0xCF => "Program Change".to_string(),
        0xD0..=0xDF => "Channel Aftertouch".to_string(),
        0xE0..=0xEF => "Pitch Bend Change".to_string(),
        0xF0 => "System Exclusive (Begin)".to_string(),
        0xF1 => "MIDI Timecode Quarter Frame".to_string(),
        0xF2 => "Song Position Pointer".to_string(),
        0xF3 => "Song Select".to_string(),
        0xF6 => "Tune Request".to_string(),
        0xF7 => "End of System Exclusive".to_string(),
        0xF8 => "Timing Clock".to_string(),
        0xFA => "Start".to_string(),
        0xFB => "Continue".to_string(),
        0xFC => "Stop".to_string(),
        0xFE => "Active Sensing".to_string(),
        0xFF => "System Reset".to_string(),
        _ => format!("Unknown message type: 0x{:02X}", status_byte),
    }
}

#[allow(dead_code)]
fn log_midi_message(timestamp_ms: u64, message: &[u8]) {
    let seconds = timestamp_ms / 1000;
    let ms = timestamp_ms % 1000;
    eprintln!(
        "LOG virtual_midi_port: {seconds}:{ms} {}: {}",
        message
            .iter()
            .map(|m| format!("{m:x}"))
            .collect::<Vec<String>>()
            .join(" "),
        get_message_type(message[0]),
    );
}

fn inner_main() -> Result<(), Box<dyn Error>> {
    create_virtual_midi_port_with_logging(PORT_NAME)
}

fn main() {
    if let Err(err) = inner_main() {
        eprintln!("Error read_midi_virtual: {err}");
    }
}
