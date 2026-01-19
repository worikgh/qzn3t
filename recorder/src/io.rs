// Copyright (c) 2025 Worik Turei Stanton
// License: GPL-3.0

//! Structures and code to maintain the input and output buffers and
//! processes

#[allow(unused_imports)]
use crate::{errors::RecorderError, utils::get_sample_rate};
use jack::{Client, PortFlags};
use serde::Serialize;
use std::fs::OpenOptions;
#[allow(unused_imports)]
use std::{
    self,
    collections::HashMap,
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
    sync::mpsc,
    thread,
    time::Duration,
};

// ---- Inputs Start ----
/// Define the Jack pipes to use.  FIXME: This probably can be
/// replaced with `Vec<String>`
#[derive(Debug, Clone)]
pub struct JackPipes {
    // names_ports: HashMap<String, String>,
    ports: Vec<String>,
    input: bool, // True if inputs to recorder
}

/// Public interface
impl JackPipes {
    #[allow(clippy::new_without_default)]
    pub fn new(input: bool) -> Self {
        Self {
            //names_ports: HashMap::new(),
            ports: Vec::new(),
            input,
        }
    }

    /// Using the strings from the command line
    /// [`crate::structs::Args`] `-i` and `-o` add input or output
    /// jack pipes.
    pub fn from_command_line(pipes: &[String], input: bool) -> Result<Self, RecorderError> {
        let mut result = Self::new(input);
        if pipes.is_empty() {
            return Ok(result);
        }

        for i in pipes.iter() {
            let parts: Vec<&str> = i.split(':').collect();
            if parts.len() < 2 || parts.len() > 3 {
                return Err(RecorderError::InvalidPipeName(i.to_string()));
            } else if parts.len() == 2 {
                if result.ports().contains(i) {
                    if input {
                        return Err(RecorderError::DuplicateInput(i.to_string()));
                    } else {
                        return Err(RecorderError::DuplicateOutput(i.to_string()));
                    }
                }
                result.add(i)?;
            } else {
                let client = parts[0];
                let port = parts[1];
                let _name = parts[2];
                let client_port = format!("{client}:{port}");
                result.add(&client_port)?;
            }
        }

        Ok(result)
    }

    /// Add an input to the collection with a default name.  The port
    /// must be of the type form "`client`:`port name`".  The caller
    /// must call this in the correct order so channels (0-based
    /// channel number) and ports (Jack port fully qualified names)
    /// match up correctly.
    pub fn add(&mut self, port: &str) -> Result<(), RecorderError> {
        Self::validate_jack_pipe(port, self.input)?;
        self.ports.push(port.to_string());
        Ok(())
    }

    // /// Add an input to the collection with a defined name.  The port
    // /// must be of the type form "`client`:`port name`".
    // pub fn add_name(&mut self, port: &str, name: &str) -> Result<(), RecorderError> {
    //     Self::validate_jack_input_pipe(port)?;
    //     if self.names_ports.values().any(|n| n == port) {
    //         Err(RecorderError::DuplicateInput(port.to_string()))
    //     } else if self.names_ports.iter().any(|pn| pn.0 == name) {
    //         Err(RecorderError::DuplicateInputName(name.to_string()))
    //     } else {
    //         // self.ports.push(port.to_string());
    //         let channel = self.names_ports.len() as u32;
    //         self.names_ports.insert(name.to_string(), port.to_string());
    //         self.channels_ports.insert(channel, port.to_string());
    //         Ok(())
    //     }
    // }

    /// Get a copy of all the ports
    pub fn ports(&self) -> Vec<String> {
        self.ports.clone()
    }

    // pub fn names(&self) -> Vec<String> {
    //     self.names_ports.keys().map(|n| n.to_string()).collect()
    // }
    // /// Get a copy of the named ports
    // pub fn names_ports(&self) -> HashMap<String, String> {
    //     self.names_ports.clone()
    // }
    // /// Get a copy of the numbered (matching channel)  ports
    // pub fn channels_ports(&self) -> HashMap<u32, String> {
    //     self.ports.clone()
    // }
}

