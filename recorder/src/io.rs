// Copyright (c) 2025 Worik Turei Stanton
// License: GPL-3.0

//! Structures and code to maintain the input and output buffers and
//! processes
use crate::{app::App, errors::RecorderError, utils::get_sample_rate};
use jack::{Client, PortFlags};
use std::fs::OpenOptions;
use std::{self, collections::HashMap, fs, path::PathBuf, sync::mpsc, thread, time::Duration};

// ---- Inputs Start ----
/// Define the  Jack pipes to use
#[derive(Debug, Clone)]
pub struct JackPipes {
    names_ports: HashMap<String, String>,
}

/// Public interface
impl JackPipes {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            names_ports: HashMap::new(),
        }
    }

    /// Using the strings from the command line [`crate::structs::Args`] `-i` add input jack pipes.
    pub fn from_command_line(input_pipes: Vec<String>) -> Result<Self, RecorderError> {
        let mut result = Self::new();
        if input_pipes.is_empty() {
            return Err(RecorderError::NoInputs);
        }

        for i in input_pipes.iter() {
            let parts: Vec<&str> = i.split(':').collect();
            if parts.len() < 2 || parts.len() > 3 {
                return Err(RecorderError::InvalidPipeName(i.to_string()));
            } else if parts.len() == 2 {
                result.add(i)?;
            } else {
                let client = parts[0];
                let port = parts[1];
                let name = parts[2];
                let client_port = format!("{client}:{port}");
                result.add_name(&client_port, name)?;
            }
        }

        Ok(result)
    }

    /// Add an input to the collection with a default name.  The port
    /// must be of the type form "`client`:`port name`"
    pub fn add(&mut self, port: &str) -> Result<(), RecorderError> {
        self.add_name(port, port)
    }

    /// Add an input to the collection with a defined name.  The port
    /// must be of the type form "`client`:`port name`".
    pub fn add_name(&mut self, port: &str, name: &str) -> Result<(), RecorderError> {
        Self::validate_jack_input_pipe(port)?;
        if self.names_ports.values().any(|n| n == port) {
            Err(RecorderError::DuplicateInput(port.to_string()))
        } else if self.names_ports.iter().any(|pn| pn.0 == name) {
            Err(RecorderError::DuplicateInputName(name.to_string()))
        } else {
            // self.ports.push(port.to_string());
            self.names_ports.insert(name.to_string(), port.to_string());
            Ok(())
        }
    }

    /// Get a copy of all the ports
    pub fn ports(&self) -> Vec<String> {
        self.names_ports.values().cloned().collect()
    }

    pub fn names(&self) -> Vec<String> {
        self.names_ports.keys().map(|n| n.to_string()).collect()
    }
    /// Get a copy of the named ports
    pub fn named_ports(&self) -> HashMap<String, String> {
        self.names_ports.clone()
    }
}

/// Private interface
impl JackPipes {
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

