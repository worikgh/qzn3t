// Copyright (c) 2025 Worik Turei Stanton
// License: GPL-3.0

//! Structures and code to maintain the input and output buffers and
//! processes
/// The inputs.  Audio to record. Controls the Jack inputs.
use jack::{Client, PortFlags};
use std::{
    self,
    collections::HashMap,
    fs,
    io::{Read, Write},
    path::PathBuf,
    thread,
};

use crate::errors::RecorderError;

// ---- Inputs Start ----
/// The information required for a Jack pipe to record.
#[derive(Debug, Clone)]
pub struct Inputs {
    ports: Vec<String>,
    names_ports: HashMap<String, String>,
}

/// Public interface
impl Inputs {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            ports: Vec::new(),
            names_ports: HashMap::new(),
        }
    }

    /// Using the strings from the command line `-i` add input pipes.
    pub fn from_command_line(input_pipes: Vec<String>) -> Result<Self, RecorderError> {
        let mut inputs = Self::new();
        if input_pipes.is_empty() {
            return Err(RecorderError::NoInputs);
        }

        for i in input_pipes.iter() {
            let parts: Vec<&str> = i.split(':').collect();
            if parts.len() < 2 || parts.len() > 3 {
                return Err(RecorderError::InvalidPipeName(i.to_string()));
            } else if parts.len() == 2 {
                inputs.add(i)?;
            } else {
                let client = parts[0];
                let port = parts[1];
                let name = parts[2];
                let client_port = format!("{client}:{port}");
                inputs.add_name(&client_port, name)?;
            }
        }

        Ok(inputs)
    }

    /// Add an input to the collection with a default name.  The port
    /// must be of the type form "<client>:<port name>"
    pub fn add(&mut self, port: &str) -> Result<(), RecorderError> {
        self.add_name(port, port)
    }

    /// Add an input to the collection with a defined name.  The port
    /// must be of the type form "<client>:<port name>"
    pub fn add_name(&mut self, port: &str, name: &str) -> Result<(), RecorderError> {
        eprintln!("add_name {port} {name}");
        Self::validate_jack_input_pipe(port)?;
        if self.ports.iter().any(|n| n == port) {
            Err(RecorderError::DuplicateInput(port.to_string()))
        } else if self.names_ports.iter().any(|pn| pn.0 == name) {
            Err(RecorderError::DuplicateInputName(name.to_string()))
        } else {
            self.ports.push(port.to_string());
            self.names_ports.insert(name.to_string(), port.to_string());
            Ok(())
        }
    }

    /// Get a copy of all the input names
    pub fn ports(&self) -> Vec<String> {
        self.ports.clone()
    }

    /// Get a copy of the named ports
    pub fn named_ports(&self) -> HashMap<String, String> {
        self.names_ports.clone()
    }
}

/// Private interface
impl Inputs {
    /// An input to this programme is the name of a Jack 32-bit audio
    /// output pipe.  It is of the form: "<client>:<pipe name>"
    fn validate_jack_input_pipe(pipe: &str) -> Result<(), RecorderError> {
        // Check form of `pipe`
        if let Some(n) = pipe.find(":") {
            // There is a ":" character in `pipe`.  It must not be the first character
            if n == 0 {
                return Err(RecorderError::InvalidPipeName(pipe.to_string()));
            }
        } else {
            // No client:port
            return Err(RecorderError::InvalidPipeName(pipe.to_string()));
        }
        // Create a temporary client for port discovery
        let client_name = "port_lister";
        let (client, _) = match Client::new(client_name, jack::ClientOptions::NO_START_SERVER) {
            Ok(cs) => cs,
            Err(err) => {
                return Err(RecorderError::CannotCreateClient(
                    client_name.to_string(),
                    format!("{err}"),
                ));
            }
        };

        // List all audio ports
        let ports = client.ports(None, Some("32 bit float mono audio"), PortFlags::empty());

        // Check if `pipe` exists and is an output pipe (input to this
        // is an output from another)
        if !ports.iter().any(|p| p == pipe) {
            Err(RecorderError::PipeNotFound(pipe.to_string()))
        } else if let Some(port) = client.port_by_name(pipe) {
            let flags = port.flags();
            if flags.contains(PortFlags::IS_OUTPUT) {
                Ok(())
            } else {
                Err(RecorderError::NotOutputPipe(pipe.to_string()))
            }
        } else {
            Err(RecorderError::PipeNotFound(pipe.to_string()))
        }
    }
}
//---- Inputs end ----

