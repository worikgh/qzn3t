// Copyright (c) 2026 Worik Turei Stanton
// License: GPL-3.0

use std::fmt;

use cpal::{
    ChannelCount, Devices, Host, SupportedStreamConfig, SupportedStreamConfigRange,
    available_hosts, default_host, host_from_id,
    traits::{DeviceTrait, HostTrait},
};
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
}
impl fmt::Debug for Engine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("")
            .field(&format!("Host: {}", self.host.id()))
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
        Self { host }
    }
    pub fn host(&self) -> &Host {
        &self.host
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
