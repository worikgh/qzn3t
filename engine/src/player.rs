// Copyright (c) 2026 Worik Turei Stanton
// License: GPL-3.0

//! Structures and code required for playing audio

use crate::stepper::{StepResult, Stepper};
use qzn3t_audio_buffer::AudioBuffer;
use qzn3terror::Qzn3tError;
use std::sync::mpsc::{self};
/// Hold the data reuired to play an audio buffer one step at a time
#[derive(Debug)]
pub struct Player {
    audio_buffer: AudioBuffer,
    position: usize,
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
            position: 0,
        }
    }
}

impl Stepper for Player {
    fn step(&mut self, sample_cnt: usize) -> Result<StepResult, Qzn3tError> {
        let mut ret = StepResult::Continue;
        let samples_to_play =
            if self.position + sample_cnt < self.audio_buffer.len() {
                sample_cnt
            } else {
                ret = StepResult::Complete;
                (self.audio_buffer.len() - 1) - self.position
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
        self.position += samples_to_play;
        Ok(ret)
    }
    fn channels(&self) -> usize {
        self.audio_buffer.channels()
    }
}