    /// Get the names of all the buffers.
    pub fn names(&self) -> Vec<String> {
        self.buffers
            .keys()
            .map(|k| k.to_string())
            .collect::<Vec<String>>()
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

// ---- AudioBuffers end ----

// ---- FileManager start ----
/// Manage the writing of the audio buffers into files.  Each audio
/// channel to be recorded has a `mpsc::Sender<f32>` associated that
/// sends the audio bytes.  Each audio channel is saved separately to
/// a file named for the input it is from.
pub struct FileManager {
    /// Make the audio channel name to the file that holds its data
    audio_files: HashMap<String, PathBuf>,

    /// Channels to receive audio data on
    receivers: HashMap<String, mpsc::Receiver<f32>>,

    /// Senders to send audio data on.  Created and stored here in the
    /// constructor and returned to a caller with `get_senders(&self)`
    senders: HashMap<String, mpsc::Sender<f32>>,

    /// There is one thread per file to monitor.  This stores the
    /// handles
    handles: HashMap<String, thread::JoinHandle<Result<(), RecorderError>>>,
}

/// Public interface
impl FileManager {
    /// Passed in the list of names of audio channels ad the path to
    /// the directory where they will be saved
    pub fn new(names: Vec<String>, dir: &PathBuf) -> Result<Self, RecorderError> {
        // Check directory exists
        if !dir.exists() {
            return Err(RecorderError::FileManager(format!(
                "FileManager::new: Directory does not exist: {dir:?}.",
            )));
        }

        let mut result = Self {
            audio_files: HashMap::new(),
            receivers: HashMap::new(),
            senders: HashMap::new(),
            handles: HashMap::new(),
        };

        for name in names.iter() {
            // For each name create path to backing file (most not already
            // exist), and a channel for communication

            let path = dir.join(name);
            // TODO: Create an option for not overwriting files,
            // perhaps an "append" option?
            result.audio_files.insert(name.to_string(), path);
            let (tx, rx) = mpsc::channel::<f32>();
            result.receivers.insert(name.to_string(), rx);
            result.senders.insert(name.to_string(), tx);
        }
        Ok(result)
    }

    /// Start the FileManager and return the senders.  Can only be called once
    pub fn start(&mut self) -> Result<HashMap<String, mpsc::Sender<f32>>, RecorderError> {
        let mut result: HashMap<String, mpsc::Sender<f32>> = HashMap::new();
        for name in self.audio_files.keys() {
            // This stops repeated calls
            let receiver = match self.receivers.remove(name) {
                Some(r) => r,
                None => {
                    return Err(RecorderError::FileManager(format!(
                        "FileManager::new.  No receiver for {name}"
                    )));
                }
            };
            let handle = self.thread_fn(name, receiver)?;
            self.handles.insert(name.to_string(), handle);
            let sender = match self.senders.remove(name) {
                Some(s) => s,
                None => {
                    return Err(RecorderError::FileManager(format!(
                        "FileManager::start: No sender for {name}"
                    )));
                }
            };
            result.insert(name.to_string(), sender);
        }
        // Write the sample rate to a file.  Cannot be converted
        // without this
        let sample_rate = get_sample_rate();
        let p = self.audio_files.values().next();
        if p.is_none() {
            return Err(RecorderError::NoAudioFiles);
        }
        let p: PathBuf = match p.unwrap().parent() {
            Some(p) => p.into(),
            None =>
            // Audio files being placed in root.  Ok.  Not
            // sensible, but not against the law....
            {
                "/".into()
            }
        };
        assert!(p.is_dir());
        let sample_rate_path = p.join(".sample_rate");
        if let Err(err) = fs::write(&sample_rate_path, sample_rate.to_string()) {
            Err(RecorderError::CannotCreateFile(
                sample_rate_path,
                format!("{err}"),
            ))
        } else {
            Ok(result)
        }
    }

    /// Function to check that the file manager is behaving.  Returns
    /// Ok(true) if running smoothly.  Ok(false) if it has stopped
    /// properly otherwise a vector of errors
    pub fn check(&mut self) -> Result<bool, Vec<RecorderError>> {
        if self.handles.iter().all(|(_, h)| !h.is_finished()) {
            return Ok(true);
        }

        // Some handles are finished.  Store their names here.
        let mut finished: Vec<String> = Vec::new();
        {
            for (name, h) in self.handles.iter() {
                if h.is_finished() {
                    finished.push(name.to_string());
                }
            }
        }
        // Store any errors
        let mut errors: Vec<RecorderError> = Vec::new();

        for name in finished.iter() {
            let handle = self.handles.remove(name).unwrap();
            match handle.join() {
                Ok(Ok(())) => continue,
                Ok(Err(err)) => errors.push(err),
                Err(err) => errors.push(RecorderError::FileManager(format!(
                    "FileManager::check  Recording thread for {name} panicked.  Error: {err:?}"
                ))),
            };
        }

        if !self.handles.is_empty() {
            // Recorder has half stopped
            errors.push(RecorderError::FileManager(
                "FileManager::check only some internal threads have stopped".to_string(),
            ));
        }

        if errors.is_empty() {
            Ok(false)
        } else {
            Err(errors)
        }
    }
}

/// Private interface
impl FileManager {
    /// Manage one audio channel file output.  Take the audio data a
    /// byte at a time, buffer it until either there is a pause in the
    /// data or there is sufficient data (an arbitrary amount so data
    /// gets written even if it is constant) and write the buffer to a
    /// file.
    fn thread_fn(
        &self,
        name: &str,
        receiver: mpsc::Receiver<f32>,
    ) -> Result<thread::JoinHandle<Result<(), RecorderError>>, RecorderError> {
        let path = match self.audio_files.get(name) {
            Some(p) => p.clone(),
            None => {
                return Err(RecorderError::FileManager(format!(
                    "FileManager::thread_fn.  No path stored for {name}"
                )));
            }
        };

        // Copies of data the thread needs
        let name = name.to_string();

        let result = thread::spawn(move || -> Result<(), RecorderError> {
            if path.exists() {
                eprintln!("DBG recorder FileManager::thread_fn overwriting {path:?}");
            }
            let mut file = match OpenOptions::new()
                .write(true)
                .create(true) // Create if doesn't exist
                .truncate(true) // This causes overwriting
                .open(&path)
            {
                Ok(f) => f,
                Err(err) => {
                    return Err(RecorderError::FileManager(format!(
                        "FileManager::new.  Cannot create file at {path:?}  Error: {err}"
                    )));
                }
            };

            // The most f32 to receive before they must be written
            const MAX_C: usize = 4096;

            let mut buffer: Vec<f32> = Vec::new();
            let mut connected = true;
            while connected {
                buffer.clear();

                loop {
                    match receiver.try_recv() {
                        Err(mpsc::TryRecvError::Empty) => break,
                        Err(mpsc::TryRecvError::Disconnected) => {
                            connected = false;
                            break;
                        }
                        Ok(b) => {
                            buffer.push(b);
                        }
                    }
                    if buffer.len() > MAX_C {
                        break;
                    }
                }
                if buffer.is_empty() {
                    continue;
                }

                // Write the buffer.  Either when there is a pause in
                // data being sent or when MAX_C samples have been
                // received
                let bytes_written = App::write_f32_to_file(&mut file, &buffer)?;
                assert_eq!(bytes_written, buffer.len() * std::mem::size_of::<f32>());

                // Not too fast....
                thread::sleep(Duration::from_millis(1));
            }
            eprintln!("FileManager main thread ending {name}");
            Ok(())
        });
        Ok(result)
    }
}
// ---- FileManager end ----

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_capture_add_valid_port() {
        let port = "system:capture_1";
        let mut inputs = JackPipes::new();
        inputs.add(port).unwrap();
        dbg!(&inputs, port);
        assert!(inputs.ports().iter().any(|p| p.as_str() == port));
    }

    #[test]
    fn list_capture_add_invalid_port() {
        let port = "system_capture_1";
        let mut inputs = JackPipes::new();
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
        println!("port: {port}");
        let name = "Capture One";
        println!("name: {name}");
        let portv = vec![format!("{port}:{name}")];
        println!("portv: {portv:?}");
        let inputs = match JackPipes::from_command_line(portv) {
            Ok(i) => i,
            Err(err) => panic!("Panicked! {err}"),
        };
        assert_eq!(inputs.ports()[0], port);
        assert_eq!(inputs.names_ports.get(name), Some(&port));
    }
}
