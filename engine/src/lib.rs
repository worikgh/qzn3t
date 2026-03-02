// Copyright (c) 2026 Worik Turei Stanton
// License: GPL-3.0

use std::{fmt, path::Path};

use cpal::{
    ChannelCount, Devices, Host, InputCallbackInfo, SupportedStreamConfig,
    SupportedStreamConfigRange, available_hosts, default_host, host_from_id,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use qzn3t_audio_buffer::AudioBuffer;
use qzn3terror::Qzn3tError;

/// Error conversion
pub fn list_hosts() -> Result<Vec<String>, Qzn3tError> {
    let available_hosts: Vec<&str> = available_hosts().iter().map(|h| h.name()).collect();
    Ok(available_hosts.iter().map(|h| h.to_string()).collect())
}

pub fn list_devices_for_host(host: &Host) -> Result<Devices, Qzn3tError> {
    host.devices()
        .map_err(|err| Qzn3tError::CpalError(err.to_string()))
}

pub fn device_report<T>(d: &T) -> String
where
    T: DeviceTrait,
{
    let supported_input_configs: Vec<SupportedStreamConfigRange> = match d.supported_input_configs()
    {
        Ok(sscfg) => sscfg.collect(),
        Err(_) => vec![],
    };
    let input_cfg = supported_input_configs.iter().fold("".to_string(), |a, b| {
        let a = format!("{a}\n\tChannels      {:2}", b.channels());
        let a = format!("{a}\n\tSample Format {:2}", b.sample_format());
        let a = format!("{a}\n\tBuffer Size   {:?}", b.buffer_size());
        a
    });
    let supported_output_configs: Vec<SupportedStreamConfigRange> =
        match d.supported_output_configs() {
            Ok(sscfg) => sscfg.collect(),
            Err(_) => vec![],
        };
    let output_cfg = supported_output_configs
        .iter()
        .fold("".to_string(), |a, b| {
            let a = format!("{a}\n\tChannels      {:2}", b.channels());
            let a = format!("{a}\n\tSample Format {:2}", b.sample_format());
            let a = format!("{a}\n\tBuffer Size   {:?}", b.buffer_size());
            let a = format!(
                "{a}\n\tSample Rates  {}:{}",
                b.min_sample_rate(),
                b.max_sample_rate()
            );
            a
        });
    let description = match d.description() {
        Err(_) => "".to_string(),
        Ok(description) => {
            format!("Dir: {}:{}", description.name(), description.direction(),)
        }
    };

    format!(
        "Device: {:?} {description}\nInput CFG{input_cfg}\nOutput CFG{output_cfg}",
        d.id()
    )
}

pub fn get_config_with_channels<T>(
    device: &T,
    channels: ChannelCount,
) -> Result<SupportedStreamConfig, Qzn3tError>
where
    T: DeviceTrait,
{
    if let Ok(mut input_cfg) = device.supported_input_configs() {
        match input_cfg.find(|cfg| cfg.channels() == channels) {
            Some(cfgrange) => Ok(cfgrange.with_max_sample_rate()),
            None => Err(Qzn3tError::CpalError(format!(
                "No configuration available with {channels} channels"
            ))),
        }
    } else {
        Err(Qzn3tError::CpalError(
            "No input configuration available".into(),
        ))
    }
}
pub struct Engine {
    host: Host,
    name_in: String,
    name_out: String,
    stream: Option<cpal::Stream>,
}
impl fmt::Debug for Engine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("")
            .field(&format!("Host: {}/{}", self.name_in, self.host.id()))
            .finish()
    }
}
impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}
impl Engine {
    pub fn new() -> Self {
        // Use Jack by default.  Otherwise whatever else is available
        let host = if let Some(hid) = cpal::available_hosts()
            .into_iter()
            .find(|id| *id == cpal::HostId::Jack)
        {
            host_from_id(hid).unwrap()
        } else {
            default_host()
        };
        // Default name to use to find devices.  Jack devices can be
        // renamed, yet to work out how
        let name_in = "cpal_client_in".to_string();
        let name_out = "cpal_client_out".to_string();

        Self {
            name_in,
            name_out,
            host,
            stream: None,
        }
    }

    /// Start recording from `ch_count` channels and save the raw data in `path`. Does not block
    pub fn start_recording(&mut self, ch_count: usize, path: &Path) -> Result<(), Qzn3tError> {
        // Get/set up the input device
        let mut devices = list_devices_for_host(&self.host)?;
        let device_name = self.name_in();
        let device = match devices.find(|d| {
            if let Ok(d) = d.description() {
                d.name().contains(device_name)
            } else {
                false
            }
        }) {
            Some(d) => d,
            None => return Err(Qzn3tError::NoDevice(device_name.to_string())),
        };

        let input_cfg = get_config_with_channels(&device, ch_count as u16).unwrap();
        let mut audio_buffer = AudioBuffer::new(ch_count).unwrap();
        let mut cache = vec![Vec::<f32>::new(); ch_count];
        audio_buffer.add_file_backing(path).unwrap();
        let data_callback = move |data: &[f32], _info: &InputCallbackInfo| {
            let frames = data.len() / ch_count;
            cache.iter_mut().for_each(|v| v.clear());
            for frame in 0..frames {
                let base = frame * ch_count;
                for c in 0..ch_count {
                    cache[c].push(data[base + c]);
                }
            }
            for (c, sample) in cache.iter().enumerate().take(ch_count) {
                audio_buffer.add_samples(c, sample).unwrap();
            }
        };
        let err_fn = move |err| {
            eprintln!("an error occurred on stream: {err}");
        };
        let stream = device
            .build_input_stream(&input_cfg.into(), data_callback, err_fn, None)
            .unwrap();
        stream.play().unwrap();
        self.stream = Some(stream);
        Ok(())
    }

    // Getters
    pub fn name_in(&self) -> &str {
        self.name_in.as_str()
    }
    pub fn name_out(&self) -> &str {
        self.name_out.as_str()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_engine() {
        let test = Engine::new();
        assert!(!format!("{test:?}").is_empty());
    }
}