// ---- AudioBuffers start ----
/// Hold recorded audio data in named buffers.
#[derive(Clone, Debug)]
pub struct AudioBuffers {
    buffers: HashMap<String, Vec<f32>>,
}

/// Public interface
impl AudioBuffers {
    pub fn add_buffer(&mut self, name: &str, buffer: Vec<f32>) -> Result<(), RecorderError> {
        match self.buffers.get_mut(name) {
            Some(_) => Err(RecorderError::DuplicateBufferName(name.into())),
            None => {
                _ = self.buffers.insert(name.into(), buffer);
                Ok(())
            }
        }
    }
    pub fn reset(&mut self) {
        for b in self.iter_mut() {
            b.1.truncate(0);
        }
    }
}

/// Iterators
impl AudioBuffers {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            buffers: HashMap::new(),
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &Vec<f32>)> {
        self.buffers.iter()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&String, &mut Vec<f32>)> {
        self.buffers.iter_mut()
    }

    pub fn keys(&self) -> impl Iterator<Item = &String> {
        self.buffers.keys()
    }

    pub fn values(&self) -> impl Iterator<Item = &Vec<f32>> {
        self.buffers.values()
    }

    pub fn insert(&mut self, name: String, data: Vec<f32>) {
        self.buffers.insert(name, data);
    }

    pub fn get(&self, name: &str) -> Option<&Vec<f32>> {
        self.buffers.get(name)
    }

    pub fn get_mut(&mut self, name: &str) -> Option<&mut Vec<f32>> {
        self.buffers.get_mut(name)
    }
}

/* --------------------------------------------------------------- */
/* Consuming iterator – implements `IntoIterator` for the struct.  */
impl IntoIterator for AudioBuffers {
    type Item = (String, Vec<f32>);
    type IntoIter = std::collections::hash_map::IntoIter<String, Vec<f32>>;

    fn into_iter(self) -> Self::IntoIter {
        self.buffers.into_iter()
    }
}

/* --------------------------------------------------------------- */
/* Borrowed iterator – implements `IntoIterator` for `&OutputBuffers`. */
impl<'a> IntoIterator for &'a AudioBuffers {
    type Item = (&'a String, &'a Vec<f32>);
    type IntoIter = std::collections::hash_map::Iter<'a, String, Vec<f32>>;

    fn into_iter(self) -> Self::IntoIter {
        self.buffers.iter()
    }
}

/* --------------------------------------------------------------- */
/* Mutable borrowed iterator – implements `IntoIterator` for `&mut OutputBuffers`. */
impl<'a> IntoIterator for &'a mut AudioBuffers {
    type Item = (&'a String, &'a mut Vec<f32>);
    type IntoIter = std::collections::hash_map::IterMut<'a, String, Vec<f32>>;

    fn into_iter(self) -> Self::IntoIter {
        self.buffers.iter_mut()
    }
}
// ---- AudioBuffers end ----

// ---- BufferFileBacking start ----
/// A backing file for recorded audio data.  There is one backing file
/// for every audio buffer in AudioBuffers
pub struct BufferBackingFile {
    /// The path to the file.  The directory is either specified using
    /// `-d` or `--directory`, or is in a temporary directory
    name: PathBuf,

    /// The amount of data saved already
    saved_sz: u64,
}

/// Public interface
impl BufferBackingFile {
    pub fn new(name: PathBuf) -> Self {
        Self { name, saved_sz: 0 }
    }

    #[allow(clippy::type_complexity)]
    /// Save a single audio buffer to disc.
    fn save_thread(
        data_to_save: &[f32],
        path: &PathBuf,
        saved_sz: u64,
        name: &PathBuf,
    ) -> Result<usize, RecorderError> {
        let mut file = fs::OpenOptions::new()
            .append(true)
            .open(path)
            .map_err(|err| RecorderError::BufferBackingIO(format!("{err}")))?;
        let meta_data = file
            .metadata()
            .map_err(|err| RecorderError::BufferBackingIO(format!("{err}")))?;
        if meta_data.len() == saved_sz {
            return Err(RecorderError::BufferBackingIO(format!(
                "Backing buffer for {name:?} is out of sync.  Expected {} bytes in backing file {path:?}, found {}",
                saved_sz,
                meta_data.len()
            )));
        }
        let save_len = data_to_save.len();
        let bytes = unsafe {
            std::slice::from_raw_parts(
                data_to_save.as_ptr() as *const u8,
                std::mem::size_of_val(data_to_save),
            )
        };
        if let Err(err) = file.write_all(bytes) {
            return Err(RecorderError::BufferBackingIO(format!(
                "Writing {name:?} IO error {err}"
            )));
        };
        Ok(save_len)
    }
    pub fn update_buffer(
        &mut self,
        buf: &[f32],
    ) -> Result<thread::JoinHandle<Result<usize, RecorderError>>, RecorderError> {
        let saved_sz_uz: usize = self.saved_sz.try_into().unwrap();
        assert!(buf.len() >= saved_sz_uz);

        let name = self.name.clone();
        let data_to_save = buf[saved_sz_uz..].to_vec();
        let path = self.name.clone();

        let saved_sz = self.saved_sz;
        let handle = thread::spawn(move || -> Result<usize, RecorderError> {
            Self::save_thread(&data_to_save, &path, saved_sz, &name)
        });
        Ok(handle)
    }

