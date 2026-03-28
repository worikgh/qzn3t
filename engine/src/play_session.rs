// Copyright (c) 2026 Worik Turei Stanton
// License: GPL-3.0

//! Structures required for playing audio, and the ancillary code to
//! create and assess them

use std::{
    sync::mpsc::{self, SendError},
    time::Duration,
};

use qzn3t_audio_buffer::AudioBuffer;

/// Everything needed to be passed into a thread playing audio
#[derive(Debug)]
#[allow(unused)]
pub struct PlaySession {
    pub audio_buffer: AudioBuffer,
    pub senders: Vec<mpsc::Sender<f32>>,
}

impl PlaySession {
    pub fn new(
        audio_buffer: AudioBuffer,
        senders: Vec<mpsc::Sender<f32>>,
    ) -> Self {
        Self {
            senders,
            audio_buffer,
        }
    }

    /// Check that this object is ready for use
    pub fn is_valid(&self) -> bool {
        !self.senders.is_empty()
            && !self.audio_buffer.is_empty()
            && self.senders.len() == self.audio_buffer.channels()
    }
}

/// The structure to carry the status of a play session.
#[derive(Debug)]
#[allow(unused)]
pub struct PlaySessionResult {
    pub status: PlaySessionStatus,
    pub elapsed: Duration,
}

/// The result of the play session. TODO: Am I sure I do not want to
/// return a `Result<PlaySessionResult, Qzn3tError>` from the
/// PlaySession thread?
#[derive(Debug)]
pub enum PlaySessionStatus {
    /// Normal session, played audio buffer to completion
    Finished,

    /// The first argument is the sample rate, the second samples per
    /// loop
    SampleRateNotMultipleOfSamplesPerLoop(u32, usize),

    /// Sending data from the inner loop, over a `mpsc::Sender<f32>`
    /// failed
    SendFailed(SendError<f32>),
}
