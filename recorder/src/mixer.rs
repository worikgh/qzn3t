// Copyright (c) 2025 Worik Turei Stanton
// License: GPL-3.0

//! Mix audio signals together.  This is barely started.

use crate::io::AudioBuffers;
#[allow(dead_code)]
pub struct AudioMixer {
    clip_threshold: f32,
    use_soft_clip: bool,
}

impl AudioMixer {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            clip_threshold: 0.9, // Start limiting before actual clipping
            use_soft_clip: true,
        }
    }

    /// Mix two buffers into one.
    pub fn mix_buffers(&self, _audio_buffers: &AudioBuffers) -> Vec<f32> {
        todo!();
        // let max_len = audio_buffers.iter().fold(0, |a, b| a.max(b.1.len()));
        // let mut mixed = vec![0.0f32; max_len];

        // // Sum all buffers
        // for (_, data) in audio_buffers.iter() {
        //     for (i, sample) in data.iter().enumerate() {
        //         mixed[i] += *sample;
        //     }
        // }

        // // Apply clipping protection
        // if self.use_soft_clip {
        //     self.apply_soft_clip(&mut mixed);
        // } else {
        //     self.apply_hard_clip(&mut mixed);
        // }

        // mixed
    }

    /// Soft clipping of an audio signal using tanh
    #[allow(dead_code)]
    fn apply_soft_clip(&self, buffer: &mut [f32]) {
        for sample in buffer {
            if sample.abs() > self.clip_threshold {
                *sample = sample.tanh();
            }
        }
    }

    /// Hard clipping of an audio signal
    #[allow(dead_code)]
    fn apply_hard_clip(&self, buffer: &mut [f32]) {
        for sample in buffer {
            *sample = sample.clamp(-1.0, 1.0);
        }
    }
}
