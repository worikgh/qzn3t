// Copyright (c) 2026 Worik Turei Stanton
// License: GPL-3.0

use jack::{AsyncClient, AudioIn, AudioOut, Client, ClientOptions};
#[allow(unused_imports)]
use qzn3t_audio_buffer::AudioBuffer;
use qzn3terror::Qzn3tError;
#[allow(unused_imports)]
use std::{
    fmt,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
};

use crate::process_audio::{Notifications, ProcessAudio};
mod process_audio;

#[derive(Debug)]
#[allow(dead_code)]
pub struct Engine {
    /// The Jack Client
    client: Option<AsyncClient<Notifications, ProcessAudio>>,

    /// Audio data being sent into the engine.
    receivers: Vec<mpsc::Receiver<f32>>,

    /// Audio data output from engine
    senders: Vec<mpsc::Sender<f32>>,

    /// "Power" switch.  When reset the client shuts down
    run_f: Arc<AtomicBool>,

    /// When `Engine` is active it will have an `AudioBuffer`
    audio_buffer: Option<AudioBuffer>,
}
impl Engine {
    /// The `Engine` constructor.
    pub fn new() -> Result<Self, Qzn3tError> {
        // The "power switch".  Run flag...
        let run_f = Arc::new(AtomicBool::new(true));

        Ok(Self {
            client: None,
            senders: vec![],
            receivers: vec![],
            run_f,
            audio_buffer: None,
        })
    }

    /// A session defines input and output ports and the path to file backing
    pub fn start_session(&mut self, session: Session) -> Result<(), Qzn3tError> {
        self.shut_down()?; // If there is a session already end it
        self.add_client(session.in_ports, session.out_ports)?;
        self.audio_buffer = Some(AudioBuffer::new(session.in_ports.len())?);
        self.start_saving(session.path)?;
        Ok(())
    }

    /// Set up a new AsyncClient.
    #[allow(dead_code)]
    pub fn add_client(&mut self, in_p: &[&str], out_p: &[&str]) -> Result<(), Qzn3tError> {
        if let Some(c) = self.client.take() {
            c.deactivate()?;
        }
        let name = "qzn3t";
        let (client, _status) = Client::new(name, ClientOptions::NO_START_SERVER)?;
        let (process_audio, senders, receivers) =
            Self::create_process(&client, in_p, out_p, self.run_f.clone())?;
        let async_client = client.activate_async(Notifications, process_audio)?;
        self.client = Some(async_client);
        self.senders = senders;
        self.receivers = receivers;
        Ok(())
    }

    /// Shut down the engine
    #[allow(dead_code)]
    pub fn shut_down(&mut self) -> Result<(), Qzn3tError> {
        if let Some(c) = self.client.take() {
            c.deactivate()?;
        }
        self.senders.clear();
        self.receivers.clear();
        self.audio_buffer = None;
        Ok(())
    }

    /// Start saving the audio data from the inputs set up
    #[allow(unused_variables)]
    pub fn start_saving(&mut self, path: &Path) -> Result<(), Qzn3tError> {
        // Get/set up the input device
        if self.receivers.is_empty() {
            return Err(Qzn3tError::EngineNotReady);
        }
        let ch_count = self.receivers.len();
        let mut audio_buffer = AudioBuffer::new(self.receivers.len())?;
        audio_buffer.add_file_backing(path)?;
        Ok(())
    }

    // Getter
    pub fn client(&self) -> Option<&Client> {
        match &self.client {
            Some(c) => Some(c.as_client()),
            None => None,
        }
    }

    /// This is the name by which this engine's Jack client is known
    pub fn name(&self) -> Option<&str> {
        if let Some(c) = self.client() {
            Some(c.name())
        } else {
            None
        }
    }
}

/// Private methods
impl Engine {
    /// Set up the Process structure for Jack
    #[allow(clippy::type_complexity)]
    #[allow(dead_code)]
    fn create_process(
        client: &Client,
        in_p: &[&str],
        out_p: &[&str],
        run_f: Arc<AtomicBool>,
    ) -> Result<
        (
            ProcessAudio,
            Vec<mpsc::Sender<f32>>,
            Vec<mpsc::Receiver<f32>>,
        ),
        Qzn3tError,
    > {
        let mut ports_receivers = vec![];
        let mut ports_senders = vec![];
        let mut senders = vec![];
        let mut receivers = vec![];
        for p in in_p.iter() {
            let port = client.register_port(p, AudioIn::default())?;
            let (tx, rx) = mpsc::channel::<f32>();
            receivers.push(rx);
            ports_senders.push((port, tx));
        }
        for p in out_p.iter() {
            let port = client.register_port(p, AudioOut::default())?;
            let (tx, rx) = mpsc::channel::<f32>();
            senders.push(tx);
            ports_receivers.push((port, rx));
        }
        Ok((
            ProcessAudio::new(
                run_f,
                // Get data from/send data to owner
                ports_receivers,
                ports_senders,
            ),
            // For owner to use to send/reveive data
            senders,
            receivers,
        ))
    }
}

//-------------------
// Session code: TODO: move this to its own unit
pub struct Session<'a> {
    in_ports: &'a [&'a str],
    out_ports: &'a [&'a str],
    path: &'a Path,
}
impl<'a> Session<'a> {
    pub fn new(in_ports: &'a [&'a str], out_ports: &'a [&'a str], path: &'a Path) -> Self {
        Self {
            in_ports,
            out_ports,
            path,
        }
    }
}
