// Copyright (c) 2025 Worik Turei Stanton
// License: GPL-3.0
use jack::{Client, ClientOptions};
/// Get the sample rate of the Jackd server
pub fn get_sample_rate() -> usize {
    let (client, _status) = match Client::new("SampleRateQuery", ClientOptions::default()) {
        Ok(cs) => cs,
        Err(err) => panic!("Cannot cleate a client t get the ample rate: {err}"),
    };

    // Get the sample rate from the client
    client.sample_rate()
}
