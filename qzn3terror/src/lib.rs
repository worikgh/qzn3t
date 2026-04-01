// Copyright (c) 2026 Worik Turei Stanton
// License: GPL-3.0
use std::{
    error::Error,
    fmt::Display,
    io::{self},
    path::PathBuf,
};
#[derive(Debug, PartialEq)]
pub enum Qzn3tError {
    ChannelOutOfBound(usize),
    CpalError(String),
    EngineNotReady(String),
    FileBackerNotReady(String),
    FileError(String),
    InvalidAudioData,
    InvalidChannel,
    InvalidChannelCount(usize, usize),
    InvalidSessionMode,
    InvalidIndex,
    InvalidPath(PathBuf),
    JackClient(String),
    JsonError(String),
    NoAudioBuffer,
    NoDevice(String),
    NumericError(String),
    SampleIndexOutOfBound(usize),
    SendError(String),
    SymphoniaError(String),
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
impl From<jack::Error> for Qzn3tError {
    fn from(err: jack::Error) -> Self {
        Qzn3tError::JackClient(format!("Jack Error: {err}"))
    }
}
impl From<symphonia_core::errors::Error> for Qzn3tError {
    fn from(err: symphonia_core::errors::Error) -> Self {
        Qzn3tError::SymphoniaError(format!("Symphonia: {err}"))
    }
}

impl Display for Qzn3tError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Qzn3tError::FileError(err) => write!(f, "{self:?} {err}"),
            Qzn3tError::ChannelOutOfBound(c)
            | Qzn3tError::SampleIndexOutOfBound(c) => {
                write!(f, "{self:?} {c}")
            }
            Qzn3tError::InvalidChannelCount(supplied, required) => write!(
                f,
                "{self:?} Invalid channel count.  Required {required} Supplied: {supplied}"
            ),
            Qzn3tError::InvalidPath(pb) => write!(f, "{self:?} Path: {pb:?}"),
            Qzn3tError::InvalidAudioData
            | Qzn3tError::InvalidChannel
            | Qzn3tError::InvalidIndex
            | Qzn3tError::NoAudioBuffer
            | Qzn3tError::InvalidSessionMode => write!(f, "{self:?}"),

            Qzn3tError::JsonError(reason)
            | Qzn3tError::CpalError(reason)
            | Qzn3tError::EngineNotReady(reason)
            | Qzn3tError::JackClient(reason)
            | Qzn3tError::FileBackerNotReady(reason)
            | Qzn3tError::NoDevice(reason)
            | Qzn3tError::NumericError(reason)
            | Qzn3tError::SendError(reason)
            | Qzn3tError::SymphoniaError(reason) => {
                write!(f, "{self:?} {reason}")
            }
        }
    }
}

impl Error for Qzn3tError {}
