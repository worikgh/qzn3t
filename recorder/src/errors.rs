// Copyright (c) 2025 Worik Turei Stanton
// License: GPL-3.0

//! Errors for recorder crate

/// The errors for handling inputs
use std::{error::Error, fmt};

use crate::structs::Command;

#[derive(Debug, PartialEq, Eq)]
pub enum RecorderError {
    // Command sent from UI to recorder is invalid
    BadCommand(Command),

    // If a client cannot be created: Client/error
    CannotCreateClient(String, String),

    // An error when deactivating a cleint
    DeactivateClientFailed(String),

    // A dupliacte ouput buffer.  The names must be unique
    DuplicateBufferName(String),

    // An input port was defined twice
    DuplicateInput(String),

    // An input port was defined twice with the same name
    DuplicateInputName(String),

    // For errors from other systems
    Generic(String),

    // No inputs supplied
    NoInputs,

    // A pipe to act as input to recorder is not an output pipe
    NotOutputPipe(String),

    // The pipe name was invalid
    InvalidPipeName(String),

    // Cannot find the pipe
    PipeNotFound(String),
}
impl fmt::Display for RecorderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RecorderError::NoInputs => write!(f, "There are no input pipes supplied"),
            RecorderError::DuplicateBufferName(name)
            | RecorderError::DuplicateInput(name)
            | RecorderError::DuplicateInputName(name)
            | RecorderError::InvalidPipeName(name)
            | RecorderError::PipeNotFound(name)
            | RecorderError::NotOutputPipe(name) => {
                write!(f, "{self:?} Name {name}")
            }
            RecorderError::DeactivateClientFailed(error) => write!(f, "{self:?}: Error: {error}"),
            RecorderError::CannotCreateClient(name, reason) => {
                write!(f, "{self:?}: Name: {name}. Reason: {reason}")
            }

            RecorderError::Generic(err) => write!(f, "{self:?}: {err}"),

            RecorderError::BadCommand(command) => write!(f, "{self:?}: Command: {command}"),
        }
    }
}
impl Error for RecorderError {}
impl From<Box<dyn Error>> for RecorderError {
    fn from(err: Box<dyn Error>) -> Self {
        eprintln!("RecorderError: `From<Box<dyn Error>>`  err: {err}");
        RecorderError::Generic(format!("{err}"))
    }
}
