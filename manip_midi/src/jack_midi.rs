// Copyright (c) 2025 Worik Turei Stanton
// License: GPL-3.0

//! Read MIDI on stdin.  Make Jack connections Once configured this
//! programme takes MIDI inputs and edits Jackd audio connections The
//! MIDI responded to are Programme Change messages.  The channel can
//! be optionally set, and defaults to channel 0 The connections are
//! defined in a local directory 'PEDALS/' Originally this was used to
//! activate pedal efects for
//! [120Pedal](https://gitgub.com/worikgh/120Pedal.git).
use crate::jack_connections::JackConnections;
use crate::midi_byte_reader::MidiByteReader;
use crate::midi_status::MidiStatus;
use std::collections::{HashMap, HashSet};
use std::env;
use std::error::Error;
use std::fs::File;
use std::io::Read;
use std::io::{self};
use std::path::Path;
use std::path::PathBuf;
mod jack_connections;
mod midi_byte_reader;
mod midi_status;
/// The path to where pedal information is held.
const PEDAL_DIR: &str = "PEDALS/";
/// A trait for (un)making Jack connections.
pub trait JackConnectionHandler {
    fn make_jack(&mut self, src: &str, dst: &str) -> Result<(), Box<dyn Error>>;
    fn unmake_jack(&mut self, src: &str, dst: &str) -> Result<(), Box<dyn Error>>;
}
/// Implementation of `JackConnectionHandler`.  (This pattern of using
/// a trait rather than using `JackConnections::make_connection`
/// directly makes testing easier.  Teh `JackConnections` can be
/// mocked
impl JackConnectionHandler for JackConnections {
    // ...
    fn make_jack(&mut self, src: &str, dst: &str) -> Result<(), Box<dyn Error>> {
        self.make_connection(src, dst)?;
        Ok(())
    }
    fn unmake_jack(&mut self, src: &str, dst: &str) -> Result<(), Box<dyn Error>> {
        self.unmake_connection(src, dst)?;
        Ok(())
    }
}
/// A "Jack connection" is the name of an input pipe and the name of
/// an output pipe.  The input pipe (inputs to the simulator) is
/// connected to "system:capture_N" pipes, and output pipes (will be)
/// connected to "qzn3t_mixer:input_N" pipes.  Each pedal is defined
/// in a file "pedal_N".  Each input is from "system:capture_1" and
/// for the file "pedal_Q" the output is (outputs are) sent to
/// "qzn3t_mixer:input_Q".  (For now each simulator uses only one
/// output. If there are more tan one output they are combined).
/// Return a HashMap from the index value to a vector of src/dst pair.
/// This vector will have one value until the code is extended for
/// multi-channel effects
#[allow(clippy::type_complexity)]
pub fn make_table(
    description: &str,
) -> Result<(HashMap<u8, Vec<(String, String)>>, u8), Box<dyn Error>> {
    let mut table = HashMap::new();
    let dir_path = Path::new(PEDAL_DIR);
    // The paths to PEDAL files indexed by `u8`
    let mut activation_files: HashMap<u8, PathBuf> = HashMap::new();
    for line in description.lines().filter(|l| l.starts_with("j ")) {
        let nf: Vec<&str> = line.split_whitespace().collect();
        let f_name = nf[3];
        let path = dir_path.join(f_name);
        if !path.is_file() {
            return Err(format!("Invalid name for PEDAL file: {f_name}").into());
        }
        let number = nf[2].parse::<u8>()?;
        if number == 0 {
            // Invalid pedal file name
            eprintln!("Error jack_midi: Invalid configuration line: {line}");
            continue;
        }
        activation_files.insert(number, path);
    }
    // For each activation file
    for (k, path) in activation_files.iter() {
        let file_name = path.to_str().unwrap();
        let mut file = match File::open(file_name) {
            Ok(f) => f,
            Err(err) => {
                eprintln!("Error jack_midi: Failed to open file:{file_name}. Err: {err:?}");
                return Err(Box::new(err));
            }
        };
        let mut jack_cfg = String::new();
        file.read_to_string(&mut jack_cfg)?;
        let lines = jack_cfg.lines();
        let mut jack_pairs: Vec<(String, String)> = Vec::new();
        for line in lines {
            // Each line must be of the form "<src jack pipe> <sink
            // jack pipe>".  Each must have "system:playback_N" or
            // "system:capture_N".  Jack pipes do not contain
            // whitespace
            let mut src_dst = line.split_whitespace();
            let src = src_dst
                .next()
                .ok_or(format!("A bad jack description: {line}"))?;
            let dst = src_dst
                .next()
                .ok_or(format!("A bad jack description: {line}"))?;
            if !src.contains("system:capture") && !dst.contains("system:playback") {
                eprintln!("Error jack_midi. Invalid Jack I/O: src{src} -> dst: {dst}");
                continue;
            }
            // The output is now directed to the input of the mixer so
            // volume of each pedal board can be set at runtime
            if dst.contains("system:playback") {
                // The name of the mixer input pipe is
                // `qzn3t_mixer:input_N` where `N` is `k`.  `k`
                // starts at 1
                let dst = format!("qzn3t_mixer:input_{k}",);
                jack_pairs.push((src.to_string(), dst.to_string()));
            } else {
                jack_pairs.push((src.to_string(), dst.to_string()));
            }
        }
        table.insert(*k, jack_pairs);
    }
    let channel: u8 = description
        .lines()
        .rev() // If more than one, use last
        .find(|s| s.starts_with("c "))
        .unwrap_or("c 0")[2..]
        .parse()?;
    Ok((table, channel))
}
pub fn run<B: MidiByteReader, J: JackConnectionHandler + std::fmt::Debug>(
    byte_reader: &mut B,
    command_table: &HashMap<u8, Vec<(String, String)>>,
    channel: u8,
    jack_connections: &mut J,
) -> Result<(), Box<dyn Error>> {
    // Track MIDI status
    let mut status: Option<MidiStatus> = None;
    // The currently selected effect
    let mut effect: Option<u8> = None;
    // Record connections set so can be idempotent
    let mut connected: HashSet<(&str, &str)> = HashSet::new();
    // Ensure that all the connections in `command_table` are disconnected
    for civ in command_table.iter() {
        for ci in civ.1.iter() {
            _ = jack_connections.unmake_jack(&ci.0, &ci.1);
        }
    }
    loop {
        let byte = match byte_reader.read_byte() {
            Ok(o) => match o {
                Some(b) => b,
                None => {
                    eprintln!("DBG jack_midi: Break from main loop  No byte from input");
                    break;
                }
            },
            Err(err) => panic!("Error:{err} midi_jack"),
        };
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
                if let Some(jack_pipes) = command_table.get(&byte) {
                    // Changing the pedal to byte
                    // Have jack connections to establish in `jack_pipes`
                    for jc in jack_pipes.iter() {
                        if !connected.contains(&(&jc.0, &jc.1)) {
                            jack_connections.make_jack(&jc.0, &jc.1)?;
                            connected.insert((&jc.0, &jc.1));
                        }
                    }
                    // If there were old ones disconnect them
                    if let Some(old_jack) = effect {
                        let old_pipes = command_table
                            .get(&old_jack)
                            .ok_or(format!("old_jack: {old_jack} INVALID"))?;
                        for op in old_pipes {
                            if !jack_pipes.iter().any(|jp| jp.0 == op.0 && jp.1 == op.1) {
                                // Not in the set just connected so disconnect
                                jack_connections.unmake_jack(&op.0, &op.1)?;
                                connected.remove(&(&op.0, &op.1));
                            }
                        }
                    }
                    effect = Some(byte);
                } else {
                    eprintln!("Error jack_midi: No command for byte: {byte:x}");
                    eprintln!("Error jack_midi: command_table: {command_table:?}");
                }
            }
        }
    }
    Ok(())
}
/// Build the command table from a file name
#[allow(clippy::type_complexity)]
pub fn load_configuration(
    cfg_file_name: &str,
) -> Result<(HashMap<u8, Vec<(String, String)>>, u8), Box<dyn Error>> {
    let mut s = String::new();
    let mut file = match File::open(cfg_file_name) {
        Ok(f) => f,
        Err(err) => {
            eprintln!("Error jack_midi: Opening configuration: {err:?}");
            return Err(Box::new(err));
        }
    };
    file.read_to_string(&mut s)?;
    make_table(&s)
}
fn inner_main() -> Result<(), Box<dyn Error>> {
    let mut jack_connections = JackConnections::new("midi_client")?;
    let cfg_file_name = env::args()
        .nth(1)
        .expect("Configuration file on the command line");
    let (command_table, channel): (HashMap<u8, Vec<(String, String)>>, u8) =
        load_configuration(&cfg_file_name)?;
    run(
        &mut io::stdin().lock(),
        &command_table,
        channel,
        &mut jack_connections,
    )
}
fn main() {
    if let Err(err) = inner_main() {
        eprintln!("Error jack_midi failed: {err:?}");
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::fmt::{Display, Formatter, Result as FmtResult};
    use std::io::Cursor;
    // Mock implementation for testing JackConnectionHandler
    #[derive(Debug)]
    struct MockJackConnectionHandler {
        made_connections: Vec<(String, String)>,
        unmade_connections: Vec<(String, String)>,
    }
    #[derive(Debug)]
    struct MockJackError {
        pub what: String,
    }
    impl Display for MockJackError {
        fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
            write!(f, "MockJackError: {}", self.what)
        }
    }
    impl Error for MockJackError {}
    impl MockJackConnectionHandler {
        fn new(connections: &[(String, String)]) -> Self {
            MockJackConnectionHandler {
                // Start with no connections made
                made_connections: Vec::new(),
                // Start with all connections unmade
                unmade_connections: connections.to_vec(),
            }
        }
    }
    impl JackConnectionHandler for MockJackConnectionHandler {
        fn make_jack(&mut self, src: &str, dst: &str) -> Result<(), Box<dyn Error>> {
            if self
                .made_connections
                .contains(&(src.to_string(), dst.to_string()))
            {
                Err(Box::new(MockJackError {
                    what: format!("({src}, {dst}) is already connected"),
                }))
            } else {
                self.made_connections
                    .push((src.to_string(), dst.to_string()));
                self.unmade_connections
                    .retain(|c| c != &(src.to_string(), dst.to_string()));
                Ok(())
            }
        }
        fn unmake_jack(&mut self, src: &str, dst: &str) -> Result<(), Box<dyn Error>> {
            if self
                .unmade_connections
                .contains(&(src.to_string(), dst.to_string()))
            {
                Err(Box::new(MockJackError {
                    what: format!("({src}, {dst}) is already disconnected"),
                }))
            } else {
                self.unmade_connections
                    .push((src.to_string(), dst.to_string()));
                self.made_connections
                    .retain(|c| c != &(src.to_string(), dst.to_string()));
                Ok(())
            }
        }
    }
    // Tests for make_table function
    mod make_table_tests {
        use super::*;
        #[test]
        fn test_empty_config() {
            let config = "";
            let result = make_table(config);
            assert!(result.is_ok());
            let (table, channel) = result.unwrap();
            assert_eq!(table.len(), 0);
            assert_eq!(channel, 0);
        }
        #[test]
        fn test_channel_config() {
            let config = "c 5\n";
            let result = make_table(config);
            assert!(result.is_ok());
            let (_, channel) = result.unwrap();
            assert_eq!(channel, 5);
        }
        #[test]
        fn test_multiple_channel_config() {
            let config = "c 1\nc 5\n";
            let result = make_table(config);
            assert!(result.is_ok());
            let (_, channel) = result.unwrap();
            assert_eq!(channel, 5); // should take the last one
        }
        #[test]
        fn test_invalid_channel_config() {
            let config = "c abc\n";
            let result = make_table(config);
            assert!(result.is_err());
        }
        #[test]
        fn test_single_jack_config() {
            let config = "j 1 test_jack.txt\n";
            // This will fail because the file doesn't exist
            let result = make_table(config);
            assert!(result.is_err());
        }
        #[test]
        fn test_invalid_jack_config_line() {
            let config = "j abc\n";
            let result = make_table(config);
            assert!(result.is_err());
        }
    }
    // Tests for MidiByteReader implementation
    mod midi_byte_reader_tests {
        use super::*;
        #[test]
        fn test_read_byte() {
            let mut data = Cursor::new(vec![0x90u8, 0x40, 0x7F]);
            let reader: &mut dyn MidiByteReader = &mut data;
            assert_eq!(reader.read_byte().unwrap(), Some(0x90));
            assert_eq!(reader.read_byte().unwrap(), Some(0x40));
            assert_eq!(reader.read_byte().unwrap(), Some(0x7F));
            assert_eq!(reader.read_byte().unwrap(), None);
        }
        #[test]
        fn test_read_byte_error() {
            struct ErrorReader;
            impl Read for ErrorReader {
                fn read(&mut self, _buf: &mut [u8]) -> io::Result<usize> {
                    Err(io::Error::other("test error"))
                }
            }
            let mut reader = ErrorReader;
            let result = reader.read_byte();
            assert!(result.is_err());
        }
    }
    // Tests for run function
    mod run_tests {
        use super::*;
        fn create_test_table() -> HashMap<u8, Vec<(String, String)>> {
            let mut table = HashMap::new();
            table.insert(
                1,
                vec![
                    ("src1".to_string(), "dst1".to_string()),
                    ("src2".to_string(), "dst2".to_string()),
                ],
            );
            table.insert(
                2,
                vec![
                    ("src1".to_string(), "dst1".to_string()),
                    ("src3".to_string(), "dst3".to_string()),
                ],
            );
            table
        }
        fn table_connections(input: &HashMap<u8, Vec<(String, String)>>) -> Vec<(String, String)> {
            input.values().flatten().cloned().collect()
        }
        #[test]
        fn test_run_with_program_change() {
            let table = create_test_table();
            let mut mock_jack = MockJackConnectionHandler::new(&table_connections(&table));
            let midi_data = vec![
                0xC0, // Program change on channel 0
                0x01, // Program number 1
            ];
            let mut reader = Cursor::new(midi_data);
            eprintln!(
                "DBG test jack_midi: mock_jack.unmade_connections.len(): {}",
                mock_jack.unmade_connections.len()
            );
            eprintln!(
                "DBG test jack_midi: mock_jack.made_connections.len(): {}",
                mock_jack.made_connections.len()
            );
            run(&mut reader, &table, 0, &mut mock_jack).unwrap();
            assert_eq!(mock_jack.made_connections.len(), 2);
            assert_eq!(
                mock_jack.made_connections[0],
                ("src1".to_string(), "dst1".to_string())
            );
            assert_eq!(
                mock_jack.made_connections[1],
                ("src2".to_string(), "dst2".to_string())
            );
            assert_eq!(mock_jack.unmade_connections.len(), 1);
            assert_eq!(
                mock_jack.unmade_connections[0],
                ("src3".to_string(), "dst3".to_string())
            );
        }
        #[test]
        fn test_run_with_program_change_and_previous_effect() {
            let table = create_test_table();
            let mut mock_jack = MockJackConnectionHandler::new(&table_connections(&table));
            let midi_data = vec![
                0xC0, // Program change on channel 0
                0x01, // Program number 1
                0xC0, // Program change on channel 0
                0x02, // Program number 2
            ];
            let mut reader = Cursor::new(midi_data);
            run(&mut reader, &table, 0, &mut mock_jack).unwrap();
            // First program change
            assert!(
                mock_jack
                    .made_connections
                    .contains(&("src1".to_string(), "dst1".to_string()))
            );
            assert!(
                mock_jack
                    .made_connections
                    .contains(&("src3".to_string(), "dst3".to_string()))
            );
            // Second program change should:
            // 1. Keep src1-dst1 (common to both)
            // 2. Add src3-dst3
            // 3. Remove src2-dst2
            assert!(
                mock_jack
                    .made_connections
                    .contains(&("src3".to_string(), "dst3".to_string()))
            );
            assert_eq!(mock_jack.unmade_connections.len(), 1);
            assert_eq!(
                mock_jack.unmade_connections[0],
                ("src2".to_string(), "dst2".to_string())
            );
        }
        #[test]
        fn test_run_with_wrong_channel() {
            let table = create_test_table();
            let mut mock_jack = MockJackConnectionHandler::new(&table_connections(&table));
            let midi_data = vec![
                0xC1, // Program change on channel 1 (we're listening to channel 0)
                0x01, // Program number 1
            ];
            let mut reader = Cursor::new(midi_data);
            run(&mut reader, &table, 0, &mut mock_jack).unwrap();
            assert_eq!(mock_jack.made_connections.len(), 0);
            assert_eq!(mock_jack.unmade_connections.len(), 4);
        }
        #[test]
        fn test_run_with_non_program_change_message() {
            let table = create_test_table();
            let mut mock_jack = MockJackConnectionHandler::new(&table_connections(&table));
            let midi_data = vec![
                0x90, // Note on (not program change)
                0x40, // Note number
                0x7F, // Velocity
            ];
            let mut reader = Cursor::new(midi_data);
            run(&mut reader, &table, 0, &mut mock_jack).unwrap();
            assert_eq!(mock_jack.made_connections.len(), 0);
            assert_eq!(mock_jack.unmade_connections.len(), 4);
        }
        #[test]
        fn test_run_with_unknown_program() {
            let table = create_test_table();
            let mut mock_jack = MockJackConnectionHandler::new(&table_connections(&table));
            assert_eq!(mock_jack.unmade_connections.len(), 4);
            let midi_data = vec![
                0xC0, // Program change on channel 0
                0x03, // Program number 3 (not in our table)
            ];
            let mut reader = Cursor::new(midi_data);
            let result = run(&mut reader, &table, 0, &mut mock_jack);
            assert!(result.is_ok()); // Unknown programs should be ignored, not cause errors
            assert_eq!(mock_jack.made_connections.len(), 0);
            assert_eq!(mock_jack.unmade_connections.len(), 4);
        }
    }
    // Tests for load_configuration function
    mod load_configuration_tests {
        use super::*;
        use std::fs;
        use tempfile::NamedTempFile;
        #[test]
        fn test_load_valid_configuration() {
            let file = NamedTempFile::new().unwrap();
            let config_content = "j 1 test_jack.txt\nc 3\n";
            fs::write(file.path(), config_content).unwrap();
            // This will fail because test_jack.txt doesn't exist
            let result = load_configuration(file.path().to_str().unwrap());
            assert!(result.is_err());
        }
        #[test]
        fn test_load_nonexistent_file() {
            let result = load_configuration("nonexistent_file.txt");
            assert!(result.is_err());
        }
    }
}
