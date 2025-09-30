// Copyright (c) 2025 Worik Turei Stanton
// License: GPL-3.0
use jack::{Client, ClientOptions};
use jack_rec::run_port;
use std::error::Error;
use std::sync::mpsc;
use std::sync::mpsc::TryRecvError;

/// Attach to a Jack port, buffer audio from it until it goes away
/// then return the data.  Designed to work with [jack_rec::run_port]
/// which records all audio on a port it creates
pub fn get_audio_from_jack(port: &str) -> Result<Vec<f32>, Box<dyn Error>> {
    // Buffer and channel to get data on
    let mut audio_data: Vec<f32> = Vec::new();
    let (sender, receiver) = mpsc::channel::<f32>();

    let async_jack_client = run_port(port.to_string(), sender)?;
    loop {
        match receiver.try_recv() {
            Ok(b) => audio_data.push(b),
            Err(e) => match e {
                TryRecvError::Empty => (),
                TryRecvError::Disconnected => break,
            },
        };
        // There should probably be a sleep here
    }
    async_jack_client.deactivate().unwrap();
    Ok(audio_data)
}

/// Required to send an audio buffer out on Jack
struct AudioPlayerState {
    data_ptr: *const f32,
    total_len: usize,
    current_pos: usize,
}

pub fn get_sample_rate() -> usize {
    let (client, _status) = Client::new("SampleRateQuery", ClientOptions::default()).unwrap();

    // Get the sample rate from the client
    client.sample_rate()
}
