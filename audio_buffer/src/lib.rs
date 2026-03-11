// Copyright (c) 2026 Worik Turei Stanton
// License: GPL-3.0

use qzn3terror::Qzn3tError;
use serde::{Deserialize, Serialize};
use std::fs::exists;
use std::{
    collections::VecDeque,
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::mpsc,
    thread::{JoinHandle, spawn},
};
use uuid::{Context, Timestamp, Uuid};

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
    pub fn new(channels: usize) -> Result<Self, Qzn3tError> {
        let id = Self::id();
        let file_backer = None;
        let data = vec![vec![]; channels];
        let this = Self {
            channels,
            data,
            id,
            file_backer,
        };
        if this.valid() {
            Ok(this)
        } else {
            Err(Qzn3tError::InvalidAudioData)
        }
    }

    pub fn new_data(data: Vec<Vec<f32>>) -> Result<Self, Qzn3tError> {
        let id = Self::id();
        let file_backer = None;
        let channels = data.len();
        let this = Self {
            channels,
            data,
            id,
            file_backer,
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

    pub fn restore_file_backing(&mut self, path: &Path) -> Result<(), Qzn3tError> {
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

    /// Writing data.  A channel at a time
    #[allow(dead_code)]
    pub fn add_samples(&mut self, channel: usize, data: &[f32]) -> Result<(), Qzn3tError> {
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

    /// Check that all channels have the same number of samples and
    /// the `channels` field is correct
    #[allow(dead_code)]
    pub fn valid(&self) -> bool {
        assert_eq!(self.id.get_version_num(), 6);
        if self.data.is_empty() {
            true
        } else {
            let s = self.data[0].len();
            self.data.len() == self.channels && self.data.iter().all(|d| d.len() == s)
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
    fn from_bytes(bytes: &[u8], channels: usize) -> Result<Vec<Vec<f32>>, Qzn3tError> {
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

/// The structure that is written beside raw data files to provide
/// metadata required to convert the raw audio into other audio
/// formats
#[derive(Deserialize, Serialize, Debug)]
struct Metadata {
    pub channels: usize,
    pub sample_rate: u32,
}

/// Flags for initialising FileBacker
#[derive(PartialEq, Eq)]
enum InitialiseMode {
    Truncate,
    NoTruncate,
}

/// Manage the file backing for the AudioBuffer.
#[allow(dead_code)]
#[derive(Debug)]
struct FileBacker {
    /// Every FileBacker has an associated path.  It will have two
    /// files associated: ``path.with_extension("raw") for the audio
    /// data and `path.with_extension("json")` for the metadata
    path: PathBuf,

    /// The file is accessed in a dedicated thread.  `audio_tx` and
    /// `audio_rx` send audio to that thread which marshals the data
    /// and writes them to disc
    handle: Option<JoinHandle<Result<(), Qzn3tError>>>,

    /// Path to the file that contains the serialised `MetaData`
    /// object
    path_metadata: Option<PathBuf>,

    /// Communitcations with the `AudioBuffer` this is backing
    audio_rx: Option<mpsc::Receiver<AudioMsg>>,
    audio_tx: mpsc::Sender<AudioMsg>,
}
impl PartialEq for FileBacker {
    fn eq(&self, other: &Self) -> bool {
        self.path_metadata == other.path_metadata
            && self.path == other.path
            && self.handle.is_some() == other.handle.is_some()
    }
}
#[allow(dead_code)]
impl FileBacker {
    fn new(path: &Path) -> Self {
        let (audio_tx, audio_rx) = Self::mk_channels();
        Self {
            path: path.to_path_buf(),
            handle: None,
            path_metadata: None,
            audio_rx: Some(audio_rx),
            audio_tx,
        }
    }

    fn mk_channels() -> (mpsc::Sender<AudioMsg>, mpsc::Receiver<AudioMsg>) {
        mpsc::channel::<AudioMsg>()
    }

    /// Start the thread that receives audio data on self.receiversaudio_rx
    ///  and marshals it to save it to file.
    ///
    /// The `FileBacker` must be ready with a valid `path` and `audio_rx`
    ///  initialised.
    ///
    /// `mode` specifies if the file is truncated.  `n_channels` specifies the
    /// number of audio channels.
    //
    // The thread is started and the
    fn initialise(&mut self, n_channels: usize, mode: InitialiseMode) -> Result<(), Qzn3tError> {
        let sample_rate = get_sample_rate();
        let metadata = Metadata {
            channels: n_channels,
            sample_rate,
        };
        let raw_path = Self::get_raw_path(&self.path);
        let md_path = FileBacker::get_metadata_path(&self.path);
        match mode {
            InitialiseMode::Truncate => {
                File::create(&raw_path)?;
                File::create(&md_path)?;
            }
            InitialiseMode::NoTruncate => {
                if !exists(&raw_path)? {
                    return Err(Qzn3tError::FileError(format!(
                        "Path {raw_path:?} does not exist"
                    )));
                }
                if !exists(&md_path)? {
                    return Err(Qzn3tError::FileError(format!(
                        "Path {md_path:?} does not exist"
                    )));
                }
            }
        };
        let audio_rx = if let Some(audio_rx) = self.audio_rx.take() {
            audio_rx
        } else {
            return Err(Qzn3tError::FileBackerNotReady(
                "audio receivers not intialised".to_string(),
            ));
        };
        // Thread to read data from `audio_rx` and marshal it into a file
        let handle = spawn(move || -> Result<(), Qzn3tError> {
            let mut buffers: Vec<VecDeque<f32>> = vec![];
            for _ in 0..n_channels {
                buffers.push(VecDeque::new());
            }
            let mut handle_raw = OpenOptions::new()
                .create(true) // create if missing
                .write(true) // open in append mode
                .truncate(mode == InitialiseMode::Truncate)
                .open(&raw_path)
                .map_err(|_| Qzn3tError::InvalidPath(raw_path.clone()))?;

            match mode {
                InitialiseMode::Truncate => {
                    handle_raw.seek(SeekFrom::Start(0))?;
                    handle_raw.set_len(0)?;
                }
                InitialiseMode::NoTruncate => {
                    handle_raw.seek(SeekFrom::End(0))?;
                }
            };
            assert!(raw_path.is_file());

            while let Ok(AudioMsg {
                samples: data,
                channel,
            }) = audio_rx.recv()
            {
                buffers[channel].extend(data);
                if channel == n_channels - 1 {
                    // `available` is the minimum amount of data available for all channels.
                    let available = buffers.iter().map(VecDeque::len).min().unwrap_or(0);

                    if available > 0 {
                        let mut bytes: Vec<u8> = vec![];
                        for _ in 0..available {
                            for buf in buffers.iter_mut() {
                                let sample = buf.pop_front().unwrap();
                                let sample_bytes = sample.to_ne_bytes();
                                bytes.extend_from_slice(&sample_bytes);
                            }
                        }
                        handle_raw
                            .write_all(&bytes)
                            .map_err(|err| Qzn3tError::FileError(format!("{err}")))?;
                    }
                    let max = buffers.iter().map(VecDeque::len).max().unwrap_or(0);
                    if max > 0 {
                        dbg!(max);
                    }
                }
            }
            Ok(())
        });
        // This is very reliable.  Unwrap OK
        let metadata = serde_json::to_string_pretty(&metadata).unwrap();
        fs::write(&md_path, metadata)
            .map_err(|err| Qzn3tError::FileError(format!("Path: {md_path:?} Error: {err}")))?;

        self.handle = Some(handle);

        Ok(())
    }

    /// Send data to the thread that manages the disc file
    fn send(&self, samples: &[f32], channel: usize) -> Result<(), Qzn3tError> {
        self.audio_tx
            .send(AudioMsg {
                samples: samples.to_vec(),
                channel,
            })
            .map_err(|seend_err| Qzn3tError::SendError(format!("{seend_err}")))
    }

    /// The member `path` must be a file path and point into a
    /// directory that we can read and write from.  The file does not
    /// need to exist.  When it is created the `path` is the stem, and
    /// it will have ".raw" and ".json" suffixes added
    fn check_path(&self) -> Result<bool, Qzn3tError> {
        if self.path.exists() && !self.path.is_file() {
            return Ok(false);
        }
        // Get parent directory
        let parent = self
            .path
            .parent()
            .ok_or_else(|| Qzn3tError::FileError(format!("{:?} has no parent", self.path)))?;

        // Test if parent is writable by trying to create a temp file
        let temp = parent.join(format!(".tmp_{}", std::process::id()));
        let ret = match fs::File::create(&temp) {
            Ok(_) => fs::remove_file(&temp).is_ok(),

            Err(_) => false,
        };
        if ret && fs::read_dir(parent).is_ok() {
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Getter for the file path
    fn path(&self) -> PathBuf {
        self.path.clone()
    }

    pub fn get_data_from_file(path: &Path) -> Result<Vec<Vec<f32>>, Qzn3tError> {
        let metadata = FileBacker::read_metadata(path)?;
        let channels = metadata.channels;
        let rd_path = FileBacker::get_raw_path(path);
        let mut file = OpenOptions::new()
            .read(true)
            .write(false)
            .create(false)
            .open(&rd_path)?;
        file.seek(SeekFrom::Start(0))?;

        // Get the bytes
        let mut buf = Vec::new();
        file.read_to_end(&mut buf)?;
        let f32sz = std::mem::size_of::<f32>();
        assert_eq!(f32sz, 4);

        // Ensure length is multiple of size_of(f32)
        if !buf.len().is_multiple_of(f32sz) {
            return Err(Qzn3tError::FileError(
                "file length is not multiple of size of f32".to_string(),
            ));
        }

        // Convert bytes -> f32 preserving native endianness Safe
        // because f32 has no padding and buf length is multiple of
        // size of f32
        let samples: Vec<f32> = buf
            .chunks_exact(4)
            .map(|chunk| {
                let arr = [chunk[0], chunk[1], chunk[2], chunk[3]];
                f32::from_ne_bytes(arr)
            })
            .collect();
        // All channels must have same number of data
        if !samples.len().is_multiple_of(channels) {
            return Err(Qzn3tError::InvalidAudioData);
        }

        let mut ret = vec![vec![]; channels];
        for chunk in samples.chunks_exact(channels) {
            for c in 0..channels {
                ret[c].push(chunk[c]);
            }
        }
        Ok(ret)
    }

    fn read_metadata(path: &Path) -> Result<Metadata, Qzn3tError> {
        let md_path = FileBacker::get_metadata_path(path);
        let mut md_f = File::open(&md_path)?;
        let mut md_s = "".to_string();
        md_f.read_to_string(&mut md_s).unwrap();
        let metadata: Metadata = serde_json::from_str(&md_s)?;
        Ok(metadata)
    }

    fn get_raw_path(stem: &Path) -> PathBuf {
        stem.with_extension("raw")
    }
    fn get_metadata_path(stem: &Path) -> PathBuf {
        stem.with_extension("json")
    }
}

/// Messages from the AudioBuffer -> FileBacker
struct AudioMsg {
    samples: Vec<f32>,
    channel: usize,
}

//impl AudioMsg
#[cfg(test)]
mod tests {
    use std::{env::temp_dir, fs::File, io::Read, thread, time::Duration};

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
    fn audio_buffer_as_bytes_empty() {
        let audio = AudioBuffer {
            data: vec![],
            channels: 1,
            id: AudioBuffer::id(),
            file_backer: None,
        };
        assert!(audio.as_bytes().is_empty());
    }

    #[test]
    fn audio_buffer_as_bytes_single_channel() {
        let audio = AudioBuffer::new_data(vec![vec![1.0f32, 2.0f32, 3.0f32]]).unwrap();
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
        let audio_buffer =
            AudioBuffer::new_data(vec![vec![1.0f32, 2.0f32], vec![3.0f32, 4.0f32]]).unwrap();

        let bytes = audio_buffer.as_bytes();

        // Should be 4 samples * 4 bytes per f32
        assert_eq!(bytes.len(), 16);
        // Verify flattened order
        let floats: Vec<f32> = bytes
            .chunks_exact(std::mem::size_of::<f32>())
            .map(|chunk| f32::from_ne_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
            .collect();
        assert_eq!(floats, vec![1.0f32, 2.0f32, 3.0f32, 4.0f32]);
    }

    #[test]
    fn audio_buffer_usage() {
        let data = vec![vec![1.0, 2.0, 3.0], vec![4.0, 5.0, 6.0]];
        let audio = AudioBuffer {
            file_backer: None,
            data: data.clone(),
            id: AudioBuffer::id(),
            channels: 2,
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
            file_backer: None,
            data: data.clone(),
            channels: 2,
            id: AudioBuffer::id(),
        };
        assert!(audio.valid());
    }

    #[test]
    fn audio_buffer_invalid_channels() {
        let data = vec![vec![1.0, 2.0, 3.0], vec![4.0, 5.0, 6.0]];
        let audio = AudioBuffer {
            file_backer: None,
            data: data.clone(),
            channels: 3,
            id: AudioBuffer::id(),
        };
        assert!(!audio.valid());
    }

    #[test]
    fn audio_buffer_invalid() {
        let data = vec![vec![1.0, 2.0], vec![4.0, 5.0, 6.0]];
        let audio = AudioBuffer {
            file_backer: None,
            data: data.clone(),
            channels: 2,
            id: AudioBuffer::id(),
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
    fn file_backer_new() {
        let path = PathBuf::new();
        let fm = FileBacker::new(&path);
        assert!(fm.handle.is_none());
        assert!(fm.audio_rx.is_some());
        assert_eq!(fm.path(), path);
        assert!(fm.handle.is_none());
    }

    #[test]
    fn file_backer_path_dir() {
        let path = temp_dir();
        assert!(path.exists());
        let fm = FileBacker::new(&path);
        let test = fm.check_path();
        assert!(test.is_ok());
        assert!(!test.unwrap());
    }
    #[test]
    fn file_backer_good_path() {
        let path = temp_dir();
        let path = path.join("test_path");
        assert!(!path.exists());
        let fm = FileBacker::new(&path);
        let test = fm.check_path();
        assert!(test.is_ok());
        assert!(test.unwrap());
    }
    #[test]
    fn file_backer_invalid_path() {
        let path = PathBuf::from("/qqqq");
        assert!(!path.exists());
        let fm = FileBacker::new(&path);
        let test = fm.check_path();
        assert!(test.is_ok());
        assert!(!test.unwrap());
        let path = PathBuf::from("/dev/null");
        assert!(path.exists());
        let fm = FileBacker::new(&path);
        let test = fm.check_path();
        assert!(test.is_ok());
        assert!(!test.unwrap());
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
            data: vec![],
            channels: 2,
            file_backer: None,
            id: AudioBuffer::id(),
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
