// Copyright (c) 2025 Worik Turei Stanton
// License: GPL-3.0

//! Read MIDI on standin.
//! Write out the MIDI as HEX pairs
//! Each ststus message starts a new line
use std::error::Error;
use std::io::Read;
use std::io::{self, Write};
mod midi_status;
fn main() -> Result<(), Box<dyn Error>> {
    // Read stdin a byte at a time
    let mut buffer = [0u8; 1];
    let stdin = io::stdin();
    let mut handle = stdin.lock(); // Lock the stdin handle for efficient reading
    let mut stdout = io::stdout();
    loop {
        match handle.read(&mut buffer) {
            Ok(0) =>
            // EOF
            {
                break;
            }
            Err(e) => return Err(Box::new(e)),
            Ok(2..) => panic!("Cannot happen"),
            Ok(1) => {
                let byte = buffer[0];
                if byte & 0x80 == 0x80 {
                    // status
                    stdout.write_all(format!("\n{byte:x}: ").as_bytes())?;
                } else {
                    // Data byte
                    stdout.write_all(format!("{byte:x} ").as_bytes())?;
                }
                stdout.flush()?;
            }
        }
    }
    Ok(())
}
