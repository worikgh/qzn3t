// Copyright (c) 2026 Worik Turei Stanton
// License: GPL-3.0

use std::{error::Error, fmt::Display};
#[derive(Debug)]
enum Qzn3tError {
    InvalidAudioData,
    InvalidChannel,
}
impl Display for Qzn3tError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Qzn3tError::InvalidAudioData => write!(f, "{self:?} invalid audio data"),
            Qzn3tError::InvalidChannel => write!(f, "{self:?} invalid channel"),
        }
    }
}

impl Error for Qzn3tError {}

#[allow(dead_code)]
struct AudioBuffer {
    data: Vec<Vec<f32>>,
    /// Require this so there can be an empty buffer.  `usize` not
    /// `u32` as it is == `data.len()` if `data` is not empty
    channels: usize,
}

impl AudioBuffer {
    /// Constructor from data
    #[allow(dead_code)]
    pub fn new(data: Vec<Vec<f32>>) -> Result<Self, Qzn3tError> {
        let channels = data.len();
        let data = data.iter().map(|d| d.to_vec()).collect::<Vec<Vec<f32>>>();
        let this = Self { channels, data };
        if this.valid() {
            Ok(this)
        } else {
            Err(Qzn3tError::InvalidAudioData)
        }
    }

    #[allow(dead_code)]
    pub fn frames(&self) -> FrameIterator<'_> {
        let num_channels = self.data.len();
        let num_frames = self.data.first().map_or(0, |v| v.len());

        FrameIterator {
            data: &self.data,
            index: 0,
            num_frames,
            buffer: vec![0.0; num_channels],
        }
    }

    /// Check that all channels have the same number of samples and
    /// the `channels` field is correct
    #[allow(dead_code)]
    pub fn valid(&self) -> bool {
        if self.data.is_empty() {
            self.channels > 0 // Only zero channels is invalid
        } else {
            let s = self.data[0].len();
            self.data.len() == self.channels && self.data.iter().all(|d| d.len() == s)
        }
    }
}

#[allow(dead_code)]
pub struct FrameIterator<'a> {
    data: &'a [Vec<f32>],
    index: usize,
    num_frames: usize,
    buffer: Vec<f32>,
}

impl<'a> FrameIterator<'a> {
    #[allow(dead_code)]
    pub fn next_frame(&mut self) -> Option<&[f32]> {
        if self.index >= self.num_frames {
            return None;
        }

        for (ch, channel_data) in self.data.iter().enumerate() {
            self.buffer[ch] = channel_data[self.index];
        }

        self.index += 1;
        Some(&self.buffer)
    }
}

// Usage:

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usage() {
        let data = vec![vec![1.0, 2.0, 3.0], vec![4.0, 5.0, 6.0]];
        let audio = AudioBuffer {
            data: data.clone(),
            channels: 2,
        };

        let mut iter = audio.frames();
        while let Some(frame) = iter.next_frame() {
            assert_eq!(frame.len(), data.len());
        }
    }

    #[test]
    fn valid() {
        let data = vec![vec![1.0, 2.0, 3.0], vec![4.0, 5.0, 6.0]];
        let audio = AudioBuffer {
            data: data.clone(),
            channels: 2,
        };
        assert!(audio.valid());
    }

    #[test]
    fn invalid_channels() {
        let data = vec![vec![1.0, 2.0, 3.0], vec![4.0, 5.0, 6.0]];
        let audio = AudioBuffer {
            data: data.clone(),
            channels: 3,
        };
        assert!(!audio.valid());
    }

    #[test]
    fn invalid() {
        let data = vec![vec![1.0, 2.0], vec![4.0, 5.0, 6.0]];
        let audio = AudioBuffer {
            data: data.clone(),
            channels: 2,
        };
        let test = audio.valid();
        assert!(!test);
    }

    #[test]
    fn constructor_valid() {
        let data = vec![vec![1.0, 2.0, 3.0], vec![4.0, 5.0, 6.0]];
        let audio = AudioBuffer::new(data);
        assert!(audio.is_ok());
        assert!(audio.unwrap().valid());
    }
    #[test]
    fn constructor_invalid() {
        let data = vec![vec![1.0, 2.0, 3.0], vec![4.0, 5.0, 6.0, 7.0]];
        let audio = AudioBuffer::new(data);
        assert!(matches!(audio, Err(Qzn3tError::InvalidAudioData)));
    }
}
