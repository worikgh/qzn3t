// Copyright (c) 2025 Worik Turei Stanton
// License: GPL-3.0

//! Create a client that can be used to play audio

use crate::errors::RecorderError;
use crate::io::AudioBuffers;
use crate::test_utils::common::describe_linear_buffer;
use jack::{AsyncClient, AudioOut, Client, ClientOptions, ProcessHandler};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::sync::mpsc::TryRecvError;

/// The name of the client
const CLIENT_NAME: &str = "qzn3t-recorder";

pub struct AudioSenderState {
    audio_rxs: Vec<mpsc::Receiver<f32>>,
}

impl AudioSenderState {
    pub fn empty_rx(&mut self) {
        for rx in self.audio_rxs.iter() {
            while rx.try_recv().is_ok() {}
        }
    }
}
pub struct Notifications;
impl jack::NotificationHandler for Notifications {}
pub struct SendAudioToJackProcess {
    run_f: Arc<AtomicBool>,
    out_ports: Vec<jack::Port<AudioOut>>,
    state: AudioSenderState,

    // Debugging code: Finding zeros bug.  All good from here
    buffers: AudioBuffers,
}
impl Drop for SendAudioToJackProcess {
    fn drop(&mut self) {
        for c in 0..self.buffers.channels() {
            dbg!(describe_linear_buffer(
                self.buffers.get_buffer_idx(c as usize).unwrap()
            ));
        }
    }
}
impl ProcessHandler for SendAudioToJackProcess {
    fn process(&mut self, _: &Client, ps: &jack::ProcessScope) -> jack::Control {
        let run_flag = self.run_f.load(Ordering::Relaxed);

        // Quit the loop when all channels are disconnected.  Start
        // with the assumption that every channel is disconnected
        let channel_count = self.out_ports.len();
        let mut disconnect_guard: HashMap<usize, bool> =
            (0..channel_count).map(|c| (c, true)).collect();

        for (idx, out) in self.out_ports.iter_mut().enumerate() {
            let out = out.as_mut_slice(ps);
            if run_flag {
                for sample in out.iter_mut() {
                    match self.state.audio_rxs[idx].try_recv() {
                        Ok(s) => {
                            *sample = {
                                self.buffers.get_buffer_mut(idx as u32).unwrap().push(s);
                                s
                            }
                        }
                        Err(TryRecvError::Empty) => *sample = 0.0,
                        Err(TryRecvError::Disconnected) => {
                            disconnect_guard.insert(idx, false);
                            *sample = 0.0;
                            self.run_f.store(false, Ordering::Relaxed);

                            // When every channel is disconnected
                            if disconnect_guard.iter().all(|(_, f)| !f) {
                                return jack::Control::Quit;
                            }
                        }
                    }
                }
            } else {
                self.state.empty_rx();
                for sample in out.iter_mut() {
                    *sample = 0.0;
                }
            }
        }

        jack::Control::Continue
    }
}
/// Create the audio output port and send raw audio data received on
/// channel `data_channel` to the sound hardware.
pub fn send_audo_to_jack(
    // Audio data received on the receiver and send out on the named
    // port (fully qualified name).  Any mixing of audio happens in
    // the caller, so there is a 1-1 relationship betwen channels and
    // ports.
    mut data_channels_ports: Vec<(mpsc::Receiver<f32>, String)>,

    // When this is set to false all data is ignored
    run_f: Arc<AtomicBool>,
) -> Result<AsyncClient<Notifications, SendAudioToJackProcess>, RecorderError> {
    // Own the inputs.  `mpsc::Receiver<_>`s cannot be shared, so consume them here
    let audio_rxs: Vec<mpsc::Receiver<f32>> = data_channels_ports
        .iter_mut()
        .map(|(dc, _)| {
            let replacement: mpsc::Receiver<f32> = mpsc::channel().1;
            std::mem::replace(dc, replacement)
        })
        .collect();
    // The matching output ports - the sinks
    let sinks = data_channels_ports
        .iter()
        .map(|(_, p)| p.clone())
        .collect::<Vec<String>>();

    // Client to play audio from
    let (client, _status) = match Client::new(CLIENT_NAME, ClientOptions::NO_START_SERVER) {
        Ok(cs) => cs,
        Err(err) => {
            return Err(RecorderError::Generic(format!(
                "Cannot create client {CLIENT_NAME}: {err}"
            )));
        }
    };

    let port_name = "output";
    let out_ports: Vec<jack::Port<AudioOut>> = (0..sinks.len())
        .map(|n| {
            let name = format!("{port_name}_{}", n + 1);
            client
                .register_port(name.as_str(), AudioOut::default())
                .map_err(|err| {
                    RecorderError::Generic(format!(
                        "Cannot register port {port_name} for {CLIENT_NAME}: {err}"
                    ))
                })
        })
        .collect::<Result<Vec<_>, _>>()?;

    // For making connections
    let out_port_names = out_ports
        .iter()
        .map(|p| {
            p.name()
                .map_err(|err| panic!("Cannot get name of port: {p:?}.  Error: {err}"))
                .unwrap()
        })
        .collect::<Vec<String>>();
    assert_eq!(out_port_names.len(), sinks.len());
    // The call back handler for Jackd
    let state = AudioSenderState { audio_rxs };
    let channel_count = out_ports.len();
    let send_audio_to_jack_process = SendAudioToJackProcess {
        run_f: run_f.clone(),
        out_ports,
        state,
        buffers: AudioBuffers::new_channels(channel_count),
    };
    let active_client = match client.activate_async(Notifications, send_audio_to_jack_process) {
        Ok(ac) => ac,
        Err(err) => {
            return Err(RecorderError::Generic(format!(
                "Cannot activate client {CLIENT_NAME}: {err}"
            )));
        }
    };

    // Connect the ports that the audio data is being sent from to the
    // sinks
    for ps in out_port_names.iter().zip(sinks.iter()) {
        if let Err(err) = active_client.as_client().connect_ports_by_name(ps.0, ps.1) {
            return Err(RecorderError::Generic(format!(
                "Cannot connect ports: {err}"
            )));
        }
    }
    Ok(active_client)
}
