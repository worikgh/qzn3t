// Copyright (c) 2025 Worik Turei Stanton
// License: GPL-3.0

//! Errors for recorder crate

/// The errors for handling inputs
use std::{error::Error, fmt};

use crate::structs::Command;

#[derive(Debug, PartialEq, Eq)]
pub enum RecorderError {
    BadCommand(Command),
    CannotCreateClient(String, String),
    DuplicatePipeName(String),
    Generic(String), // For errors from other systems
    InvalidOutputPipe(String),
    InvalidPipeName(String),
}
impl fmt::Display for RecorderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RecorderError::DuplicatePipeName(name)
            | RecorderError::InvalidPipeName(name)
            | RecorderError::InvalidOutputPipe(name) => {
                write!(f, "{self:?} Name {name}")
            }
            RecorderError::CannotCreateClient(name, reason) => {
                write!(f, "{self:?}: Name: {name}. Reason: {reason}")
            }
            RecorderError::Generic(err) => {
                write!(f, "{self:?}: {err}")
            }
            RecorderError::BadCommand(command) => {
                write!(f, "{self:?}: Command: {command}")
            }
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
