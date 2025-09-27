// Copyright (c) 2025 Worik Turei Stanton
// License: GPL-3.0

pub struct AudioMixer {
    clip_threshold: f32,
    use_soft_clip: bool,
}

impl AudioMixer {
    pub fn new() -> Self {
        Self {
            clip_threshold: 0.9, // Start limiting before actual clipping
            use_soft_clip: true,
        }
    }

    pub fn mix_buffers(&self, l: &[f32], r: &[f32]) -> Vec<f32> {
        let max_len = if l.len() > r.len() { l.len() } else { r.len() };
        let mut mixed = vec![0.0f32; max_len];

        // Sum all buffers
        for (i, sample) in l.iter().enumerate() {
            mixed[i] = *sample;
        }
        for (i, sample) in r.iter().enumerate() {
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

    fn apply_soft_clip(&self, buffer: &mut [f32]) {
        for sample in buffer {
            if sample.abs() > self.clip_threshold {
                *sample = sample.tanh(); // Soft clipping using tanh
            }
        }
    }

    fn apply_hard_clip(&self, buffer: &mut [f32]) {
        for sample in buffer {
            *sample = sample.clamp(-1.0, 1.0);
        }
    }
}
