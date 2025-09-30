// Copyright (c) 2025 Worik Turei Stanton
// License: GPL-3.0

use jack::contrib::ClosureProcessHandler;
use jack::{AudioOut, Client, ClientOptions};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::sync::mpsc::TryRecvError;

pub struct AudioSenderState {
    audio_rx: mpsc::Receiver<f32>,
}

impl AudioSenderState {
    pub fn empty_rx(&mut self) {
        while self.audio_rx.try_recv().is_ok() {}
    }
}

/// Create the audio output port that monitors a channel `data_channel` that the main
/// programme can use to send (raw) audio to the Jack output
pub fn create_out_port(
    port_name: &str,
    data_channel: mpsc::Receiver<f32>,
    ok_to_run: Arc<AtomicBool>,
) -> Result<impl std::any::Any, jack::Error> {
    let (client, _status) = Client::new("qzn3t", ClientOptions::NO_START_SERVER)?;
    let mut out_port = client.register_port(port_name, AudioOut::default())?;
    let mut state = AudioSenderState {
        audio_rx: data_channel,
    };
    let process_callback = move |_: &jack::Client, ps: &jack::ProcessScope| -> jack::Control {
        let out = out_port.as_mut_slice(ps);
        let run_flag = ok_to_run.load(Ordering::Relaxed);
        if run_flag {
            for sample in out.iter_mut() {
                match state.audio_rx.try_recv() {
                    Ok(s) => *sample = s,
                    Err(TryRecvError::Empty) => *sample = 0.0,
                    Err(TryRecvError::Disconnected) => {
                        *sample = 0.0;
                        eprintln!(
                            "Error compose: Disconnected from audio channel in create_out_port"
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
    let active_client = client.activate_async((), process_handler)?;
    active_client
        .as_client()
        .connect_ports_by_name("qzn3t:output", "system:playback_1")?;
    active_client
        .as_client()
        .connect_ports_by_name("qzn3t:output", "system:playback_2")?;
    eprintln!("DBG composer: create_out_port returns: active_client: {active_client:?}");
    Ok(active_client)
}
