// Copyright (c) 2026 Worik Turei Stanton
// License: GPL-3.0

use file_backer::{FileBacker, InitialiseMode};
use qzn3terror::Qzn3tError;
#[allow(unused_imports)]
use std::fs;
#[allow(unused_imports)]
use std::io::Write;
use std::{path::Path, time::Instant};
use uuid::{Context, Timestamp, Uuid};

pub mod file_backer;

// Stubs for now as this will be moved to another crate when I get to
// implementing the actual audio software

/// Ask the host for the sample rate setting.
pub fn get_sample_rate() -> u32 {
    48_000
}

/// Ask the host for the frame size
pub fn get_frame_sz() -> usize {
    1024
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct AudioBuffer {
    created: Instant,

    sample_rate: usize,

    data: Vec<Vec<f32>>,

    /// Require this so there can be an empty buffer.  `usize` not
    /// `u32` as it is == `data.len()` if `data` is not empty
    channels: usize,

    file_backer: Option<FileBacker>,

    /// Identifier
    id: Uuid,
}

/// The ID is unique so is not considered in the equality test
impl PartialEq for AudioBuffer {
    fn eq(&self, other: &Self) -> bool {
        self.data == other.data
            && self.channels == other.channels
            && self.file_backer == other.file_backer
    }
}
impl AudioBuffer {
    /// Utility to make a new ID
    fn id() -> Uuid {
        Uuid::new_v6(Timestamp::now(Context::new(0)), &[1, 2, 3, 4, 5, 6])
    }

    pub fn channels(&self) -> usize {
        self.channels
    }

    #[allow(dead_code)]
    /// Constructor for a buffer with `n_channels` audio channels
    pub fn new(n_channels: usize) -> Result<Self, Qzn3tError> {
        let id = Self::id();
        let file_backer = None;
        let data = vec![vec![]; n_channels];
        let this = Self {
            channels: n_channels,
            data,
            id,
            file_backer,
            created: Instant::now(),
            sample_rate: 48_000,
        };

        Ok(this)
    }

    /// Constructor from data.
    pub fn new_data(data: Vec<Vec<f32>>) -> Result<Self, Qzn3tError> {
        let id = Self::id();
        let file_backer = None;
        let channels = data.len();
        let this = Self {
            sample_rate: 48_000,
            channels,
            data,
            id,
            file_backer,
            created: Instant::now(),
        };
        if this.valid() {
            Ok(this)
        } else {
            Err(Qzn3tError::InvalidAudioData)
        }
    }

    #[allow(dead_code)]
    /// Create an `AudioBuffer` from data in a file
    pub fn from_file(path: &Path) -> Result<Self, Qzn3tError> {
        let init_data = FileBacker::get_data_from_file(path)?;
        let mut this = AudioBuffer::new_data(init_data)?;
        this.restore_file_backing(path)?;
        Ok(this)
    }

    #[allow(dead_code)]
    /// Attach a new `FileBacker` object to buffer and update it with
    /// buffer contents deleting any data previously in the underlying
    /// disc data
    pub fn add_file_backing(&mut self, path: &Path) -> Result<(), Qzn3tError> {
        let mut fb = FileBacker::new(path);
        fb.initialise(self.channels(), InitialiseMode::Truncate)?;
        for (i, data) in self.data.iter().enumerate() {
            if !data.is_empty() {
                fb.send(data, i)?;
            }
        }
        self.file_backer = Some(fb);
        Ok(())
    }

    /// Attach a `FileBacker` object to the buffer.  Do not change or
    /// edit the underlying disc data
    pub fn restore_file_backing(
        &mut self,
        path: &Path,
    ) -> Result<(), Qzn3tError> {
        let mut fb = FileBacker::new(path);
        fb.initialise(self.channels(), InitialiseMode::NoTruncate)?;
        self.file_backer = Some(fb);
        Ok(())
    }

    /// Reading data
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

    /// Read a channel into a vector
    pub fn get_channel(&self, channel: usize) -> Result<Vec<f32>, Qzn3tError> {
        if channel > self.channels() {
            return Err(Qzn3tError::InvalidChannel);
        }
        let fi = self.frames();
        let mut ret = vec![];
        for f in fi {
            ret.push(f[channel]);
        }
        Ok(ret)
    }
    /// Writing data.  A channel at a time
    #[allow(dead_code)]
    pub fn add_samples(
        &mut self,
        channel: usize,
        data: &[f32],
    ) -> Result<(), Qzn3tError> {
        if channel < self.channels {
            self.data[channel].extend(data);
            if let Some(fb) = self.file_backer.as_ref() {
                fb.send(data, channel)?;
            }
            Ok(())
        } else {
            Err(Qzn3tError::InvalidChannel)
        }
    }

    /// Getting a sample one at a time: TODO Replace this with an iterator
    pub fn get_sample_play(
        &self,
        channel: usize,
        idx: usize,
    ) -> Result<f32, Qzn3tError> {
        if channel > self.channels {
            Err(Qzn3tError::InvalidChannel)
        } else if idx > self.len() {
            Err(Qzn3tError::InvalidIndex)
        } else {
            Ok(self.data[channel][idx])
        }
    }
    /// Check that all channels have the same number of samples and
    /// the `channels` field is correct
    #[allow(dead_code)]
    pub fn valid(&self) -> bool {
        assert_eq!(self.id.get_version_num(), 6);
        if self.data.is_empty() {
            true
        } else {
            let s = self.data[0].len();
            self.data.len() == self.channels
                && self.data.iter().all(|d| d.len() == s)
        }
    }

    pub fn is_empty(&self) -> bool {
        if self.data.is_empty() {
            true
        } else {
            self.data[0].is_empty()
        }
    }

    pub fn len(&self) -> usize {
        if self.data.is_empty() {
            0
        } else {
            self.data[0].len()
        }
    }

    /// Convert the buffer to a format that can be written to a binary
    /// file
    #[allow(dead_code)]
    fn as_bytes(&self) -> Vec<u8> {
        self.data
            .iter()
            .flatten()
            .flat_map(|&f| f.to_ne_bytes())
            .collect()
    }

    #[allow(dead_code)]
    /// Convert binary data into format for `data` element.
    fn from_bytes(
        bytes: &[u8],
        channels: usize,
    ) -> Result<Vec<Vec<f32>>, Qzn3tError> {
        let f32sz = std::mem::size_of::<f32>();
        if !bytes.len().is_multiple_of(f32sz) {
            return Err(Qzn3tError::NumericError(
                "Bytes array not divisible by f32 size".to_string(),
            ));
        }
        let float_vec: Vec<f32> = bytes
            .chunks_exact(f32sz)
            .map(|chunk| {
                chunk
                    .try_into()
                    .map(f32::from_ne_bytes)
                    .map_err(|err| Qzn3tError::NumericError(format!("{err}")))
            })
            .collect::<Result<Vec<f32>, Qzn3tError>>()?;
        if !float_vec.len().is_multiple_of(channels) {
            return Err(Qzn3tError::NumericError(
                "F32 array not divisible by channels: {channels}".to_string(),
            ));
        }
        let mut result: Vec<Vec<f32>> = vec![];
        for _ in 0..channels {
            result.push(vec![]);
        }
        for i in 0..float_vec.len() {
            result[i % channels].push(float_vec[i]);
        }
        Ok(result)
    }
}

//-------------------
/// Some help from Claude making an Iterator...
/// Iterate over the data in an `AudioBuffer` one frame at a time.
///
/// A frame is one sample per channel at the same time index:
/// `[ch0[i], ch1[i], ..., chN[i]]`.
///
/// ## Allocation strategy
///
/// `FrameIterator` allocates a single frame-sized buffer (`Vec<f32>` of
/// length `channels`) at construction and reuses it for every frame.
///
/// ### Allocation-free path — `next_frame`
///
/// [`next_frame`] fills the internal buffer in place and returns a
/// reference to it.  No heap allocation occurs during iteration.  The
/// catch is that the slice is only valid until the *next* call to
/// `next_frame` (or until the iterator is dropped), so you must use or
/// copy it before advancing.
///
/// ```rust,ignore
/// let mut iter = audio.frames();
/// while let Some(frame) = iter.next_frame() {
///     process(frame);   // frame is &[f32], valid here
/// }
/// ```
///
/// ### Standard-iterator path — `Iterator::next`
///
/// [`FrameIterator`] also implements [`std::iter::Iterator`] (with
/// `Item = Vec<f32>`) so it works with `for` loops, `map`, `collect`,
/// etc.  Each call to `next()` fills the internal buffer then **clones**
/// it into an owned `Vec<f32>` — one allocation per frame.  Use this
/// when you need iterator adaptors and the allocation cost is acceptable.
///
/// ## Why not `Iterator<Item = &[f32]>`?
///
/// `std::iter::Iterator` requires items to be independently owned; the
/// returned reference cannot borrow from the iterator's own buffer.  This
/// is the *streaming iterator* (lending iterator) problem.  There is no
/// solution in stable `std` without per-frame allocation.
#[derive(Debug)]
pub struct FrameIterator<'a> {
    data: &'a [Vec<f32>],
    /// Pre-allocated once at construction; filled in place each frame.
    buffer: Vec<f32>,
    index: usize,
    num_frames: usize,
}

