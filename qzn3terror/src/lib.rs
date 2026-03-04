// Copyright (c) 2026 Worik Turei Stanton
// License: GPL-3.0
use std::{error::Error, io::{self}, path::PathBuf, fmt::Display};

#[derive(Debug)]
pub enum Qzn3tError {
    CpalError(String),
    FileError(String),
    InvalidAudioData,
    InvalidChannel,
    InvalidPath(PathBuf),
    JackClient(String),
    JsonError(String),
    NoDevice(String),
    NumericError(String),
    SendError(String),
}
impl From<io::Error> for Qzn3tError {
    fn from(error: io::Error) -> Self {
	Qzn3tError::FileError(error.to_string())
    }
}
impl From<serde_json::Error> for Qzn3tError {
    fn from(error: serde_json::Error) -> Self {
	Qzn3tError::JsonError(error.to_string())
    }
}
impl From<jack::Error> for Qzn3tError{
    fn from(err: jack::Error) -> Self {
	Qzn3tError::JackClient(format!("Jack Error: {err}"))
    }
}
impl Display for Qzn3tError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
	match self {
	    Qzn3tError::FileError(err) => write!(f, "{self:?} {err}"),
	    Qzn3tError::InvalidAudioData => write!(f, "{self:?} invalid audio data"),
	    Qzn3tError::InvalidChannel => write!(f, "{self:?} invalid channel"),
	    Qzn3tError::InvalidPath(pb) => write!(f, "{self:?} Path: {pb:?}"),
	    Qzn3tError::CpalError(reason) |
	    Qzn3tError::JsonError(reason)
	    | Qzn3tError::NoDevice(reason)
	    | Qzn3tError::JackClient(reason)
	    | Qzn3tError::NumericError(reason)
	    | Qzn3tError::SendError(reason) => {
		write!(f, "{self:?} {reason}")
	    }
	}
    }
}

impl Error for Qzn3tError {}


pub fn add(left: u64, right: u64) -> u64 {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
	let result = add(2, 2);
	assert_eq!(result, 4);
    }
}
