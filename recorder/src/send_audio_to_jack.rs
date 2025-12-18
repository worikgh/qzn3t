// Copyright (c) 2025 Worik Turei Stanton
// License: GPL-3.0

//! Create a client ad a port that can be used to play audio

use crate::errors::RecorderError;
use jack::contrib::ClosureProcessHandler;
use jack::{AudioOut, Client, ClientOptions};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::sync::mpsc::TryRecvError;

/// The name of the client
const CLIENT_NAME: &str = "qzn3t-recorder";

pub struct AudioSenderState {
    audio_rx: mpsc::Receiver<f32>,
}

impl AudioSenderState {
    pub fn empty_rx(&mut self) {
        while self.audio_rx.try_recv().is_ok() {}
    }
}

/// Create the audio output port and send raw audio data received on
/// channel `data_channel` to the sound hardware
pub fn send_audo_to_jack(
    // Audio data received on this
    data_channel: mpsc::Receiver<f32>,

    // When this is set to false all data is ignored
    audio_run: Arc<AtomicBool>,
) -> Result<impl std::any::Any, RecorderError> {
    let port_name = "output";

    // Client to play audio from
    let (client, _status) = match Client::new(CLIENT_NAME, ClientOptions::NO_START_SERVER) {
        Ok(cs) => cs,
        Err(err) => {
            return Err(RecorderError::Generic(format!(
                "Cannot create client {CLIENT_NAME}: {err}"
            )));
        }
    };
    let mut out_port = match client.register_port(port_name, AudioOut::default()) {
        Ok(p) => p,
        Err(err) => {
            return Err(RecorderError::Generic(format!(
                "Cannot register port {port_name} for {CLIENT_NAME}: {err}"
            )));
        }
    };
    let mut state = AudioSenderState {
        audio_rx: data_channel,
    };

    // The call back handler for Jackd
    let process_callback = move |_: &jack::Client, ps: &jack::ProcessScope| -> jack::Control {
        let run_flag = audio_run.load(Ordering::Relaxed);
        let out = out_port.as_mut_slice(ps);
        if run_flag {
            for sample in out.iter_mut() {
                match state.audio_rx.try_recv() {
                    Ok(s) => *sample = s,
                    Err(TryRecvError::Empty) => *sample = 0.0,
                    Err(TryRecvError::Disconnected) => {
                        *sample = 0.0;
                        eprintln!(
                            "Error recorder: Disconnected from audio channel in create_out_port"
                        );
                        return jack::Control::Quit;
                    }
                }
            }
        } else {
            state.empty_rx();
            for sample in out.iter_mut() {
                *sample = 0.0;
            }
        }
        jack::Control::Continue
    };
    let process_handler = ClosureProcessHandler::new(process_callback);
    // The name of the port being created
    let full_port_name = format!("{CLIENT_NAME}:{port_name}");
    let active_client = match client.activate_async((), process_handler) {
        Ok(ac) => ac,
        Err(err) => {
            return Err(RecorderError::Generic(format!(
                "Cannot activate client {CLIENT_NAME}: {err}"
            )));
        }
    };

    if let Err(err) = active_client
        .as_client()
        .connect_ports_by_name(&full_port_name, "system:playback_1")
    {
        return Err(RecorderError::Generic(format!(
            "Cannot connect ports: {err}"
        )));
    }
    if let Err(err) = active_client
        .as_client()
        .connect_ports_by_name(&full_port_name, "system:playback_2")
    {
        return Err(RecorderError::Generic(format!(
            "Cannot connect ports: {err}"
        )));
    }

    Ok(active_client)
}
