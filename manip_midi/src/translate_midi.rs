// Copyright (c) 2025 Worik Turei Stanton
// License: GPL-3.0

//! Reads a stream of MIDI data from the stdin
//! Writes the data on the stdout with MIDI data translated
//! Midi status is not translated but the data can be
use crate::midi_byte_reader::MidiByteReader;
use crate::midi_status::MidiStatus;
use std::collections::HashMap;
use std::env;
use std::error::Error;
use std::fs::File;
use std::io::{self, Read, Write};
use std::iter::FromIterator;
use std::num::ParseIntError;
mod midi_byte_reader;
mod midi_status;
#[derive(Debug)]
/// Translate a status and input byte into a differnt byte.
struct TranslateTable {
    table: HashMap<u16, u8>,
}
impl TranslateTable {
    fn get(&self, idx: &u16) -> Option<&u8> {
        self.table.get(idx)
    }
    #[allow(dead_code)]
    fn is_empty(&self) -> bool {
        self.table.is_empty()
    }
    #[allow(dead_code)]
    fn len(&self) -> usize {
        self.table.len()
    }
    /// Build the table to translate MIDI inputs.  Make a HashMap keyed by
    /// the status of the messages to change, the index of the byte
    /// (\[0,1\]) in the message, and the message itself.  The value is the
    /// message to output in its stead.
    fn new(description: &str) -> Result<TranslateTable, Box<dyn Error>> {
        description
            .lines()
            .filter(|&s| s.trim().starts_with("t "))
            .map(|s| {
                let parts: Vec<&str> = s[2..].split_whitespace().collect();
                if parts.len() != 4 {
                    return Err(format!(
                        "Invalid translation line: {}.  `parts.len()`: {}",
                        s,
                        parts.len()
                    )
                    .into());
                }
                let s = str_u8(parts[0])?; // Status byte
                let x = str_u8(parts[1])?; // index byte
                let k = str_u8(parts[2])?; // Value to translate
                let v = str_u8(parts[3])?; // Output
                let key = TranslateTable::make_key(s, x, k);
                Ok((key, v))
            })
            .collect()
    }
    /// Make a key for the translation table.  Combine the 4 bits of
    /// status `s` with the index `x` in the message (0 or 1) in the
    /// MSB and put the value to translate in the LSB of the key.
    /// The index is in [0..1].  The only MIDI messages that have more
    /// than two data bytes following are:
    /// * Sysex messages.  This programme does not translate those
    /// * NoteOn/NoteOff: These can be followed by an arbitrary number of
    ///   pairs of bytes for note/volume.  So when dealing with data for
    ///   these messages only need to know if the byte is at an odd
    ///   address (relative to status) which means it is "note", or at an
    ///   even address, in which case it is "volume"
    fn make_key(s: u8, x: u8, k: u8) -> u16 {
        ((s as u16 | x as u16) << 8) | (k as u16)
    }
}
impl FromIterator<(u16, u8)> for TranslateTable {
    fn from_iter<I: IntoIterator<Item = (u16, u8)>>(iter: I) -> Self {
        TranslateTable {
            table: HashMap::from_iter(iter),
        }
    }
}
/// Hold state for translating streams of MIDI bytes
struct Translator {
    /// Working memory
    working: Vec<u8>,
    status: Option<MidiStatus>,
    in_channel: Option<u8>,
    out_channel: Option<u8>,
    translator_table: TranslateTable,
}
impl Translator {
    fn new(
        in_channel: Option<u8>,
        out_channel: Option<u8>,
        translator_table: TranslateTable,
    ) -> Self {
        Self {
            working: vec![],
            status: None,
            in_channel,
            out_channel,
            translator_table,
        }
    }
    fn write_working(&self) {
        //w: &Vec<u8>) {
        let w = &self.working;
        io::stdout()
            .write_all(w)
            .unwrap_or_else(|e| panic!("Cannot write to stdout: {}", e));
        io::stdout().flush().expect("Failed to flush stdout");
    }
    // fn get_status(&self) -> Option<MidiStatus> {
    //     self.status
    // }
    fn is_empty(&self) -> bool {
        self.working.is_empty()
    }
    fn translate(&mut self, byte: u8, status: &MidiStatus) -> Result<Option<&u8>, Box<dyn Error>> {
        match status.arg_count() {
            1 => {
                let key = TranslateTable::make_key(status.to_byte(), 0, byte);
                Ok(self.translator_table.get(&key))
            }
            2 => {
                // Two bytes.  So index can be 0, or 1
                // The working memory has the status byte and whatever
                // bytes of data have been received before this one.
                // NoteOn and NotOff have a running status so there
                // can be an arbitrary number of bytes in working
                // memory.  If there are an odd number of bytes in
                // working, index is 0, else it is 1
                let idx = if self.working.len() % 2 == 1 { 0 } else { 1 };
                let key = TranslateTable::make_key(status.to_byte(), idx, byte);
                Ok(self.translator_table.get(&key))
            }
            _ => {
                // These values of status should not be seen as they have no data part
                Err(format![
                    "Error: midi_translate: Got data: {byte:x} but status: {status:?} has no data"
                ]
                .into())
            }
        }
    }
    /// A MIDI byte
    fn add_byte(&mut self, byte: u8) -> Result<(), Box<dyn Error>> {
        // If `in_channel` is Some then check the MIDI byte is on that
        // channel.
        // Check if `byte` is a status byte or a data byte.  Status
        // bytes are not translated
        if byte & 0x80 == 0x80 {
            // This a status byte.
            if let Some(channel) = self.in_channel
                && (byte & 0x0f) != channel
            {
                // Not on ths channel.  Not an error.  Reset
                // `self.status` and drop the byte
                self.status = None;
                return Ok(());
            }
            // If `out_channel` Some then set the channel
            let byte = if let Some(channel) = self.out_channel {
                (byte & 0xf0) | channel
            } else {
                byte
            };
            self.status = Some(
                MidiStatus::from_byte(byte)
                    .unwrap_or_else(|| panic!["Invalid status byte: {byte}"]),
            );
            // The working memory should be empty.  If not, clear and
            // report it
            if !self.is_empty() {
                eprintln!(
                    "Error translate_midi: Working memory not empty: input: {byte:x} working: {:?}",
                    self.working
                );
                self.truncate();
            }
            // Keep the status byte in working memory
            self.working.push(byte);
        } else {
            // Data byte
            match self.status {
                Some(status) => {
                    let byte = match self.translate(byte, &status)? {
                        Some(byte) => *byte,
                        None => {
                            // No translation for `byte`
                            // Pass untranslated bytes through
                            byte
                        }
                    };
                    self.working.push(byte);
                    self.write_working();
                    self.truncate();
                }
                // If the status is none, this message was on another
                // channel that is not being monitored
                None => {
                    return Ok(());
                }
            }
        };
        Ok(())
    }
    // fn len(&self) -> usize {
    //     self.working.len()
    // }
    fn truncate(&mut self) {
        self.working.truncate(0)
    }
}
// #[derive(Debug)]
// The channel is either `Literal`, `value` is the new channel or
// `Minus` the incoming channel has `value` subtracted or `Plus`
// where `value` is added to the incomming channel.  It is perfectly
// possible to have an invalid channel.  See
// [this error](TranslateError::InvalidChannel)
// enum ChannelOperation {
//     Literal,
//     Minus,
//     Plus,
// }
fn main() {
    if let Err(err) = inner_main() {
        eprintln!("Error translate_midi failed: {err:?}");
    }
}
fn inner_main() -> Result<(), Box<dyn Error>> {
    let cfg_file_name = env::args()
        .nth(1)
        .expect("A configuration file name on the command line");
    // The contents of the configuration file as a `String`
    let mut s: String = "".to_string();
    File::open(&cfg_file_name)
        .unwrap_or_else(|e| panic!("{e:?}: Could not open file: {cfg_file_name}"))
        .read_to_string(&mut s)?;
    let translation_table: TranslateTable = TranslateTable::new(&s)?;
    let in_channel: Option<u8> = match s.lines().find(|l| l.starts_with("ci ")) {
        Some(l) => Some(l[3..].parse()?),
        None => None,
    };
    let out_channel: Option<u8> = match s.lines().find(|l| l.starts_with("co ")) {
        Some(l) => Some(l[3..].parse()?),
        None => None,
    };
    // Read stdin a byte at a time
    let stdin = io::stdin();
    let byte_reader: &mut dyn MidiByteReader = &mut stdin.lock();
    let mut translator = Translator::new(in_channel, out_channel, translation_table);
    // While MIDI data is incoming
    while let Some(byte) = byte_reader.read_byte()? {
        translator.add_byte(byte)?;
    }
    if !translator.is_empty() {
        translator.write_working();
        translator.truncate();
    }
    Ok(())
}
/// Helper function for reading `u8` from `&str`.  Hex if prefixed
/// with "0x", else decimal
fn str_u8(inp: &str) -> Result<u8, ParseIntError> {
    if inp.len() > 1 && &inp[..2] == "0x" {
        u8::from_str_radix(inp.trim_start_matches("0x"), 16)
    } else {
        inp.parse::<u8>()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::fmt::Write;
    #[test]
    fn test_str_u8() {
        // Test decimal parsing
        assert_eq!(str_u8("10").unwrap(), 10);
        assert_eq!(str_u8("255").unwrap(), 255);
        // Test hex parsing
        assert_eq!(str_u8("0xA").unwrap(), 10);
        assert_eq!(str_u8("0xFF").unwrap(), 255);
        assert_eq!(str_u8("0x0F").unwrap(), 15);
        // Test invalid cases
        assert!(str_u8("256").is_err()); // Overflow
        assert!(str_u8("0xG").is_err()); // Invalid hex
        assert!(str_u8("abc").is_err()); // Invalid decimal
    }
    #[test]
    fn test_make_key() {
        // Status 0x90, index 0, value 0x3C
        assert_eq!(TranslateTable::make_key(0x90, 0, 0x3C), 0x9000 | 0x3C);
        // Status 0x80, index 1, value 0x40
        assert_eq!(TranslateTable::make_key(0x80, 1, 0x40), 0x8100 | 0x40);
        // Status 0xB0, index 0, value 0x07
        assert_eq!(TranslateTable::make_key(0xB0, 0, 0x07), 0xB000 | 0x07);
    }
    #[test]
    fn test_make_table_invalid_line() {
        // Missing one field
        let input = "t 0x90 0 0x3C";
        assert!(TranslateTable::new(input).is_err());
        // Invalid number format
        let input = "t 0x90 0 abc 0x40";
        assert!(TranslateTable::new(input).is_err());
        // Invalid line prefix
        let input = "x 0x90 0 0x3C 0x40";
        let result = TranslateTable::new(input).unwrap();
        assert!(result.is_empty());
    }
    #[test]
    fn test_make_table_mixed_lines() {
        let input = r#"
# Comment line
t 0x90 0 0x3C 0x40
t 0x90 1 0x40 0x3C
t 0x80 0 0x3C 0x40
# Another comment
t 0x80 1 0x40 0x3C
t 0x0c 0 0 1
	"#;
        let result = TranslateTable::new(input).unwrap();
        assert_eq!(result.len(), 5);
        assert_eq!(
            result.get(&TranslateTable::make_key(0x90, 0, 0x3C)),
            Some(&0x40)
        );
        assert_eq!(
            result.get(&TranslateTable::make_key(0x80, 1, 0x40)),
            Some(&0x3C)
        );
        assert_eq!(result.get(&TranslateTable::make_key(0x0c, 0, 0)), Some(&1));
    }
    fn to_hex(bytes: &[u8]) -> String {
        bytes
            .iter()
            .fold(String::with_capacity(bytes.len() * 3), |mut s, b| {
                write!(&mut s, "{:02x} ", b).unwrap();
                s
            })
    }
    // Integration test for the main processing logic
    #[test]
    fn test_midi_processing_logic() {
        // Create a simple translation table
        let mut translation_table = HashMap::new();
        // Translate note 0x3C to 0x40 when status is 0x90 and it's the first data byte
        translation_table.insert(TranslateTable::make_key(0x90, 0, 0x3C), 0x40);
        // Translate velocity 0x40 to 0x3C when status is 0x90 and it's the second data byte
        translation_table.insert(TranslateTable::make_key(0x90, 1, 0x40), 0x3C);
        // Test MIDI message processing
        let test_cases = vec![
            // Note On message (status 0x90, note 0x3C, velocity 0x40)
            // Should be translated to (status 0x90, note 0x40, velocity 0x3C)
            (vec![0x90, 0x3C, 0x40], vec![0x90, 0x40, 0x40]),
            // Note On message with different values that shouldn't be translated
            (vec![0x90, 0x3D, 0x41], vec![0x90, 0x3D, 0x41]),
            // Different status byte (0x80) shouldn't be translated
            (vec![0x80, 0x3C, 0x40], vec![0x80, 0x3C, 0x40]),
            // System Exclusive message should pass through unchanged
            (
                vec![0xF0, 0x01, 0x02, 0x03, 0xF7],
                vec![0xF0, 0x01, 0x02, 0x03, 0xF7],
            ),
        ];
        for (input, expected) in test_cases {
            let mut working = Vec::new();
            let mut status = None;
            let mut output = Vec::new();
            for byte in input {
                if byte & 0x80 == 0x80 {
                    // Status byte
                    status = MidiStatus::from_byte(byte);
                    working.push(byte);
                } else {
                    // Data byte
                    if let Some(status_byte) = status.as_ref().map(|s| s.to_byte()) {
                        let x = (working.len() % 2) as u8;
                        let key = TranslateTable::make_key(status_byte, x, byte);
                        let v = translation_table.get(&key).copied().unwrap_or(byte);
                        working.push(v);
                    } else {
                        working.push(byte);
                    }
                }
                // Simulate the write_working function
                if !working.is_empty() {
                    output.extend_from_slice(&working);
                    working.clear();
                }
            }
            assert_eq!(to_hex(&output), to_hex(&expected));
        }
    }
    // Test for handling incomplete messages
    #[test]
    fn test_incomplete_messages() {
        let translation_table: HashMap<u16, u8> = HashMap::new(); // Empty table
        let input = vec![0x90, 0x3C]; // Missing velocity byte
        let mut working = Vec::new();
        let mut status = None;
        let mut output = Vec::new();
        for byte in input {
            if byte & 0x80 == 0x80 {
                status = MidiStatus::from_byte(byte);
                working.push(byte);
            } else if let Some(status_byte) = status.as_ref().map(|s| s.to_byte()) {
                let x = (working.len() % 2) as u8;
                let key = TranslateTable::make_key(status_byte, x, byte);
                let v = translation_table.get(&key).copied().unwrap_or(byte);
                working.push(v);
            } else {
                working.push(byte);
            }
            if !working.is_empty() {
                output.extend_from_slice(&working);
                working.clear();
            }
        }
        // The incomplete message should still be output
        assert_eq!(output, vec![0x90, 0x3C]);
    }
}
