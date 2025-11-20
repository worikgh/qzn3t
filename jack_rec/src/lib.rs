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
    inport: jack::Port<jack::AudioIn>,
    sender: mpsc::Sender<f32>,
}
impl jack::ProcessHandler for OutProcess {
    fn process(&mut self, _c: &jack::Client, ps: &jack::ProcessScope) -> jack::Control {
        // Called every time there is data available
        let in_a_p: &[f32] = self.inport.as_slice(ps);
        for v in in_a_p {
            if let Err(err) = self.sender.send(*v) {
                panic!("Error jack_rec: Cannot send data ({v}) through channel. Error: {err} ");
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
    jack_input: String,
    sender: mpsc::Sender<f32>,
    run_flag: Arc<AtomicBool>,
) -> Result<jack::AsyncClient<Notifications, OutProcess>, Box<dyn Error>> {
    let (client, _status) =
        jack::Client::new(client.as_str(), jack::ClientOptions::NO_START_SERVER)
            .expect("Client qzn3t");
    let spec = jack::AudioIn::default();
    let inport = match client.register_port("input", spec) {
        Ok(p) => p,
        Err(err) => panic!(
            "Error jack_rec: Cannot create inport: {}:input.  Err({err})",
            client.name()
        ),
    };
    let to_port = inport.name().as_ref().unwrap().to_string();

    let out_process = OutProcess {
        run_flag: run_flag.clone(),
        inport,
        sender,
    };
    // Activate the client, which starts the processing.
    let active_client = client.activate_async(Notifications, out_process).unwrap();

    match active_client
        .as_client()
        .connect_ports_by_name(&jack_input, to_port.as_str())
    {
        Ok(()) => (),
        Err(err) => {
            return Err(format!(
                "qzn3t/jack_rec: Failed to connect {jack_input} -> {} {err}",
                to_port
            )
            .into());
        }
    }
    Ok(active_client)
}
