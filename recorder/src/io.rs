// Copyright (c) 2025 Worik Turei Stanton
// License: GPL-3.0

//! Structures and code to maintain the input and output buffers and
//! processes

use std::collections::HashMap;

/// The inputs.  Audio to record. Controls the Jack inputs.
use jack::{Client, PortFlags};

use crate::errors::RecorderError;

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

/// Recorded data.  Named buffers.
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
