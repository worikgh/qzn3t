use std::path::PathBuf;

use cpal::{
    InputCallbackInfo,
    traits::{DeviceTrait, StreamTrait},
};
// Copyright (c) 2026 Worik Turei Stanton
// License: GPL-3.0
use qzn3t_audio_buffer::AudioBuffer;
use qzn3t_engine::{
    Engine,
    device_report,
    get_config_with_channels,
    list_devices_for_host,
    list_hosts, // , device_report
};

fn main() {
    let engine = Engine::new();
    println!("Host: {}", engine.host().id(),);
    let hosts = list_hosts().unwrap();
    let devices = list_devices_for_host(engine.host()).unwrap();
    println!(
        "Hosts:\n\t{}",
        hosts
            .iter()
            .fold("".to_string(), |a, b| format!("{a}{b}\n\t"))
    );
    println!(
        "{}",
        devices
            .map(|d| device_report(&d))
            .fold("".to_string(), |a, b| format!("{a}{b}"))
    );

    let mut devices = list_devices_for_host(engine.host()).unwrap();
    let device_name = "cpal_client_in";
    let device = devices.find(|d| {
        if let Ok(d) = d.description() {
            d.name().contains(device_name)
        } else {
            false
        }
    });
    let device = device.unwrap();
    let channels: usize = 1;
    let input_cfg = get_config_with_channels(&device, channels as u16).unwrap();
    // println!("Config is: {input_cfg:?}");

    let path: PathBuf = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/recorded.raw"));
    let err_fn = move |err| {
        eprintln!("an error occurred on stream: {err}");
    };
    let mut audio_buffer = AudioBuffer::new(vec![vec![]; channels]).unwrap();
    let mut cache = vec![Vec::<f32>::new(); channels];
    audio_buffer.add_file_backing(&path).unwrap();
    let data_callback = move |data: &[f32], _info: &InputCallbackInfo| {
        let frames = data.len() / channels;
        cache.iter_mut().for_each(|v| v.clear());
        for frame in 0..frames {
            let base = frame * channels;
            for c in 0..channels {
                cache[c].push(data[base + c]);
            }
        }
        for (c, sample) in cache.iter().enumerate().take(channels) {
            audio_buffer.add_samples(c, sample).unwrap();
        }
    };
    let stream = device
        .build_input_stream(&input_cfg.into(), data_callback, err_fn, None)
        .unwrap();
    dbg!();
    stream.play().unwrap();
    println!("<enter> to stop:");
    let mut buf = String::new();
    std::io::stdin().read_line(&mut buf).unwrap();
    dbg!();
}
