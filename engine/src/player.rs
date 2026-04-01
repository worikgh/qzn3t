// Copyright (c) 2026 Worik Turei Stanton
// License: GPL-3.0

//! Structures and code required for playing audio

use crate::stepper::{StepResult, Stepper};
use qzn3t_audio_buffer::{AudioBuffer, get_sample_rate};
use qzn3terror::Qzn3tError;
use std::{
    sync::mpsc::{self},
    time::{Duration, Instant},
};
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
    fn step(&mut self, step_ns: u128) -> Result<StepResult, Qzn3tError> {
        let mut ret = StepResult::Continue;
        if self.position == 0 {
            // Initial call.  set last to 10ms ago to kick things off
            self.last = Instant::now()
                .checked_sub(Duration::from_millis(10))
                .unwrap();
        }
        let samples_to_play =
            (self.sample_rate as u128 * 1_000_000_000_000 / step_ns) as usize;
        let samples_to_play =
            if self.position + samples_to_play < self.audio_buffer.len() {
                samples_to_play
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
