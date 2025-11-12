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

#[derive(Serialize)]
pub struct Description {
    pub sample_rate: usize,
    pub output_files: Vec<String>,
}

#[allow(clippy::type_complexity)]
/// Start a Jack client that reads audio data from `port` and sends
/// them on `sender`.
pub fn run_port(
    port: String,
    sender: mpsc::Sender<f32>,
    run_flag: Arc<AtomicBool>,
) -> Result<
    jack::AsyncClient<
        Notifications,
        jack::ClosureProcessHandler<
            impl FnMut(&jack::Client, &jack::ProcessScope) -> jack::Control,
        >,
    >,
    Box<dyn Error>,
> {
    let (client, _status) =
        jack::Client::new("qzn3t", jack::ClientOptions::NO_START_SERVER).expect("Client qzn3t");
    let spec = jack::AudioIn;
    let inport = match client.register_port("input", spec) {
        Ok(p) => p,
        Err(err) => panic!("Error jack_rec: Cannot create inport: {port}.  Err({err})"),
    };
    let to_port = inport.name().as_ref().unwrap().to_string();

    // Callback for Jack client
    let process_callback = move |_jc: &jack::Client, ps: &jack::ProcessScope| -> jack::Control {
        if !run_flag.load(Ordering::SeqCst) {
            return jack::Control::Quit;
        }
        // Called every time there is data available
        let in_a_p: &[f32] = inport.as_slice(ps);
        let mut max = 0.0;
        let mut min = 0.0;
        for v in in_a_p {
            if *v > max {
                max = *v;
            }
            if *v < min {
                min = *v;
            }
            if let Err(err) = sender.send(*v) {
                panic!("Error jack_rec: Cannot send data ({v}) through channel. Error: {err} ");
            }
        }

        // Is this needed?  No.  `writer` goes out ouf scope
        // when the Jack client is shut down with `deactivate`
        //writer.flush().unwrap();
        jack::Control::Continue
    };
    let process = jack::ClosureProcessHandler::new(process_callback);

    // Activate the client, which starts the processing.
    let active_client = client.activate_async(Notifications, process).unwrap();

    // let (client, _status) =
    //     jack::Client::new("qzn3t", jack::ClientOptions::NO_START_SERVER).expect("Client qzn3t");
    match active_client
        .as_client()
        .connect_ports_by_name(port.as_str(), to_port.as_str())
    {
        Ok(()) => (),
        Err(err) => {
            eprintln!("qzn3t/jack_rec: Failed  {} '{err}'", to_port);
        }
    }
    Ok(active_client)
}
