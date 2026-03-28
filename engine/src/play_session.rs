// Copyright (c) 2026 Worik Turei Stanton
// License: GPL-3.0

//! Structures required for playing audio, and the ancillary code to
//! create and assess them

use std::{
    sync::mpsc::{self, SendError},
    time::{Duration, Instant},
};

use qzn3t_audio_buffer::{AudioBuffer, get_sample_rate};
use qzn3terror::Qzn3tError;

use crate::stepper::{StepResult, Stepper};

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

/// Hold the data reuired to play an audio buffer one step at a time
#[derive(Debug)]
pub struct Player {
    audio_buffer: AudioBuffer,
    position: usize,
    last: Instant,
    sample_rate: u32,
    senders: Vec<mpsc::Sender<f32>>,
}

impl Player {
    pub fn new(
        audio_buffer: AudioBuffer,
        senders: Vec<mpsc::Sender<f32>>,
    ) -> Self {
        Self {
            audio_buffer,
            senders,
            last: Instant::now(),
            position: 0,
            sample_rate: get_sample_rate(),
        }
    }
}

impl Stepper for Player {
    fn step(&mut self) -> Result<StepResult, Qzn3tError> {
        let mut ret = StepResult::Continue;
        if self.position == 0 {
            // Initial call.  set last to 10ms ago to kick things off
            self.last = Instant::now()
                .checked_sub(Duration::from_millis(10))
                .unwrap();
        }
        let samples_to_play = (self.last.elapsed().as_millis()
            * self.sample_rate as u128
            / 1000) as usize;
        let samples_to_play =
            if self.position + samples_to_play < self.audio_buffer.len() {
                samples_to_play
            } else {
                ret = StepResult::Complete;
                self.audio_buffer.len() - self.position
            };

        for (c, s) in self.senders.iter().enumerate() {
            let data = self.audio_buffer.get_slice(
                c,
                self.position,
                samples_to_play,
            )?;
            for d in data {
                if let Err(err) = s.send(*d) {
                    return Err(Qzn3tError::SendError(format!("{err}")));
                }
            }
        }
        dbg!(samples_to_play);
        self.position += samples_to_play;
        Ok(ret)
    }
    fn channels(&self) -> usize {
        self.audio_buffer.channels()
    }
}
