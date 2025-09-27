// Copyright (c) 2025 Worik Turei Stanton
// License: GPL-3.0
use jack::{Client, ClientOptions};
use jack_rec::run_port;
use std::sync::mpsc;
use std::sync::mpsc::TryRecvError;
use std::thread;
use std::time::Duration;
use structs::ThisError;
mod structs;

/// Attach to a Jack port, buffer audio from it until it goes away
/// then return the data.  Designed to work with [jack_rec::run_port]
/// which records all audio on a port it creates
pub fn get_audio_from_jack(port: &str) -> Result<thread::JoinHandle<Vec<f32>>, ThisError> {
    let port = port.to_string();
    Ok(thread::spawn(move || -> Vec<f32> {
        eprintln!("DBG composer: get_audio_from_jack 1");
        // Buffer and channel to get data on
        let mut audio_data: Vec<f32> = Vec::new();
        let (sender, receiver) = mpsc::channel::<f32>();

        let async_jack_client = match run_port(port.clone(), sender) {
            Ok(p) => p,
            Err(err) => {
                eprintln!("Error composer: {err}: get audio from {port}");
                return vec![];
            }
        };
        eprintln!("DBG composer: get_audio_from_jack 1.5");
        let mut k = 0;
        loop {
            k += 1;
            if k % 10 == 0 {
                eprintln!("DBG composer: Record {k}");
            }
            match receiver.try_recv() {
                Ok(b) => audio_data.push(b),
                Err(e) => match e {
                    TryRecvError::Empty => (),
                    TryRecvError::Disconnected => break,
                },
            };
            thread::sleep(Duration::from_millis(100));
        }
        eprintln!("DBG composer: get_audio_from_jack 1.9");
        async_jack_client.deactivate().unwrap();
        eprintln!("DBG composer: get_audio_from_jack 2");
        audio_data
    }))
}

pub fn get_sample_rate() -> usize {
    let (client, _status) = Client::new("SampleRateQuery", ClientOptions::default()).unwrap();

    // Get the sample rate from the client
    client.sample_rate()
}
