// Copyright (c) 2025 Worik Turei Stanton
// License: GPL-3.0

//! Errors for recorder crate

/// The errors for handling inputs
use std::{error::Error, fmt};

#[derive(Debug)]
pub enum RecorderError {
    DuplicatePipeName(String),
    InvalidPipeName(String),
    InvalidOutputPipe(String),
    CannotCreateClient(String, String),
}
impl fmt::Display for RecorderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RecorderError::DuplicatePipeName(name) => write!(f, "The name {name} is a duplicate"),
            RecorderError::InvalidPipeName(name) => write!(f, "The name {name} is not a Jack pipe"),
            RecorderError::InvalidOutputPipe(name) => {
                write!(f, "The pipe named name {name} is not a Jack input pipe")
            }
            RecorderError::CannotCreateClient(name, reason) => {
                write!(f, "Cannot create client {name}. Reason: {reason}")
            }
        }
    }
}
impl Error for RecorderError {}
