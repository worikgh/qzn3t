// Copyright (c) 2025 Worik Turei Stanton
// License: GPL-3.0

use std::error::Error;
use std::io::Read;
pub trait MidiByteReader {
    fn read_byte(&mut self) -> Result<Option<u8>, Box<dyn Error>>;
}
impl<R: Read> MidiByteReader for R {
    fn read_byte(&mut self) -> Result<Option<u8>, Box<dyn Error>> {
        let mut buffer = [0u8; 1];
        match self.read(&mut buffer) {
            Ok(0) => Ok(None), // EOF
            Ok(1) => Ok(Some(buffer[0])),
            Err(e) => Err(Box::new(e)),
            _ => panic!("Unexpected read size"),
        }
    }
}
