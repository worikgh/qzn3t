// Copyright (c) 2026 Worik Turei Stanton
// License: GPL-3.0

#[allow(dead_code)]
struct AudioBuffer {
    data: Vec<Vec<f32>>,
}

impl AudioBuffer {
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
        let audio = AudioBuffer { data: data.clone() };

        let mut iter = audio.frames();
        while let Some(frame) = iter.next_frame() {
            assert_eq!(frame.len(), data.len());
        }
    }
}