/// Private interface
impl JackPipes {
    /// An input to this programme is the name of a Jack 32-bit audio
    /// output pipe.  It is of the form: "<client>:<pipe name>"
    fn validate_jack_pipe(pipe: &str, input: bool) -> Result<(), RecorderError> {
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

        // Check if `pipe` exists and is an correct type (`input` =>
        // PortFlags::IS_OUTPUT.  Input to this is an output from
        // another)
        if !ports.iter().any(|p| p == pipe) {
            Err(RecorderError::PipeNotFound(pipe.to_string()))
        } else if let Some(port) = client.port_by_name(pipe) {
            let flags = port.flags();
            if input {
                if flags.contains(PortFlags::IS_OUTPUT) {
                    Ok(())
                } else {
                    Err(RecorderError::NotOutputPipe(pipe.to_string()))
                }
            } else if flags.contains(PortFlags::IS_INPUT) {
                Ok(())
            } else {
                Err(RecorderError::NotInputPipe(pipe.to_string()))
            }
        } else {
            Err(RecorderError::PipeNotFound(pipe.to_string()))
        }
    }
}
//---- Inputs end ----

// ---- AudioBuffers start ----
/// Hold recorded audio data buffers.  Allow access by name (useful
/// for Jack pipes) and by index starting at zero.

//---- AudioBuffers start ----
#[derive(Clone, Debug)]
pub struct AudioBuffers {
    buffers: Vec<Vec<f32>>,
    buffer_names: HashMap<String, u32>,
}

/// Public interface
impl AudioBuffers {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            // TODO: Deprecate these names.  Callers should maintain
            // the connection between names and channel indexes
            buffer_names: HashMap::new(),
            buffers: Vec::new(),
        }
    }

    pub fn add_named_buffer(&mut self, name: &str, buffer: Vec<f32>) -> Result<(), RecorderError> {
        match self.buffer_names.get_mut(name) {
            Some(_) => Err(RecorderError::DuplicateBufferName(name.into())),
            None => {
                let idx = self.buffers.len() as u32;
                _ = self.buffer_names.insert(name.into(), idx);
                self.buffers.push(buffer);
                Ok(())
            }
        }
    }

    /// Add an audio buffer without a name.  Callers are responsible
    /// for keeping track of the order of buffers
    pub fn add_buffer(&mut self, buffer: Vec<f32>) -> Result<(), RecorderError> {
        let idx = self.buffers.len();
        let name = idx.to_string();
        self.add_named_buffer(name.as_str(), buffer)
    }

    pub fn reset(&mut self) {
        for b in self.buffers.iter_mut() {
            b.truncate(0);
        }
        self.buffer_names = HashMap::new();
    }

    /// Add a sample to an audio buffer.  `channel` is the 0-based
    /// index for the channel.  Callers have to keep track of this
    pub fn add_sample(&mut self, sample: f32, channel: u32) -> Result<(), RecorderError> {
        let idx = channel as usize;
        if idx >= self.buffers.len() {
            Err(RecorderError::BadChannelIndex(channel))
        } else {
            self.buffers[idx].push(sample);
            Ok(())
        }
    }

    /// Get the names of all the buffers.
    pub fn names(&self) -> Vec<String> {
        self.buffer_names
            .keys()
            .map(|k| k.to_string())
            .collect::<Vec<String>>()
    }

    /// Get some stats:
    pub fn stats(&self) -> String {
        let mut result = "".to_string();
        for (k, v) in self.buffers.iter().enumerate() {
            let length = v.len();
            result = format!("{result}Buffer: {k}\tLength: {}\n", length);
        }
        result
    }

    /// The number of channels
    pub fn channels(&self) -> u32 {
        self.buffers.len() as u32
    }

    /// Clone an audio buffer and return it
    pub fn get_buffer(&self, channel: u32) -> Result<Vec<f32>, RecorderError> {
        if channel < self.channels() {
            let result = self.buffers[channel as usize].clone();
            Ok(result)
        } else {
            Err(RecorderError::BadChannelIndex(channel))
        }
    }

    /// Get a &mut to an audio buffer
    pub fn get_buffer_mut(&mut self, channel: u32) -> Result<&mut Vec<f32>, RecorderError> {
        match self.buffers.get_mut(channel as usize) {
            Some(b) => Ok(b),
            None => Err(RecorderError::BadChannelIndex(channel)),
        }
    }

    pub fn get_named_buffer(&self, name: &str) -> Result<Vec<f32>, RecorderError> {
        match self.buffer_names.get(name) {
            Some(c) => self.get_buffer(*c),
            None => Err(RecorderError::BadChannelName(name.to_string())),
        }
    }
}

