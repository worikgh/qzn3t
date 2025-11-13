// Copyright (c) 2025 Worik Turei Stanton
// License: GPL-3.0

//! Create a client ad a port that can be used to play audio

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

/// Create the audio output port and send raw audio data received on a
/// channel to the sound hardware
pub fn send_audo_to_jack(
    // The name of the port being created
    port_name: &str,

    // Audio data received on this
    data_channel: mpsc::Receiver<f32>,

    // When this is set to false all data is ignored
    audio_run: Arc<AtomicBool>,
) -> Result<impl std::any::Any, jack::Error> {
    let (client, _status) = Client::new(CLIENT_NAME, ClientOptions::NO_START_SERVER)?;
    let mut out_port = client.register_port(port_name, AudioOut::default())?;
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
                        println!(
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
    let full_port_name = format!("{CLIENT_NAME}:{port_name}");
    let active_client = client.activate_async((), process_handler)?;
    active_client
        .as_client()
        .connect_ports_by_name(&full_port_name, "system:playback_1")?;
    active_client
        .as_client()
        .connect_ports_by_name(&full_port_name, "system:playback_2")?;
    Ok(active_client)
}