impl<'a> FrameIterator<'a> {
    /// Advance to the next frame, filling the internal buffer in-place.
    ///
    /// Returns `None` when the iterator is exhausted.  The returned slice
    /// borrows from the iterator's internal buffer and is only valid until
    /// the next call to `next_frame` (or until `self` is dropped).
    ///
    /// No heap allocation occurs during iteration.
    pub fn next_frame(&mut self) -> Option<&[f32]> {
        if self.index >= self.num_frames {
            return None;
        }
        for (ch, ch_data) in self.data.iter().enumerate() {
            self.buffer[ch] = ch_data[self.index];
        }
        self.index += 1;
        Some(&self.buffer)
    }
}

/// This has one allocation per call: the internal buffer is cloned
/// into an owned `Vec<f32>`.  Use [`FrameIterator::next_frame`]
/// instead if you want allocation-free iteration.
impl<'a> Iterator for FrameIterator<'a> {
    type Item = Vec<f32>;

    fn next(&mut self) -> Option<Self::Item> {
        self.next_frame().map(<[f32]>::to_vec)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.num_frames - self.index;
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for FrameIterator<'_> {}

/// Messages from the AudioBuffer -> FileBacker
pub struct AudioMsg {
    samples: Vec<f32>,
    channel: usize,
}

#[cfg(test)]
mod tests {
    use std::{
        env::temp_dir,
        fs::{File, OpenOptions},
        io::{Read, Seek, SeekFrom},
        path::PathBuf,
        thread,
        time::Duration,
    };

    use crate::file_backer::Metadata;

    use super::*;

    #[test]
    fn audio_buffer_from_bytes_valid() {
        let bytes: Vec<u8> = vec![0; 8];
        let test = AudioBuffer::from_bytes(&bytes, 2).unwrap();
        assert_eq!(test, vec![vec![0.0f32], vec![0.0f32]]);
    }

    #[test]
    fn audio_buffer_from_bytes_invalid() {
        let bytes: Vec<u8> = vec![0; 9];
        let test = AudioBuffer::from_bytes(&bytes, 2);
        assert!(test.is_err());
        let test = test.unwrap_err();
        let s = format!("{test}");
        assert!(!s.is_empty());

        let bytes: Vec<u8> = vec![0; 12];
        let test = AudioBuffer::from_bytes(&bytes, 2);
        let test = test.unwrap_err();
        let s = format!("{test}");
        assert!(!s.is_empty());

        let bytes: Vec<u8> = vec![0; 11];
        let test = AudioBuffer::from_bytes(&bytes, 3);
        assert!(test.is_err());
        let test = test.unwrap_err();
        let s = format!("{test}");
        assert!(!s.is_empty());
    }

    #[test]
    fn audio_buffer_file_backing_bad_path() {
        // Some goot data
        let data = vec![vec![1.0, 2.0, 3.0], vec![4.0, 5.0, 6.0]];
        let mut audio = AudioBuffer::new_data(data.clone()).unwrap();
        // A bad path
        let path: PathBuf = "/dev/null".into();
        let path = path.join("audio_buffer_file_backing_bad_path");
        assert!(audio.add_file_backing(&path).is_err());
    }

    #[test]
    fn audio_buffer_as_bytes_empty() {
        let audio = AudioBuffer {
            sample_rate: 48_000,
            data: vec![],
            channels: 1,
            id: AudioBuffer::id(),
            file_backer: None,
            created: Instant::now(),
        };
        assert!(audio.as_bytes().is_empty());
    }

    #[test]
    fn audio_buffer_as_bytes_single_channel() {
        let audio =
            AudioBuffer::new_data(vec![vec![1.0f32, 2.0f32, 3.0f32]]).unwrap();
        let bytes = audio.as_bytes();

        // Should be 3 samples * 4 bytes per f32
        assert_eq!(bytes.len(), 12);

        // Verify the bytes can be read back as f32s
        let floats: Vec<f32> = bytes
            .chunks_exact(std::mem::size_of::<f32>())
            .map(|chunk| f32::from_ne_bytes(chunk.try_into().unwrap()))
            .collect();
        assert_eq!(floats, vec![1.0f32, 2.0f32, 3.0f32]);
    }

    #[test]
    fn audio_buffer_as_bytes_multi_channel() {
        let audio_buffer = AudioBuffer::new_data(vec![
            vec![1.0f32, 2.0f32],
            vec![3.0f32, 4.0f32],
        ])
        .unwrap();

        let bytes = audio_buffer.as_bytes();

        // Should be 4 samples * 4 bytes per f32
        assert_eq!(bytes.len(), 16);
        // Verify flattened order
        let floats: Vec<f32> = bytes
            .chunks_exact(std::mem::size_of::<f32>())
            .map(|chunk| {
                f32::from_ne_bytes([chunk[0], chunk[1], chunk[2], chunk[3]])
            })
            .collect();
        assert_eq!(floats, vec![1.0f32, 2.0f32, 3.0f32, 4.0f32]);
    }

    #[test]
    fn audio_buffer_usage() {
        let data = vec![vec![1.0, 2.0, 3.0], vec![4.0, 5.0, 6.0]];
        let audio = AudioBuffer {
            sample_rate: 48_000,
            file_backer: None,
            data: data.clone(),
            id: AudioBuffer::id(),
            channels: 2,
            created: Instant::now(),
        };

        let mut iter = audio.frames();
        while let Some(frame) = iter.next_frame() {
            assert_eq!(frame.len(), data.len());
        }
    }

    #[test]
    fn audio_buffer_valid() {
        let data = vec![vec![1.0, 2.0, 3.0], vec![4.0, 5.0, 6.0]];
        let audio = AudioBuffer {
            sample_rate: 48_000,
            file_backer: None,
            data: data.clone(),
            channels: 2,
            id: AudioBuffer::id(),
            created: Instant::now(),
        };
        assert!(audio.valid());
    }

    #[test]
    fn audio_buffer_invalid_channels() {
        let data = vec![vec![1.0, 2.0, 3.0], vec![4.0, 5.0, 6.0]];
        let audio = AudioBuffer {
            sample_rate: 48_000,
            file_backer: None,
            data: data.clone(),
            channels: 3,
            id: AudioBuffer::id(),
            created: Instant::now(),
        };
        assert!(!audio.valid());
    }

    #[test]
    fn audio_buffer_invalid() {
        let data = vec![vec![1.0, 2.0], vec![4.0, 5.0, 6.0]];
        let audio = AudioBuffer {
            sample_rate: 48_000,
            file_backer: None,
            data: data.clone(),
            channels: 2,
            id: AudioBuffer::id(),
            created: Instant::now(),
        };
        let test = audio.valid();
        assert!(!test);
    }

    #[test]
    fn audio_buffer_constructor_valid() {
        let data = vec![vec![1.0, 2.0, 3.0], vec![4.0, 5.0, 6.0]];
        let audio = AudioBuffer::new_data(data.clone());
        assert!(audio.is_ok());
        assert_eq!(audio.as_ref().unwrap().channels(), data.len());
        assert!(audio.unwrap().valid());
    }

    #[test]
    fn audio_buffer_constructor_invalid() {
        let data = vec![vec![1.0, 2.0, 3.0], vec![4.0, 5.0, 6.0, 7.0]];
        let audio = AudioBuffer::new_data(data);
        assert!(matches!(audio, Err(Qzn3tError::InvalidAudioData)));
    }

    #[test]
    fn audio_buffer_add_samples() {
        let data = vec![vec![1.0, 2.0, 3.0], vec![4.0, 5.0, 6.0]];
        let mut audio = AudioBuffer::new_data(data).unwrap();
        audio.add_samples(0, &[3.5, 3.6]).unwrap();
        assert!(!audio.valid());
        audio.add_samples(1, &[6.5, 6.6]).unwrap();
        assert!(audio.valid());
    }

    #[test]
    fn audio_buffer_add_samples_bad_channel() {
        let data = vec![vec![1.0, 2.0, 3.0], vec![4.0, 5.0, 6.0]];
        let mut audio = AudioBuffer::new_data(data).unwrap();
        let test = audio.add_samples(2, &[3.5, 3.6]);
        assert!(matches!(test, Err(Qzn3tError::InvalidChannel)));
        let test = test.unwrap_err();
        let s = format!("{test}");
        assert!(!s.is_empty());
    }

    #[test]
    fn audio_buffer_equal() {
        let data = vec![vec![1.0, 2.0, 3.0], vec![4.0, 5.0, 6.0]];
        let audio = AudioBuffer::new_data(data).unwrap();
        assert_eq!(audio, audio);
        let data = vec![vec![1.0, 2.0, 3.0], vec![4.0, 5.0, 6.0]];
        let audio2 = AudioBuffer::new_data(data).unwrap();
        assert_eq!(audio2, audio); // AudioBuffer has an ID but it is ignored for equality
        assert_ne!(audio.id, audio2.id);
    }

    #[test]
    fn audio_buffer_add_file_manager() {
        let data = vec![vec![1.0, 2.0, 3.0], vec![4.0, 5.0, 6.0]];
        let mut audio = AudioBuffer::new_data(data.clone()).unwrap();
        let path = temp_dir();
        let path = path.join("audio_buffer_add_file_manager");
        audio.add_file_backing(&path).unwrap();
        let raw_path = FileBacker::get_raw_path(&path);
        let md_path = FileBacker::get_metadata_path(&path.to_path_buf());
        let mut fb = audio.file_backer.take().unwrap();
        let h = fb.handle.take().unwrap();
        drop(fb);
        _ = h.join();
        // The file size of the raw data is (size of f32) x data.len() x data[0].len()
        let raw_metadata_len = fs::metadata(raw_path).unwrap().len();
        assert_eq!(
            raw_metadata_len as usize,
            std::mem::size_of::<f32>() * data.len() * data[0].len()
        );

        // Load the metadata  and check it
        let mut md_f = File::open(md_path).unwrap();
        let mut md_s = "".to_string();
        md_f.read_to_string(&mut md_s).unwrap();
        let metadata: Metadata = serde_json::from_str(&md_s).unwrap();
        assert_eq!(metadata.channels, 2);
        assert_eq!(metadata.sample_rate, get_sample_rate());
    }

    #[test]
    fn audio_buffer_add_with_file_manager() {
        let mut audio = AudioBuffer::new(2).unwrap();
        let path = temp_dir();
        let path = path.join("audio_buffer_add_with_file_manager");
        audio.add_file_backing(&path).unwrap();
        audio.add_samples(0, &[0.0f32, 0.1, 0.2]).unwrap();
        audio.add_samples(1, &[0.0f32, -0.1, -0.2]).unwrap();
        let raw_path = FileBacker::get_raw_path(&path);
        let md_path = FileBacker::get_metadata_path(&path.to_path_buf());
        let mut fb = audio.file_backer.take().unwrap();
        let h = fb.handle.take().unwrap();
        drop(fb);
        _ = h.join();
        // The file size of the raw data is (size of f32) x 6
        let raw_metadata_len = fs::metadata(raw_path).unwrap().len();
        assert_eq!(raw_metadata_len as usize, std::mem::size_of::<f32>() * 6);

        // Load the metadata  and check it
        let mut md_f = File::open(md_path).unwrap();
        let mut md_s = "".to_string();
        md_f.read_to_string(&mut md_s).unwrap();
        let metadata: Metadata = serde_json::from_str(&md_s).unwrap();
        assert_eq!(metadata.channels, 2);
        assert_eq!(metadata.sample_rate, get_sample_rate());
    }

    #[test]
    fn audio_buffer_file_round_trip() {
        let path = temp_dir();
        let path = path.join("audio_buffer_file_round_trip");
        let data1 = vec![vec![1.0, 2.0, 3.0], vec![4.0, 5.0, 6.0]];
        let ab1 = {
            let mut audio = AudioBuffer::new_data(data1.clone()).unwrap();
            audio.add_file_backing(&path).unwrap();
            audio
        };
        // Wait for the audio buffer to be written to file TODO: Add a
        // method to either check it is synced or block until it is
        // synced
        let mut z = 0;
        let lim = 100;
        loop {
            thread::sleep(Duration::from_millis(10));
            z += 1;
            if z == lim {
                break;
            }
            let p = FileBacker::get_raw_path(&path);
            if p.is_file() {
                break;
            }
        }
        let data2 = FileBacker::get_data_from_file(&path).unwrap();
        let ab2 = {
            let mut audio = AudioBuffer::new_data(data2.clone()).unwrap();
            audio.restore_file_backing(&path).unwrap();
            audio
        };
        assert_eq!(data1, data2);
        assert_eq!(ab1, ab2);
    }

    #[test]
    fn invalid_data_file() {
        // Make some bad data and write it as a FileBacker file
        let samples = [0.0f32, 0.1, 0.2, 0.3, 0.0, 0.2, 0.3];
        let bytes = samples
            .iter()
            .flat_map(|f| f.to_ne_bytes())
            .collect::<Vec<u8>>();
        let path = temp_dir();
        let path = path.join("invalid_data_file");
        let raw_path = FileBacker::get_raw_path(&path);
        let md_path = FileBacker::get_metadata_path(&path);
        let metadata = Metadata {
            channels: 2,
            sample_rate: get_sample_rate(),
        };
        let metadata = serde_json::to_string_pretty(&metadata).unwrap();
        fs::write(&md_path, metadata).unwrap();

        let mut f = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&raw_path)
            .unwrap();
        f.write_all(&bytes).unwrap();
        f.flush().unwrap();

        let mut fm = FileBacker::new(&path);
        fm.initialise(2, InitialiseMode::NoTruncate).unwrap();
        let audio_test = AudioBuffer::from_file(&path);
        assert!(audio_test.is_err());
        let test = audio_test.unwrap_err();
        let s = format!("{test}");
        assert!(!s.is_empty());

        // An extra byte
        let samples = [0.0f32, 0.1, 0.2, 0.3, 0.0, 0.1, 0.2, 0.3];
        let mut bytes = samples
            .iter()
            .flat_map(|f| f.to_ne_bytes())
            .collect::<Vec<u8>>();
        bytes.push(0);
        f.seek(SeekFrom::Start(0)).unwrap();
        f.set_len(0).unwrap();
        f.write_all(&bytes).unwrap();
        let mut fm = FileBacker::new(&path);
        fm.initialise(2, InitialiseMode::NoTruncate).unwrap();
        let audio_test = AudioBuffer::from_file(&path);
        assert!(audio_test.is_err());
        let test = audio_test.unwrap_err();
        let s = format!("{test}");
        assert!(!s.is_empty());
    }

    #[test]
    fn valid_data_file() {
        // Make some good data and write it as a FileBacker file
        let samples = [0.0f32, 0.1, 0.2, 0.3, 0.0, 0.1, 0.2, 0.3];
        let bytes = samples
            .iter()
            .flat_map(|f| f.to_ne_bytes())
            .collect::<Vec<u8>>();
        let path = temp_dir();
        let path = path.join("valid_data_file");
        let raw_path = FileBacker::get_raw_path(&path);
        let md_path = FileBacker::get_metadata_path(&path);
        let metadata = Metadata {
            channels: 2,
            sample_rate: get_sample_rate(),
        };
        let metadata = serde_json::to_string_pretty(&metadata).unwrap();
        fs::write(&md_path, metadata).unwrap();
        let mut f = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&raw_path)
            .unwrap();
        f.write_all(&bytes).unwrap();

        let mut fm = FileBacker::new(&path);
        fm.initialise(2, InitialiseMode::NoTruncate).unwrap();

        let audio_test = AudioBuffer::from_file(&path);
        assert!(audio_test.is_ok());
        assert!(audio_test.unwrap().valid());

        // Empty buffer is valid
        let audio_buffer = AudioBuffer {
            sample_rate: 48_000,
            data: vec![],
            channels: 2,
            file_backer: None,
            id: AudioBuffer::id(),
            created: Instant::now(),
        };
        assert!(audio_buffer.valid());
    }

    #[test]
    fn bad_json() {
        let path = temp_dir().join("bad_json");
        let md_path = FileBacker::get_metadata_path(&path);
        let bad_json = "{bad_json".to_string();
        fs::write(&md_path, bad_json).unwrap();
        let test = FileBacker::read_metadata(&path);
        assert!(test.is_err());
        let test = test.unwrap_err();
        let s = format!("{test}");
        assert!(!s.is_empty());
    }

    #[test]
    fn file_error() {
        let path = PathBuf::from("/dev/null");
        let test = FileBacker::read_metadata(&path);
        let test = test.unwrap_err();
        let s = format!("{test}");
        assert!(!s.is_empty());

        let mut fb = FileBacker::new(&path);
        let test = fb.initialise(2, InitialiseMode::NoTruncate);
        assert!(matches!(test, Err(Qzn3tError::FileError(_))));
        let test = test.unwrap_err();
        let s = format!("{test}");
        assert!(!s.is_empty());
    }
}
