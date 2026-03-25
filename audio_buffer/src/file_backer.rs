// Copyright (c) 2026 Worik Turei Stanton
// License: GPL-3.0

use crate::{AudioMsg, get_sample_rate};
use qzn3terror::Qzn3tError;
use serde::{Deserialize, Serialize};
use std::{
    collections::VecDeque,
    fs::{self, File, OpenOptions, exists},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::mpsc,
    thread::{JoinHandle, spawn},
};

/// The structure that is written beside raw data files to provide
/// metadata required to convert the raw audio into other audio
/// formats
#[derive(Deserialize, Serialize, Debug)]
pub struct Metadata {
    pub channels: usize,
    pub sample_rate: u32,
}

/// Flags for initialising FileBacker
#[derive(PartialEq, Eq)]
pub enum InitialiseMode {
    Truncate,
    NoTruncate,
}

/// Manage the file backing for the AudioBuffer.
#[allow(dead_code)]
#[derive(Debug)]
pub struct FileBacker {
    /// Every FileBacker has an associated path.  It will have two
    /// files associated: ``path.with_extension("raw") for the audio
    /// data and `path.with_extension("json")` for the metadata
    path: PathBuf,

    /// The file is accessed in a dedicated thread.  `audio_tx` and
    /// `audio_rx` send audio to that thread which marshals the data
    /// and writes them to disc
    pub handle: Option<JoinHandle<Result<(), Qzn3tError>>>,

    /// Path to the file that contains the serialised `MetaData`
    /// object
    path_metadata: Option<PathBuf>,

    /// Communitcations with the `AudioBuffer` this is backing
    pub audio_rx: Option<mpsc::Receiver<AudioMsg>>,
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
    pub fn new(path: &Path) -> Self {
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
    pub fn initialise(
        &mut self,
        n_channels: usize,
        mode: InitialiseMode,
    ) -> Result<(), Qzn3tError> {
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
    pub fn send(&self, samples: &[f32], channel: usize) -> Result<(), Qzn3tError> {
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
    pub fn check_path(&self) -> Result<bool, Qzn3tError> {
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
    pub fn path(&self) -> PathBuf {
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

    /// Return meatadata from a path stem `path`
    pub fn read_metadata(path: &Path) -> Result<Metadata, Qzn3tError> {
        let md_path = FileBacker::get_metadata_path(path);
        let mut md_f = File::open(&md_path)?;
        let mut md_s = "".to_string();
        md_f.read_to_string(&mut md_s).unwrap();
        let metadata: Metadata = serde_json::from_str(&md_s)?;
        Ok(metadata)
    }

    pub fn get_raw_path(stem: &Path) -> PathBuf {
        stem.with_extension("raw")
    }

    pub fn get_metadata_path(stem: &Path) -> PathBuf {
        stem.with_extension("json")
    }
}
#[cfg(test)]
mod tests {
    use std::{env::temp_dir, path::PathBuf};

    use super::*;

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
}
