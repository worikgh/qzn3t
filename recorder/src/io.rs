// Copyright (c) 2025 Worik Turei Stanton
// License: GPL-3.0

//! Structures and code to maintain the input and output buffers and
//! processes

/// The inputs.  Audio to record. Controls the Jack inputs.
use jack::{Client, PortFlags};

use crate::errors::RecorderError;

/// The information required for a Jack pipe to record.
#[derive(Debug)]
pub struct Inputs {
    port_names: Vec<String>,
}

/// Public interface
impl Inputs {
    /// Add an input to the collection.  The port must be of the type
    /// form "<client>:<port name>"
    pub fn add_input(&mut self, name: &str) -> Result<(), RecorderError> {
        match Self::validate_jack_input_pipe(name) {
            Ok(()) => {
                if self.port_names.iter().any(|n| n == name) {
                    Err(RecorderError::DuplicatePipeName(name.to_string()))
                } else {
                    self.port_names.push(name.to_string());
                    Ok(())
                }
            }
            Err(err) => Err(err),
        }
    }

    /// Get a copy of all the input names
    pub fn names(&self) -> &Vec<String> {
        &self.port_names
    }

    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            port_names: Vec::new(),
        }
    }
}

/// Private interface
impl Inputs {
    /// An input to this programme is a Jack 32-bit audio output pipe
    fn validate_jack_input_pipe(pipe: &str) -> Result<(), RecorderError> {
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
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_capture_add_valid_port() {
        let port = "system:capture_1";
        let mut inputs = Inputs::new();
        inputs.add_input(port).unwrap();
        eprintln!("{inputs:?}");
        assert!(inputs.names().iter().find(|p| p.as_str() == port).is_some());
    }

    #[test]
    fn list_capture_add_invalid_port() {
        let port = "system_capture_1";
        let mut inputs = Inputs::new();
        eprintln!("{inputs:?}");
        match inputs.add_input(port) {
            Err(RecorderError::PipeNotFound(p)) => assert_eq!(p, port),
            Err(err) => panic!("Unknown error: {err}"),
            Ok(_) => panic!("Should not be able to add {port}"),
        };
    }
}