// /// Iterators
// impl AudioBuffers {

//     pub fn iter(&self) -> impl Iterator<Item = (&String, &Vec<f32>)> {
//	self.buffers.iter()
//     }

//     pub fn iter_mut(&mut self) -> impl Iterator<Item = (&String, &mut Vec<f32>)> {
//	self.buffer_names.iter_mut()
//     }

//     pub fn keys(&self) -> impl Iterator<Item = &String> {
//	self.buffer_names.keys()
//     }

//     pub fn values(&self) -> impl Iterator<Item = &Vec<f32>> {
//	self.buffer_names.values()
//     }

//     pub fn insert(&mut self, name: String, data: Vec<f32>) {
//	self.buffer_names.insert(name, data);
//     }

//     pub fn get(&self, name: &str) -> Option<&Vec<f32>> {
//	self.buffer_names.get(name)
//     }

//     pub fn get_mut(&mut self, name: &str) -> Option<&mut Vec<f32>> {
//	self.buffer_names.get_mut(name)
//     }
// }

// ---- AudioBuffers end ----

// ---- FileManager start ----
/// Manage audio data for a session. A "session" is the lifetime of
/// the main recording thread.  Write the audio to a file.  Audio channels
/// are interleaved in the file.  For each session create two files:
///
/// 1. With a ".raw" suffix.  The interleaved audio binary data,
///
/// 2. With a ".json" suffix describing the sanple rate and the number
///    of channels.
#[allow(dead_code)]
pub struct FileManager {
    /// Maps audio channel names to their output file paths
    pub file_path: PathBuf,

    ///  Receivers for incoming audio data (moved to threads on start)
    receivers: Vec<mpsc::Receiver<f32>>,

    /// Senders for audio data (returned to caller on start)
    senders: Vec<mpsc::Sender<f32>>,

    /// Thread handle for recording thread
    handle: Option<thread::JoinHandle<Result<(), RecorderError>>>,

    /// The number of channels
    pub channels: u32,

    /// Flag to stop ths being started twice
    started: bool,
}

impl FileManager {
    /// Creates a new FileManager for the given audio channel names.
    ///
    /// # Arguments
    /// * `channels` - The number of audio channels
    /// * `file_path` The path to the file being managed
    ///
    /// # Errors
    /// Returns an error if the directory doesn't exist.
    pub fn new(channels: u32, file_path: &Path) -> Result<Self, RecorderError> {
        let mut senders = Vec::new();
        let mut receivers = Vec::new();
        for _ in 0..channels {
            let (tx, rx) = mpsc::channel::<f32>();
            receivers.push(rx);
            senders.push(tx);
        }

        let file_path: PathBuf = file_path.into();
        Ok(Self {
            file_path,
            receivers,
            senders,
            handle: None,
            channels,
            started: false,
        })
    }

    /// Starts the FileManager thread
    ///
    /// Can only be called once. Subsequent calls will fail since receivers
    /// are moved to threads.
    ///
    /// # Errors
    /// Returns an error if:
    /// - Already started (receivers/senders already moved)
    /// - Unable to create output file
    /// - Thread creation fails
    pub fn start(&mut self) -> Result<(), RecorderError> {
        if self.started {
            return Err(RecorderError::FileManager(
                "FileManager already started".to_string(),
            ));
        }

        // Take the receivers for passing to a thread
        let receivers: Vec<mpsc::Receiver<f32>> = self.receivers.drain(0..).collect();

        // Start a thread for maintaining the data
        let handle = self.spawn_writer_thread(receivers)?;
        self.handle = Some(handle);

        Ok(())
    }

    // Checks the health of all recording threads.

    /// # Returns
    /// - `Ok(true)` - Thread running normally
    /// - `Ok(false)` -  Thread stopped cleanly or not started
    pub fn check(&mut self) -> Result<bool, Vec<RecorderError>> {
        // Fast path: all threads still running
        match self.handle.as_ref() {
            Some(h) => Ok(h.is_finished()),
            None => Ok(false),
        }
    }