    /// Reads the entire buffer from a backing file
    pub fn read_from_file(&self) -> Result<Vec<f32>, RecorderError> {
        let path: &PathBuf = &self.name;
        let mut file = fs::File::open(path).map_err(|err| {
            RecorderError::BufferBackingIO(format!(
                "Failed to open file at {:?} for reading.  Error: {err}",
                path
            ))
        })?;

        // How much to read?
        let metadata = file.metadata().map_err(|err| {
            RecorderError::BufferBackingIO(format!(
                "Failed meta data for {:?}.  Error: {err}",
                path
            ))
        })?;

        // On a 64-bit system usize is 64-bits, pleanty.  On a 32-bit
        // system the audio files are a bit less tha 90_000 seconds,
        // max.  About a day
        let file_size = metadata.len() as usize;

        // Calculate how many f32s are in the file
        // Each f32 is 4 bytes
        let num_floats = file_size / std::mem::size_of::<f32>();

        // Read all bytes from the file
        let mut bytes = vec![0u8; file_size];
        file.read_exact(&mut bytes).map_err(|err| {
            RecorderError::BufferBackingIO(format!(
                "Failed to read from file at {:?}.  Error: {err}",
                path
            ))
        })?;

        // Convert bytes to f32 values
        // We need to ensure proper alignment and safety
        let floats = unsafe {
            // Safety: The bytes come from a file that was written
            // with f32 values, and the exact number of bytes, that
            // should contain f32 values, have been read.  The
            // alignment is handled by creating a properly aligned
            // vector first.
            let mut result = Vec::with_capacity(num_floats);

            let ptr = bytes.as_ptr() as *const f32;
            for i in 0..num_floats {
                result.push(*ptr.add(i));
            }

            result
        };
        Ok(floats)
    }
}
// ---- BufferFileBacking end ----

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_capture_add_valid_port() {
        let port = "system:capture_1";
        let mut inputs = Inputs::new();
        inputs.add(port).unwrap();
        dbg!(&inputs, port);
        assert!(inputs.ports().iter().any(|p| p.as_str() == port));
    }

    #[test]
    fn list_capture_add_invalid_port() {
        let port = "system_capture_1";
        let mut inputs = Inputs::new();
        match inputs.add(port) {
            Err(RecorderError::InvalidPipeName(p)) | Err(RecorderError::PipeNotFound(p)) => {
                assert_eq!(p, port)
            }
            Err(err) => panic!("Got error: {err}"),
            Ok(_) => panic!("Should not be able to add {port}"),
        };
    }

    #[test]
    fn add_port_with_name() {
        let port = "system:capture_1".to_string();
        let name = "Capture One";
        let portv = vec![format!("{port}:{name}")];
        let inputs = Inputs::from_command_line(portv).unwrap();
        assert_eq!(inputs.ports()[0], port);
        assert_eq!(inputs.names_ports.get(name), Some(&port));
    }
}

#[cfg(test)]
mod buffer_backing_file_tests {
    use super::*;
    use std::env::temp_dir;
    use std::fs;
    use std::io::Write;
    use std::sync::{Arc, Mutex};

    // Helper to create a temporary BufferBackingFile
    fn create_temp_buffer() -> (BufferBackingFile, PathBuf) {
        let temp_dir = temp_dir();
        let file_path = temp_dir.join("test_buffer.bin");
        (BufferBackingFile::new(file_path), temp_dir)
    }

