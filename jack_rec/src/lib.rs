// Copyright (c) 2025 Worik Turei Stanton
// License: GPL-3.0

use serde::Serialize;
use std::error::Error;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};

pub struct Notifications;
impl jack::NotificationHandler for Notifications {
    fn sample_rate(&mut self, _: &jack::Client, srate: jack::Frames) -> jack::Control {
        eprintln!("DBG jack_rec: sample rate changed to {srate}");
        jack::Control::Continue
    }
}

pub struct OutProcess {
    run_flag: Arc<AtomicBool>,
    inports: Vec<jack::Port<jack::AudioIn>>,
    senders: Vec<mpsc::Sender<f32>>,
}
impl jack::ProcessHandler for OutProcess {
    fn process(&mut self, _c: &jack::Client, ps: &jack::ProcessScope) -> jack::Control {
        // Called every time there is data available
        for i in 0..self.inports.len() {
            let in_a_p: &[f32] = self.inports[i].as_slice(ps);
            for v in in_a_p {
                if let Err(err) = self.senders[i].send(*v) {
                    panic!(
                        "Error jack_rec: Cannot send data ({v}) through channel {i}. Error: {err} "
                    );
                }
            }
        }
        if !self.run_flag.load(Ordering::SeqCst) {
            dbg!(&self.run_flag);
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
/// Start a Jack client named `client` that reads audio data from
/// `jack_input` and sends them on `sender`.
pub fn run_port(
    client: String,
    // TODO: Multi-channel.  This will need to be a collection of inputs
    jack_input: Vec<String>,
    senders: Vec<mpsc::Sender<f32>>,
    run_flag: Arc<AtomicBool>,
) -> Result<jack::AsyncClient<Notifications, OutProcess>, Box<dyn Error>> {
    let (client, _status) =
        jack::Client::new(client.as_str(), jack::ClientOptions::NO_START_SERVER)
            .expect("Client qzn3t");
    let spec = jack::AudioIn::default();
    let inports = jack_input
        .iter()
        .map(|ip| match client.register_port(ip, spec) {
            Ok(p) => p,
            Err(err) => panic!(
                "Error jack_rec: Cannot create inport: {}:input.  Err({err})",
                client.name()
            ),
        })
        .collect::<Vec<jack::Port<jack::AudioIn>>>();
    let inport_names = inports
        .iter()
        .map(|p| p.name().as_ref().unwrap().to_string())
        .collect::<Vec<String>>();
    let out_process = OutProcess {
        run_flag: run_flag.clone(),
        inports,
        senders,
    };
    // Activate the client, which starts the processing.
    let active_client = client.activate_async(Notifications, out_process).unwrap();
    assert_eq!(jack_input.len(), inport_names.len());
    for i in 0..jack_input.len() {
        let source_port = &jack_input[i];
        let destination_port = &inport_names[i];
        match active_client
            .as_client()
            .connect_ports_by_name(source_port, destination_port)
        {
            Ok(()) => (),
            Err(err) => {
                return Err(format!(
                    "qzn3t/jack_rec: Failed to connect {source_port} -> {destination_port} {err}",
                )
                .into());
            }
        }
    }
    Ok(active_client)
}