    /// Take ownership of the `mpsc::Sender<f32>`s for sending data
    /// here
    pub fn drain_senders(&mut self) -> Vec<mpsc::Sender<f32>> {
        self.senders.drain(0..).collect()
    }
}

/// The structure that is written beside raw data files to provide
/// metadata required to convert the raw audio into other audio
/// formats
#[derive(Serialize, Debug)]
struct Metadata {
    channels: u32,
    sample_rate: usize,
}

/// Private implementation details
#[allow(dead_code)]
impl FileManager {
    /// Maximum number of samples to buffer before forcing a write
    const MAX_BUFFER_SIZE: usize = 4096;
    const SLEEP_DURATION_MS: u64 = 1;

    /// Spawns a thread to handle writing audio data to a file.
    fn spawn_writer_thread(
        &self,
        receivers: Vec<mpsc::Receiver<f32>>,
    ) -> Result<thread::JoinHandle<Result<(), RecorderError>>, RecorderError> {
        // The files to write: A data and a metadata file
        let (audio_path, metadata_path) = Self::make_paths(&self.file_path)?;

        // The metadata that is required to convert the raw audio to other formats.
        let channels: u32 = self.channels;
        let sample_rate: usize = get_sample_rate();
        let metadata = Metadata {
            channels,
            sample_rate,
        };
        let json = serde_json::to_string_pretty(&metadata).map_err(|err| {
            RecorderError::Generic(format!(
                "Failed to convert metadata to JSON. Metadata is: {metadata:?} Error: {err}"
            ))
        })?;
        fs::write(&metadata_path, json).map_err(|err| {
            RecorderError::Generic(format!(
                "Failed to write metadata to {metadata_path:?}.  Error: {err}"
            ))
        })?;

        // The audio data file
        let mut file = match OpenOptions::new()
            .read(true)
            .write(true)
            .create(true) // Creates file if it doesn't exist
            .truncate(false) // Don't clear file contents
            .open(audio_path)
        {
            // let mut file: File = match File::create(&audio_path). {
            Ok(f) => f,
            Err(err) => panic!("{:?}: {err}", self.file_path),
        };

        Ok(thread::spawn(move || -> Result<(), RecorderError> {
            let mut c = 0;

            // Buffer a sample from each channel before writing
            let mut buffer: Vec<f32> = Vec::with_capacity(channels as usize);
            while let Ok(s) = receivers[c].recv() {
                // Does this very naive buffering effect performance?
                // Would it be better to do in 100ms (?) chunks?
                buffer.push(s);
                c += 1;
                if c == channels as usize {
                    Self::write_samples(&mut file, &buffer)?;
                    c = 0;
                    buffer.clear();
                }
            }
            Ok(())
        }))
    }

    fn make_paths(in_path: &Path) -> Result<(PathBuf, PathBuf), RecorderError> {
        let audio_path: PathBuf = in_path.with_extension("raw");
        let metadata_path: PathBuf = in_path.with_extension("json");
        Ok((audio_path, metadata_path))
    }