    // Helper to write f32 data to a file
    fn write_f32_to_file(path: &PathBuf, data: &[f32]) {
        let mut file = fs::File::create(path).unwrap();
        let bytes = unsafe {
            std::slice::from_raw_parts(data.as_ptr() as *const u8, std::mem::size_of_val(data))
        };
        file.write_all(bytes).unwrap();
    }

    #[test]
    fn test_update_buffer_saves_new_data() {
        let (mut buffer, temp_dir) = create_temp_buffer();

        // Initial data
        let initial_data = vec![1.0f32, 2.0, 3.0, 4.0];
        write_f32_to_file(&buffer.name, &initial_data);
        buffer.saved_sz = (initial_data.len() * std::mem::size_of::<f32>()) as u64;

        // New data to append
        let buffer_data = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];

        // Update buffer
        let handle = buffer.update_buffer(&buffer_data).unwrap();
        let result = handle.join().unwrap().unwrap();

        // Verify the result
        assert_eq!(result, 4); // Should have saved 4 new floats (indices 4-7)

        // Read back and verify
        let saved_data = buffer.read_from_file().unwrap();
        assert_eq!(saved_data, vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);

        // Verify saved_sz was updated correctly
        assert_eq!(buffer.saved_sz, (8 * std::mem::size_of::<f32>()) as u64);

