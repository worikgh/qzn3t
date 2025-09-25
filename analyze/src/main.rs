// Copyright (c) 2025 Worik Turei Stanton
// License: GPL-3.0

use rodio::Decoder;
use std::fs::File;
use std::io::BufReader;

fn load_audio_to_buffer(file_path: &str) -> Result<Vec<f32>, Box<dyn std::error::Error>> {
    // Open the audio file
    let file = BufReader::new(File::open(file_path)?);

    // Decode the audio file into a source
    let source = Decoder::new(file)?;

    // Collect the samples directly from the iterator
    let samples: Vec<f32> = source.collect();

    Ok(samples)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let buffer = load_audio_to_buffer("audio.wav")?;
    println!("Loaded {} samples", buffer.len());
    Ok(())
}
