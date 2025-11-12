// Copyright (c) 2025 Worik Turei Stanton
// License: GPL-3.0
use crate::errors::RecorderError;
use jack::{Client, ClientOptions};
pub fn get_sample_rate() -> usize {
    let (client, _status) = Client::new("SampleRateQuery", ClientOptions::default()).unwrap();

    // Get the sample rate from the client
    client.sample_rate()
}

pub fn audio_to_flac(input: &[f32]) -> Result<Vec<u8>, RecorderError> {
    flac_encoder::FlacBuilder::from_planar(&[input.to_vec()], get_sample_rate() as u32)
        .build()
        .map_err(|err| RecorderError::Generic(format!("{err:?}")))
}