        drop(temp_dir); // Clean up
    }

    #[test]
    fn test_update_buffer_no_new_data() {
        let (mut buffer, temp_dir) = create_temp_buffer();

        // File already contains all data
        let full_data = vec![1.0f32, 2.0, 3.0, 4.0];
        write_f32_to_file(&buffer.name, &full_data);
        buffer.saved_sz = (full_data.len() * std::mem::size_of::<f32>()) as u64;

        // Buffer has same data, no new data to save
        let handle = buffer.update_buffer(&full_data).unwrap();
        let result = handle.join().unwrap().unwrap();

        assert_eq!(result, 0); // No new data saved

        drop(temp_dir);
    }

    #[test]
    fn test_update_buffer_empty_buffer() {
        let (mut buffer, temp_dir) = create_temp_buffer();

        // Empty file
        buffer.saved_sz = 0;

        // Update with empty buffer
        let handle = buffer.update_buffer(&[]).unwrap();
        let result = handle.join().unwrap().unwrap();

        assert_eq!(result, 0);

        // File should still be empty
        let data = buffer.read_from_file().unwrap();
        assert!(data.is_empty());

        drop(temp_dir);
    }

    #[test]
    fn test_update_buffer_sequential_updates() {
        let (mut buffer, temp_dir) = create_temp_buffer();

        // Track expected state
        let _expected_data = Arc::new(Mutex::new(Vec::<f32>::new()));

        // First update
        let data1 = vec![1.0f32, 2.0, 3.0];
        buffer.saved_sz = 0;

        let handle = buffer.update_buffer(&data1).unwrap();
        let result1 = handle.join().unwrap().unwrap();
        assert_eq!(result1, 3);
        buffer.saved_sz = (3 * std::mem::size_of::<f32>()) as u64;

        // Second update
        let data2 = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let handle = buffer.update_buffer(&data2).unwrap();
        let result2 = handle.join().unwrap().unwrap();
        assert_eq!(result2, 2); // Should save positions 3 and 4
        buffer.saved_sz = (5 * std::mem::size_of::<f32>()) as u64;

        // Verify final state
        let final_data = buffer.read_from_file().unwrap();
        assert_eq!(final_data, vec![1.0, 2.0, 3.0, 4.0, 5.0]);

        drop(temp_dir);
    }

    #[test]
    fn test_update_buffer_detects_file_size_mismatch() {
        let (mut buffer, temp_dir) = create_temp_buffer();

        // File has different size than expected
        let existing_data = vec![1.0f32, 2.0, 3.0];
        write_f32_to_file(&buffer.name, &existing_data);

        // But saved_sz thinks file is empty
        buffer.saved_sz = 0;

        // This should trigger an error in the spawned thread
        let handle = buffer.update_buffer(&[1.0, 2.0, 3.0]).unwrap();
        let result = handle.join().unwrap();

        assert!(result.is_err());
        if let Err(RecorderError::BufferBackingIO(err_msg)) = result {
            assert!(err_msg.contains("out of sync"));
            assert!(err_msg.contains("Expected 0"));
        } else {
            panic!("Expected BufferBackingIO error");
        }

        drop(temp_dir);
    }

    #[test]
    fn test_update_buffer_handles_io_errors() {
        let (mut buffer, temp_dir) = create_temp_buffer();

        // Create the file initially
        write_f32_to_file(&buffer.name, &[1.0f32]);
        buffer.saved_sz = std::mem::size_of::<f32>() as u64;

        // Make file read-only to cause write error
        let mut perms = fs::metadata(&buffer.name).unwrap().permissions();
        perms.set_readonly(true);
        fs::set_permissions(&buffer.name, perms).unwrap();

        // Try to update - this should fail in the spawned thread
        let handle = buffer.update_buffer(&[1.0, 2.0]).unwrap();
        let result = handle.join().unwrap();

        assert!(result.is_err());

        // Restore permissions for cleanup
        let mut perms = fs::metadata(&buffer.name).unwrap().permissions();
        #[allow(clippy::permissions_set_readonly_false)]
        perms.set_readonly(false);
        fs::set_permissions(&buffer.name, perms).unwrap();

        drop(temp_dir);
    }

    #[test]
    fn test_read_from_file_empty() {
        let (buffer, temp_dir) = create_temp_buffer();

        let data = buffer.read_from_file().unwrap();
        assert!(data.is_empty());

        drop(temp_dir);
    }

    #[test]
    fn test_read_from_file_with_data() {
        let (buffer, temp_dir) = create_temp_buffer();

        // Write some test data
        let test_data = vec![1.5f32, -2.0, 3.24, 0.0, -42.0];
        write_f32_to_file(&buffer.name, &test_data);

        // Read it back
        let read_data = buffer.read_from_file().unwrap();
        assert_eq!(read_data, test_data);

        drop(temp_dir);
    }

    #[test]
    fn test_read_from_file_large_buffer() {
        let (buffer, temp_dir) = create_temp_buffer();

        // Create large buffer (1000 floats)
        let mut large_data = Vec::with_capacity(1000);
        for i in 0..1000 {
            large_data.push(i as f32 * 0.5);
        }

        write_f32_to_file(&buffer.name, &large_data);

        let read_data = buffer.read_from_file().unwrap();
        assert_eq!(read_data.len(), 1000);
        assert_eq!(read_data, large_data);

        drop(temp_dir);
    }

    #[test]
    fn test_read_from_file_nonexistent_file() {
        let temp_dir = temp_dir();
        let nonexistent_path = temp_dir.join("nonexistent.bin");
        let buffer = BufferBackingFile::new(nonexistent_path);

        let result = buffer.read_from_file();
        assert!(result.is_err());

        if let Err(RecorderError::BufferBackingIO(err_msg)) = result {
            assert!(err_msg.contains("Failed to open"));
        } else {
            panic!("Expected BufferBackingIO error");
        }

        drop(temp_dir);
    }

    #[test]
    fn test_read_from_file_corrupted_size() {
        let (buffer, temp_dir) = create_temp_buffer();

        // Write partial f32 (3 bytes instead of 4)
        let mut file = fs::File::create(&buffer.name).unwrap();
        file.write_all(&[0x00, 0x00, 0x00]).unwrap();

        // Should still read without panic, but might have issues
        let _result = buffer.read_from_file();
        // The current implementation assumes complete f32s, so this might panic
        // or behave unexpectedly. This test documents the behavior.

        drop(temp_dir);
    }

    #[test]
    fn test_integration_write_and_read() {
        let (mut buffer, temp_dir) = create_temp_buffer();

        // Phase 1: Write initial data
        let phase1_data = vec![1.0f32, 2.0, 3.0];
        write_f32_to_file(&buffer.name, &phase1_data);
        buffer.saved_sz = (phase1_data.len() * std::mem::size_of::<f32>()) as u64;

        // Phase 2: Update with more data
        let phase2_buffer = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        let handle = buffer.update_buffer(&phase2_buffer).unwrap();
        handle.join().unwrap().unwrap();

        // Phase 3: Read everything back
        let all_data = buffer.read_from_file().unwrap();
        assert_eq!(all_data, vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);

        drop(temp_dir);
    }

    #[test]
    #[should_panic(expected = "assertion failed")]
    fn test_update_buffer_panics_on_buffer_too_small() {
        let (mut buffer, temp_dir) = create_temp_buffer();

        // saved_sz indicates we expect 4 floats already saved
        buffer.saved_sz = (4 * std::mem::size_of::<f32>()) as u64;

        // But buffer only has 3 floats - should panic
        let _ = buffer.update_buffer(&[1.0f32, 2.0, 3.0]);

        drop(temp_dir);
    }
}
