// Copyright (c) 2025 Worik Turei Stanton
// License: GPL-3.0

//! Mix audio signals together.  This is barely started.
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

    /// Mix two buffers into one
    pub fn mix_buffers(&self, left_in: &[f32], right_in: &[f32]) -> Vec<f32> {
        let max_len = if left_in.len() > right_in.len() {
            left_in.len()
        } else {
            right_in.len()
        };
        let mut mixed = vec![0.0f32; max_len];

        // Sum all buffers
        for (i, sample) in left_in.iter().enumerate() {
            mixed[i] = *sample;
        }
        for (i, sample) in right_in.iter().enumerate() {
            mixed[i] += *sample;
        }

        // Apply clipping protection
        if self.use_soft_clip {
            self.apply_soft_clip(&mut mixed);
        } else {
            self.apply_hard_clip(&mut mixed);
        }

        mixed
    }

    /// Soft clipping of an audio signal using tanh
    fn apply_soft_clip(&self, buffer: &mut [f32]) {
        for sample in buffer {
            if sample.abs() > self.clip_threshold {
                *sample = sample.tanh();
            }
        }
    }

    /// Hard clipping of an audio signal
    fn apply_hard_clip(&self, buffer: &mut [f32]) {
        for sample in buffer {
            *sample = sample.clamp(-1.0, 1.0);
        }
    }
}
