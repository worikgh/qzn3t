// Copyright (c) 2026 Worik Turei Stanton
// License: GPL-3.0

//! Files for cobverting audio file formats to and from
//! `Vec<Vec<f32>>`

// Use the symphonia Rust crate to write a Rust function that takes a
// path to an audio file and converts it into `Vec<Vec<f32>>`

// Here's a Rust function using the symphonia crate to decode an audio file into `Vec<Vec<f32>>` (where each inner Vec represents a channel):

// ```rust
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use qzn3terror::Qzn3tError;
use symphonia::core::audio::{AudioBufferRef, Signal};
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use symphonia_core::formats::Track;

pub fn decode_audio<P: AsRef<Path> + std::fmt::Debug>(
    path: P,
) -> Result<Vec<Vec<f32>>, Qzn3tError> {
    let file = File::open(path.as_ref())?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = path.as_ref().extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    let probed = symphonia::default::get_probe().format(
        &hint,
        mss,
        &FormatOptions::default(),
        &MetadataOptions::default(),
    )?;
    let mut format = probed.format;
    let tracks = format.tracks();
    let tracks = tracks
        .iter()
        .filter(|t| {
            t.codec_params.codec != symphonia::core::codecs::CODEC_TYPE_NULL
        })
        .collect::<Vec<&Track>>();
    let track = match tracks.first() {
        Some(&t) => t,
        None => {
            return Err(Qzn3tError::SymphoniaError(format!(
                "All tracks in {path:?} have Null codec after probe"
            )));
        }
    };

    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())?;

    let num_channels = match track.codec_params.channels.map(|c| c.count()) {
        Some(c) => c,
        None => panic!("Do not know how many channels"),
    };

    let mut channels: Vec<Vec<f32>> = vec![Vec::new(); num_channels];

    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(symphonia::core::errors::Error::IoError(_)) => break,
            Err(e) => return Err(e.into()),
        };
        //println!("Packet size (8-bit): {}", packet.data.len());

        let decoded = decoder.decode(&packet)?;

        match decoded {
            AudioBufferRef::F32(buf) => {
                for (ch, channel_data) in channels.iter_mut().enumerate() {
                    channel_data.extend_from_slice(buf.chan(ch));
                }
            }
            AudioBufferRef::S16(buf) => {
                for (ch, channel_data) in channels.iter_mut().enumerate() {
                    channel_data.extend(
                        buf.chan(ch).iter().map(|&s| s as f32 / 32768.0),
                    );
                }
            }
            AudioBufferRef::S32(buf) => {
                for (ch, channel_data) in channels.iter_mut().enumerate() {
                    channel_data.extend(
                        buf.chan(ch).iter().map(|&s| s as f32 / 2147483648.0),
                    );
                }
            }
            AudioBufferRef::U8(buf) => {
                for (ch, channel_data) in channels.iter_mut().enumerate() {
                    channel_data.extend(
                        buf.chan(ch)
                            .iter()
                            .map(|&s| (s as f32 - 128.0) / 128.0),
                    );
                }
            }
            _ => {}
        }
    }
    {
        // For debugging.
        let path_buf = PathBuf::new();
        let mut path_buf = path_buf.join(path);
        path_buf.add_extension("raw");
        let mut file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(path_buf.as_path())
            .unwrap();

        let n_samples = channels[0].len();
        let channel_cnt = channels.len();
        let buf: Vec<u8> = (0..n_samples)
            .flat_map(|s| (0..channel_cnt).map(move |c| (s, c)))
            .flat_map(|(s, c)| channels[c][s].to_ne_bytes())
            .collect();
        file.write_all(&buf)?;
    }
    Ok(channels)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_audio() {
        // Use a test audio file path
        let path = "test_audio.wav";

        // Skip test if file doesn't exist
        if !std::path::Path::new(path).exists() {
            panic!("Skipping test: {} not found", path);
        }

        let result = decode_audio(path);
        assert!(result.is_ok(), "Failed to decode audio: {:?}", result.err());

        let channels = result.unwrap();
        assert!(!channels.is_empty(), "No channels decoded");
        assert!(!channels[0].is_empty(), "No samples decoded");

        // Check samples are in valid range [-1.0, 1.0]
        for channel in &channels {
            for &sample in channel {
                if !(-1.0..1.0).contains(&sample) {
                    panic!("Sample out of range: {}", sample);
                }
            }
        }
    }

    #[test]
    fn test_decode_audio_invalid_path() {
        let result = decode_audio("nonexistent_file.wav");
        assert!(result.is_err());
    }
}