    /// Creates or truncates the output file.
    fn create_output_file(path: &Path) -> Result<File, RecorderError> {
        if path.exists() {
            eprintln!("Overwriting existing file: {:?}", path);
        }

        OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)
            .map_err(|err| {
                RecorderError::FileManager(format!("Cannot create file at {:?}: {}", path, err))
            })
    }

    /// Collects samples from the receiver into the buffer.
    ///
    /// Returns `true` if the channel is still connected, `false` if disconnected.
    fn collect_samples(
        receiver: &mpsc::Receiver<f32>,
        buffer: &mut Vec<f32>,
    ) -> Result<bool, RecorderError> {
        loop {
            match receiver.try_recv() {
                Ok(sample) => {
                    buffer.push(sample);
                    if buffer.len() >= Self::MAX_BUFFER_SIZE {
                        break;
                    }
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => return Ok(false),
            }
        }
        Ok(true)
    }

    /// Writes the buffer to file as raw f32 bytes.
    fn write_buffer(file: &mut File, buffer: &[f32]) -> Result<(), RecorderError> {
        // Convert f32 slice to byte slice
        let bytes = unsafe {
            std::slice::from_raw_parts(buffer.as_ptr() as *const u8, std::mem::size_of_val(buffer))
        };

        file.write_all(bytes).map_err(|err| {
            RecorderError::FileManager(format!("Failed to write audio data: {}", err))
        })
    }

    /// Write data to a file.  `file` is pen for appending and the
    /// file pointer is in the correct place.  `samples` are the data
    /// to write. `file` is left ready for more data to be written to
    /// it.  This function does no checking of the data. It just
    /// writes it straight to the file
    fn write_samples(file: &mut File, samples: &[f32]) -> Result<(), RecorderError> {
        let bytes = unsafe {
            std::slice::from_raw_parts(
                samples.as_ptr() as *const u8,
                std::mem::size_of_val(samples),
            )
        };
        match file.write_all(bytes) {
            Ok(_) => Ok(()),
            Err(err) => Err(RecorderError::FileManager(format!(
                "Failed writing samples to file: {err}"
            ))),
        }
    }

    /// Writes the sample rate to a metadata file.
    fn write_sample_rate_file(&self) -> Result<(), RecorderError> {
        todo!();
        // let sample_rate = get_sample_rate();

        // let dir = self
        //     .audio_files
        //     .values()
        //     .next()
        //     .and_then(|p| p.parent())
        //     .ok_or(RecorderError::NoAudioFiles)?;

        // let sample_rate_path = dir.join(".sample_rate");

        // fs::write(&sample_rate_path, sample_rate.to_string())
        //     .map_err(|err| RecorderError::CannotCreateFile(sample_rate_path, err.to_string()))
    }
}
// ---- FileManager end ----

/// Read audio data from a file into audio buffers
pub fn read_f32_vec_from_file(
    file_path: &PathBuf,
    channels: u32,
) -> Result<AudioBuffers, RecorderError> {
    // Read the file into a Vec<u8>
    let data: Vec<u8> =
        fs::read(file_path).map_err(|err| RecorderError::Generic(err.to_string()))?;

    // Convert Vec<u8> to Vec<f32>
    if !data.len().is_multiple_of(std::mem::size_of::<f32>()) {
        return Err(RecorderError::Generic(
            "File size is not a multiple of f32 size".to_string(),
        ));
    }
    if !data.len().is_multiple_of(channels as usize) {
        return Err(RecorderError::Generic(format!(
            "File size is not a multiple of channels: {channels}"
        )));
    }

    // Create a Vec<f32> from the Vec<u8>
    let float_vec: Vec<f32> = data
        .chunks_exact(std::mem::size_of::<f32>())
        .map(|chunk| f32::from_ne_bytes(chunk.try_into().unwrap()))
        .collect();
    // let float_count = data.len() / std::mem::size_of::<f32>();
    // let float_vec: Vec<f32> =
    //     unsafe { std::slice::from_raw_parts(data.as_ptr() as *const f32, float_count).to_vec() };

    // The channels are multiplexd together, demultiplex them
    let mut result: AudioBuffers = AudioBuffers::new();
    for _ in 0..channels {
        result.add_buffer(Vec::new())?;
    }
    let mut c = 0;
    for s in float_vec.iter() {
        result.add_sample(*s, c)?;
        c = (c + 1) % channels;
    }
    // Because all the channles have the same number of samples, when
    // the above loop ends `c` must be 0
    assert_eq!(c, 0);
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_capture_add_valid_port() {
        let port = "system:capture_1";
        let mut inputs = JackPipes::new(true);
        inputs.add(port).unwrap();
        assert!(inputs.ports().iter().any(|p| p.as_str() == port));
    }
    #[test]
    fn list_capture_add_invalid_port() {
        let port = "system_capture_1";
        let mut inputs = JackPipes::new(true);
        let test = inputs.add(port);
        assert!(test.is_err());
    }
}

#[cfg(test)]
mod read_audio_file_tests {
    /// Thank you Claude code
    /// These tests cover:
    /// - Single channel reading
    /// - Multi-channel (4) reading
    /// - Empty files
    /// - File not found errors
    /// - Invalid file sizes (not multiple of f32 size)
    /// - Incomplete frames (should panic per the assertion)
    /// - Special float values (negative, zero, MAX, MIN)
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    /// Helper function to create a temporary file with f32 data
    fn create_temp_f32_file(data: &[f32]) -> NamedTempFile {
        let mut file = NamedTempFile::new().unwrap();
        let bytes: Vec<u8> = data.iter().flat_map(|&f| f.to_ne_bytes()).collect();
        file.write_all(&bytes).unwrap();
        file.flush().unwrap();
        file
    }

