// Copyright (c) 2025 Worik Turei Stanton
// License: GPL-3.0

use serde::Serialize;
use std::error::Error;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};

pub struct Notifications;
impl jack::NotificationHandler for Notifications {
    fn thread_init(&self, _: &jack::Client) {
        // eprintln!("DBG NotificationHandler thread_init");
    }
    unsafe fn shutdown(&mut self, _status: jack::ClientStatus, _reason: &str) {
        // eprintln!("DBG NotificationHandler shutdown {_status:?} {_reason}");
    }
    fn freewheel(&mut self, _: &jack::Client, _is_freewheel_enabled: bool) {
        // eprintln!("DBG NotificationHandler freewheel {_is_freewheel_enabled}");
    }
    fn client_registration(&mut self, _: &jack::Client, _name: &str, _is_registered: bool) {
        // eprintln!("DBG NotificationHandler client_registration: {_name}/{_is_registered}");
    }
    fn port_registration(
        &mut self,
        _: &jack::Client,
        _port_id: jack::PortId,
        _is_registered: bool,
    ) {
        // eprintln!("DBG NotificationHandler port_registration: {_port_id}/{_is_registered}");
    }
    fn port_rename(
        &mut self,
        _: &jack::Client,
        _port_id: jack::PortId,
        _old_name: &str,
        _new_name: &str,
    ) -> jack::Control {
        // eprintln!("DBG NotificationHandler port_rename: {_port_id} {_old_name} -> {_new_name}");
        jack::Control::Continue
    }
    fn ports_connected(
        &mut self,
        _: &jack::Client,
        _port_id_a: jack::PortId,
        _port_id_b: jack::PortId,
        _are_connected: bool,
    ) {
        // eprintln!(
        //     "DBG NotificationHandler: ports_connected {_port_id_a}/{_port_id_b} {_are_connected}"
        // );
    }

    fn graph_reorder(&mut self, _: &jack::Client) -> jack::Control {
        // eprintln!("DBG NotificationHandler graph_reorder");
        jack::Control::Continue
    }

    fn xrun(&mut self, _: &jack::Client) -> jack::Control {
        eprintln!("DBG NotificationHandler xrun");
        jack::Control::Continue
    }

    fn sample_rate(&mut self, _: &jack::Client, srate: jack::Frames) -> jack::Control {
        eprintln!("DBG jack_rec: sample rate changed to {srate}");
        jack::Control::Continue
    }
}

/// Handler for audio output.  Receive multi-channel audio data on a
/// set of `mpsc::Channel`s and send them to matching Jack ports
pub struct ProcessAudioToJack {
    run_f: Arc<AtomicBool>,
    ports_receivers: Vec<(jack::Port<jack::AudioOut>, mpsc::Receiver<f32>)>,
}

impl jack::ProcessHandler for ProcessAudioToJack {
    /// If there are audio data in the channels send it out on the
    /// associated port, if no audio available send 0f32
    fn process(&mut self, _c: &jack::Client, ps: &jack::ProcessScope) -> jack::Control {
        for (port, receiver) in self.ports_receivers.iter_mut() {
            let out = port.as_mut_slice(ps);
            for s in out.iter_mut() {
                *s = match receiver.try_recv() {
                    Ok(s) => s,
                    Err(mpsc::TryRecvError::Empty) => 0.0,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        eprintln!("jack_rec. Error: Disconnected audio channel");
                        return jack::Control::Quit;
                    }
                };
            }
        }
        if !self.run_f.load(Ordering::SeqCst) {
            eprintln!("DBG jack_rec: run_f false in ProcessAudioToJack.process ");
            jack::Control::Quit
        } else {
            jack::Control::Continue
        }
    }
}

/// Handler for audio input.  Receive multi-channel audio data on a
/// set of Jack ports and send them to matching `mpsc::Channel`s
pub struct ProcessAudioFromJack {
    run_f: Arc<AtomicBool>,
    // FIXME: The `inputs` and `senders` should be in a HashMap.  Key
    // the ports, values the senders.  But since `jack::Port` is not
    // hashable, make the name of the port the key and the value a
    // 2-tupple of `jack::Port` and `mpsc::Sender`
    inports: Vec<jack::Port<jack::AudioIn>>,
    senders: Vec<mpsc::Sender<f32>>,
}
impl jack::ProcessHandler for ProcessAudioFromJack {
    fn process(&mut self, _c: &jack::Client, ps: &jack::ProcessScope) -> jack::Control {
        // Called every time there is data available
        for i in 0..self.inports.len() {
            let in_a_p: &[f32] = self.inports[i].as_slice(ps);
            for v in in_a_p {
                if let Err(err) = self.senders[i].send(*v) {
                    eprintln!(
                        "Error jack_rec: Cannot send data ({v}) through channel {i}. Error: {err} "
                    );
                    return jack::Control::Quit;
                }
            }
        }
        if !self.run_f.load(Ordering::SeqCst) {
            eprintln!("DBG jack_rec: Quit process handler");
            // Close all the pipes for sending data
            self.senders.clear();
            jack::Control::Quit
        } else {
            jack::Control::Continue
        }
    }
}

#[derive(Serialize)]
pub struct Description {
    pub sample_rate: usize,
    pub output_files: Vec<String>,
}

