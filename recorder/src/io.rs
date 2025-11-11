// Copyright (c) 2025 Worik Turei Stanton
// License: GPL-3.0

//! Structures and code to maintain the input and output buffers and
//! processes

/// The inputs.  Audio to record. Controls the Jack inputs.
use jack::{Client, PortFlags};

use crate::errors::RecorderError;

pub struct Inputs {
    names: Vec<String>,
}

/// Public interface
impl Inputs {
    /// Add an input to the collection
    pub fn add_input(&mut self, name: &str) -> Result<(), RecorderError> {
        match Self::validate_jack_input_pipe(name) {
            Ok(()) => {
                if self.names.iter().any(|n| n == name) {
                    Err(RecorderError::DuplicatePipeName(name.to_string()))
                } else {
                    self.names.push(name.to_string());
                    Ok(())
                }
            }
            Err(err) => Err(err),
        }
    }

    /// Get a copy of all the input names
    #[allow(dead_code)]
    pub fn names(&self) -> &Vec<String> {
        &self.names
    }

    pub fn new() -> Self {
        Self { names: Vec::new() }
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
        if !ports.iter().any(|p| p == pipe) {
            Err(RecorderError::InvalidPipeName(pipe.to_string()))
        } else if let Some(port) = client.port_by_name(pipe) {
            let flags = port.flags();
            eprintln!("DBG: flags: {flags:?}");
            if flags.contains(PortFlags::IS_OUTPUT) {
                Ok(())
            } else {
                Err(RecorderError::InvalidOutputPipe(pipe.to_string()))
            }
        } else {
            Err(RecorderError::InvalidPipeName(pipe.to_string()))
        }
    }
}
