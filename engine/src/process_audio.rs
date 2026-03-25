// Copyright (c) 2026 Worik Turei Stanton
// License: GPL-3.0

//! Handler for audio INPUT AND output.  Receive multi-channel audio
//! data on a set of `mpsc::Channel`s and send them to matching Jack
//! ports

// use jack::{AsyncClient, AudioIn, AudioOut, Client, ClientOptions, Port, PortFlags, Unowned};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};

#[derive(Debug)]
pub struct ProcessAudio {
    run_f: Arc<AtomicBool>,     // Off switch
    set_f: Arc<AtomicBool>,     // Set when client is running
    running_f: Arc<AtomicBool>, // Reset when client stops
    pause: Arc<AtomicBool>,

    /// Receive Audiop from the code that owns this `Engine` and send
    /// it out on Jack outputs
    ports_receivers: Vec<(jack::Port<jack::AudioOut>, mpsc::Receiver<f32>)>,

    /// Receive Audiop on Jack inputs and send it to the code that
    /// owns this `Engine`
    ports_senders: Vec<(jack::Port<jack::AudioIn>, mpsc::Sender<f32>)>,
}

impl ProcessAudio {
    pub fn new(
        run_f: Arc<AtomicBool>,     // Off switch
        running_f: Arc<AtomicBool>, // Indicates process finished when reset
        set_f: Arc<AtomicBool>,     // Set when client set up
        pause: Arc<AtomicBool>,
        ports_senders: Vec<(jack::Port<jack::AudioIn>, mpsc::Sender<f32>)>,
        ports_receivers: Vec<(jack::Port<jack::AudioOut>, mpsc::Receiver<f32>)>,
    ) -> Self {
        Self {
            run_f,
            set_f,
            running_f,
            pause,
            ports_receivers,
            ports_senders,
        }
    }
}
impl jack::ProcessHandler for ProcessAudio {
    /// Full duplex audio.  Audio Output: If there are audio data in
    /// the receiver channels send it out on the associated port, if
    /// no audio available send 0_f32.  Audio Input: Any data
    /// available on the input jack ports send them out on the sender
    /// channels
    fn process(&mut self, _c: &jack::Client, ps: &jack::ProcessScope) -> jack::Control {
        self.set_f.store(true, Ordering::Relaxed);
        self.running_f.store(true, Ordering::Relaxed);
        if self.pause.load(Ordering::Relaxed) {
            return jack::Control::Continue;
        }
        for (port, receiver) in self.ports_receivers.iter_mut() {
            let out = port.as_mut_slice(ps);
            for s in out.iter_mut() {
                *s = match receiver.try_recv() {
                    Ok(s) => s,
                    Err(mpsc::TryRecvError::Empty) => 0.0,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        eprintln!("jack_rec. Error: Disconnected audio channel");
                        self.running_f.store(false, Ordering::Relaxed);
                        return jack::Control::Quit;
                    }
                };
            }
        }
        for (port, sender) in self.ports_senders.iter_mut() {
            let in_a_p: &[f32] = port.as_slice(ps);
            for v in in_a_p {
                if let Err(err) = sender.send(*v) {
                    eprintln!(
                        "Error jack_rec: Cannot send data ({v}) through a channel. Error: {err} "
                    );
                    self.running_f.store(false, Ordering::Relaxed);
                    return jack::Control::Quit;
                }
            }
        }
        if !self.run_f.load(Ordering::Relaxed) {
            self.running_f.store(false, Ordering::Relaxed);
            jack::Control::Quit
        } else {
            jack::Control::Continue
        }
    }
}

pub struct Notifications;
impl jack::NotificationHandler for Notifications {
    fn thread_init(&self, _: &jack::Client) {
        eprintln!("DBG NotificationHandler thread_init");
    }
    unsafe fn shutdown(&mut self, _status: jack::ClientStatus, _reason: &str) {
        eprintln!("DBG NotificationHandler shutdown {_status:?} {_reason}");
    }
    fn freewheel(&mut self, _: &jack::Client, _is_freewheel_enabled: bool) {
        eprintln!("DBG NotificationHandler freewheel {_is_freewheel_enabled}");
    }
    fn client_registration(&mut self, _: &jack::Client, _name: &str, _is_registered: bool) {
        eprintln!("DBG NotificationHandler client_registration: {_name}/{_is_registered}");
    }
    fn port_registration(
        &mut self,
        _: &jack::Client,
        _port_id: jack::PortId,
        _is_registered: bool,
    ) {
        eprintln!("DBG NotificationHandler port_registration: {_port_id}/{_is_registered}");
    }
    fn port_rename(
        &mut self,
        _: &jack::Client,
        _port_id: jack::PortId,
        _old_name: &str,
        _new_name: &str,
    ) -> jack::Control {
        eprintln!("DBG NotificationHandler port_rename: {_port_id} {_old_name} -> {_new_name}");
        jack::Control::Continue
    }
    fn ports_connected(
        &mut self,
        _: &jack::Client,
        _port_id_a: jack::PortId,
        _port_id_b: jack::PortId,
        _are_connected: bool,
    ) {
        eprintln!(
            "DBG NotificationHandler: ports_connected {_port_id_a}/{_port_id_b} {_are_connected}"
        );
    }
    fn graph_reorder(&mut self, _: &jack::Client) -> jack::Control {
        eprintln!("DBG NotificationHandler graph_reorder");
        jack::Control::Continue
    }

    fn xrun(&mut self, _: &jack::Client) -> jack::Control {
        eprintln!("Error NotificationHandler xrun");
        jack::Control::Continue
    }

    fn sample_rate(&mut self, _: &jack::Client, _srate: jack::Frames) -> jack::Control {
        jack::Control::Continue
    }
}