#[allow(clippy::type_complexity)]
/// Read audio data from Jack. Start a Jack client named `client` that
/// reads audio data from the Jack ports named in `jack_inputs` and
/// sends them on the coresponding channel in `senders`.
pub fn read_port(
    client: String,
    jack_inputs: Vec<String>,
    senders: Vec<mpsc::Sender<f32>>,
    run_f: Arc<AtomicBool>,
) -> Result<jack::AsyncClient<Notifications, ProcessAudioFromJack>, Box<dyn Error>> {
    let (client, _status) =
        match jack::Client::new(client.as_str(), jack::ClientOptions::NO_START_SERVER) {
            Ok(c) => c,
            Err(err) => {
                let msg = format!("jack_rec read_port: Error creating client: {err}");
                return Err(msg.into());
            }
        };

    let spec = jack::AudioIn::default();
    let mut inports: Vec<jack::Port<jack::AudioIn>> = vec![];
    for jp in jack_inputs.iter() {
        match client.register_port(jp, spec) {
            Ok(p) => inports.push(p),
            Err(err) => {
                let msg = format!(
                    "Error jack_rec: Cannot create inport: {}:input.  Err({err})",
                    client.name()
                );
                return Err(msg.into());
            }
        };
    }
    let inport_names = inports
        .iter()
        .map(|p| p.name().as_ref().unwrap().to_string())
        .collect::<Vec<String>>();
    let in_process = ProcessAudioFromJack {
        run_f: run_f.clone(),
        inports,
        senders,
    };
    // Activate the client, which starts the processing.
    let active_client = match client.activate_async(Notifications, in_process) {
        Ok(c) => c,
        Err(err) => {
            let msg = format!("jack_rec. read_port: Error creating active client: {err}");
            return Err(msg.into());
        }
    };
    assert_eq!(jack_inputs.len(), inport_names.len());
    for i in 0..jack_inputs.len() {
        let source_port = &jack_inputs[i];
        let destination_port = &inport_names[i];
        match active_client
            .as_client()
            .connect_ports_by_name(source_port, destination_port)
        {
            Ok(()) => (),
            Err(err) => {
                return Err(format!(
		    "qzn3t/jack_rec: Failed to connect {source_port} -> {destination_port} {err}. This client: {}", active_client.as_client().name()
		)
		.into());
            }
        }
    }
    Ok(active_client)
}

#[allow(clippy::type_complexity)]
/// Write audio data to Jack.  Start a Jack client named `client` that
/// reads audio data from the channels in `receivers` and sends them
/// on the coresponding Jack port from `jack_outputs`
pub fn write_port(
    client_name: String,
    jack_outports_receivers: Vec<(String, mpsc::Receiver<f32>)>,
    run_flag: Arc<AtomicBool>,
) -> Result<jack::AsyncClient<Notifications, ProcessAudioToJack>, Box<dyn Error>> {
    // Create a client that reads data from `mpsc::Receiver<f32>`
    // channels and makes it available on a Jack port.
    let (client, _status) =
        match jack::Client::new(client_name.as_str(), jack::ClientOptions::NO_START_SERVER) {
            Ok(c) => c,
            Err(err) => return Err(err.into()),
        };

    // The audio transmission is implemented by connecting ports.
    // Collect the port names that will be used. The destination ports
    // (fully qualified with client names) are in
    // `jack_outports_receivers`, the source ports are from the client
    // created herein, and the port name (except client part) can be
    // the same as the destination port.  Collect the names now before
    // `jack_outports_receivers` is consumed below
    let mut ports_to_connect: Vec<(String, String)> = Vec::new();
    for (port_name, _) in jack_outports_receivers.iter() {
        // `port_name` is fully qualified with client:port
        // Want just the name
        match port_name.split_once(":") {
            Some((_, name)) => {
                let source_port = format!("{client_name}:{name}");
                ports_to_connect.push((source_port, port_name.to_string()));
            }
            None => {
                let msg = format!("Invalid port name: {port_name}");
                return Err(msg.into());
            }
        };
    }
    let mut ports_receivers: Vec<(jack::Port<jack::AudioOut>, mpsc::Receiver<f32>)> = vec![];
    for (jp_name, receiver) in jack_outports_receivers {
        match client.register_port(jp_name.as_ref(), jack::AudioOut::default()) {
            Ok(p) => ports_receivers.push((p, receiver)),
            Err(err) => {
                let msg = format!(
                    "Error jack_rec: Cannot create inport: {}:input.  Err({err})",
                    client.name()
                );
                return Err(msg.into());
            }
        }
    }

    let out_process = ProcessAudioToJack {
        run_f: run_flag,
        ports_receivers,
    };
    // Activate the client, which starts the processing.
    let active_client = match client.activate_async(Notifications, out_process) {
        Ok(c) => c,
        Err(err) => {
            let msg = format!("jack_rec. write_port: Error creating active client: {err}");
            return Err(msg.into());
        }
    };

    // Connect the ports from `client` to the destination ports in `jack_outports_receivers`
    for (source_port, destination_port) in ports_to_connect.iter() {
        match active_client
            .as_client()
            .connect_ports_by_name(source_port, destination_port)
        {
            Ok(()) => (),
            Err(err) => {
                return Err(format!(
		    "qzn3t/jack_rec: Failed to connect {source_port} -> {destination_port} {err}. This client: {}", active_client.as_client().name()
		)
		.into());
            }
        }
    }
    eprintln!("DBG jack_rec: Actve client returned");
    Ok(active_client)
}