    #[test]
    fn test_read_single_channel() {
        let samples = vec![1.0f32, 2.0, 3.0, 4.0];
        let temp_file = create_temp_f32_file(&samples);
        let path = temp_file.path().to_path_buf();

        let result = read_f32_vec_from_file(&path, 1).unwrap();

        assert_eq!(result.channels(), 1);

        // Verify the samples are correctly read
        let mut test_buffer = AudioBuffers::new();
        test_buffer.add_buffer(samples.clone()).unwrap();

        assert_eq!(result.stats(), test_buffer.stats());
    }

    #[test]
    fn read_four_channels() {
        // 12 samples = 3 frames of 4 channels
        let samples = vec![
            1.0f32, 2.0, 3.0, 4.0, // Frame 1
            5.0, 6.0, 7.0, 8.0, // Frame 2
            9.0, 10.0, 11.0, 12.0, // Frame 3
        ];
        let temp_file = create_temp_f32_file(&samples);
        let path = temp_file.path().to_path_buf();

        let result = read_f32_vec_from_file(&path, 4).unwrap();

        assert_eq!(result.channels(), 4);

        let mut expected = AudioBuffers::new();
        eprintln!("expected.channels() {}", expected.channels());
        expected.add_buffer(vec![1.0, 5.0, 9.0]).unwrap();
        expected.add_buffer(vec![2.0, 6.0, 10.0]).unwrap();
        expected.add_buffer(vec![3.0, 7.0, 11.0]).unwrap();
        expected.add_buffer(vec![4.0, 8.0, 12.0]).unwrap();

        assert_eq!(result.stats(), expected.stats());
    }

    #[test]
    fn test_empty_file() {
        let samples: Vec<f32> = vec![];
        let temp_file = create_temp_f32_file(&samples);
        let path = temp_file.path().to_path_buf();

        let result = read_f32_vec_from_file(&path, 2).unwrap();

        assert_eq!(result.channels(), 2);
    }

    #[test]
    fn test_file_not_found() {
        let path = PathBuf::from("/nonexistent/file.raw");

        let result = read_f32_vec_from_file(&path, 1);

        assert!(result.is_err());
        match result {
            Err(RecorderError::Generic(_)) => {}
            _ => panic!("Expected Generic error"),
        }
    }

    #[test]
    fn test_invalid_file_size() {
        // Create a file with 3 bytes (not a multiple of 4)
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(&[1u8, 2, 3]).unwrap();
        file.flush().unwrap();
        let path = file.path().to_path_buf();

        let result = read_f32_vec_from_file(&path, 1);

        assert!(result.is_err());
        match result {
            Err(RecorderError::Generic(msg)) => {
                assert!(msg.contains("not a multiple of f32 size"));
            }
            _ => panic!("Expected Generic error about file size"),
        }
    }

    #[test]
    #[should_panic]
    fn incomplete_frame_panics() {
        // 5 samples with 2 channels = incomplete last frame
        let samples = vec![1.0f32, 2.0, 3.0, 4.0, 5.0];
        let temp_file = create_temp_f32_file(&samples);
        let path = temp_file.path().to_path_buf();

        // This should panic
        read_f32_vec_from_file(&path, 2).unwrap();
    }

    #[test]
    fn test_negative_and_special_values() {
        let samples = vec![-1.0f32, 0.0, f32::MAX, f32::MIN];
        let temp_file = create_temp_f32_file(&samples);
        let path = temp_file.path().to_path_buf();

        let result = read_f32_vec_from_file(&path, 2).unwrap();

        assert_eq!(result.channels(), 2);

        let mut expected = AudioBuffers::new();
        expected.add_buffer(vec![-1.0, f32::MAX]).unwrap();
        expected.add_buffer(vec![0.0, f32::MIN]).unwrap();

        assert_eq!(result.stats(), expected.stats());
    }
}
