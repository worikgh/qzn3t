// Copyright (c) 2025 Worik Turei Stanton
// License: GPL-3.0

//! Errors for recorder crate

/// The errors for handling inputs
use std::{error::Error, fmt, path::PathBuf};

use crate::structs::Command;

#[derive(Debug, PartialEq, Eq)]
pub enum RecorderError {
    // An invalid channel index to AudioBuffers
    BadChannelIndex(u32),

    // An invalid channel name to AudioBuffers
    BadChannelName(String),

    // Command sent from UI to recorder is invalid
    BadCommand(Command),

    // File IO error for BufferBackingFile.  Contents is the
    // underlying error message from the std library
    BufferBackingIO(String),

    // If a client cannot be created: Client/error
    CannotCreateClient(String, String),

    // If files cannot be written to output directory.  Arguments are
    // the path that could not be written and the stringified error
    CannotCreateFile(PathBuf, String),

    // An error when deactivating a cleint
    DeactivateClientFailed(String),

    // A dupliacte ouput buffer.  The names must be unique
    DuplicateBufferName(String),

    // An input port was defined twice
    DuplicateInput(String),

    // An output port was defined twice
    DuplicateOutput(String),

    // An input port was defined twice with the same name
    DuplicateInputName(String),

    // Errors for the FileManager
    FileManager(String),

    // For errors from other systems
    Generic(String),

    // The main loop timing failed
    MainLoopTiming(String),

    // No inputs supplied
    NoInputs,

    // No audio files
    NoAudioFiles,

    // A jack pipe to act as input to recorder is not an output channel
    NotOutputPipe(String),

    // A jack pipe to act as output from recorder is not an input channel
    NotInputPipe(String),

    // The pipe name was invalid
    InvalidPipeName(String),

    // Cannot find the pipe
    PipeNotFound(String),

    // Unimplemented methods.  Pass name as argument
    Unimplemented(String),
}
impl fmt::Display for RecorderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RecorderError::NoInputs | RecorderError::NoAudioFiles => write!(f, "{self:?}"),

            RecorderError::DuplicateBufferName(name)
            | RecorderError::DuplicateInput(name)
            | RecorderError::Unimplemented(name)
            | RecorderError::DuplicateOutput(name)
            | RecorderError::DuplicateInputName(name)
            | RecorderError::FileManager(name)
            | RecorderError::InvalidPipeName(name)
            | RecorderError::PipeNotFound(name)
            | RecorderError::NotOutputPipe(name) => write!(f, "{self:?} Name {name}"),
            RecorderError::NotInputPipe(name) => write!(f, "{self:?} Name {name}"),

            RecorderError::DeactivateClientFailed(error)
            | RecorderError::BufferBackingIO(error) => write!(f, "{self:?}: Error: {error}"),

            RecorderError::CannotCreateClient(name, reason) => {
                write!(f, "{self:?}: Name: {name}. Reason: {reason}")
            }
            RecorderError::Generic(err) => write!(f, "{self:?}: {err}"),
            RecorderError::CannotCreateFile(path, error) => {
                write!(f, "{self:?}: Path: {path:?} Error: {error}")
            }
            RecorderError::BadCommand(command) => write!(f, "{self:?}: Command: {command}"),
            RecorderError::MainLoopTiming(reason) => write!(f, "{self:?}: Reason: {reason}"),
            RecorderError::BadChannelIndex(idx) => write!(f, "{self:?}: Index: {idx}"),
            RecorderError::BadChannelName(name) => write!(f, "{self:?}: Name: {name}"),
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
