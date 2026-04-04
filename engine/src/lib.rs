// Copyright (c) 2026 Worik Turei Stanton
// License: GPL-3.0

use crate::player::Player;
use crate::stepper::{StepCommand, StepResult, Stepper};
use jack::PortFlags;
use jack::{AsyncClient, AudioIn, AudioOut, Client, ClientOptions};
use qzn3t_audio_buffer::AudioBuffer;
use qzn3t_audio_buffer::get_sample_rate;
use qzn3terror::Qzn3tError;
pub mod stepper;
use crate::process_audio::{Notifications, ProcessAudio};
use std::fmt::{self, Debug, Formatter};
use std::{
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
};
use std::{
    thread::{self},
    time::{Duration, Instant},
};
pub mod converter;
pub mod player;
mod process_audio;

pub struct Engine {
    /// The Jack Client
    client: Option<AsyncClient<Notifications, ProcessAudio>>,

    /// Audio data being sent into the engine (playing)
    receivers: Vec<mpsc::Receiver<f32>>,

    /// Audio data output from engine (recording)
    senders: Vec<mpsc::Sender<f32>>,

    /// When reset the Jack client shuts down
    run_jack_f: Arc<AtomicBool>,

    /// When reset main loop exits
    run_f: Arc<AtomicBool>,

    /// Controlled by the Jack client.  Reset whem it quits
    running_f: Arc<AtomicBool>,

    /// This flag is set when the Jack client first calls process.
    /// TODO: This can be replaced (?) with `running_f`
    set_f: Arc<AtomicBool>,

    /// Function object driven by the main loop.
    stepper: Option<Box<dyn Stepper>>,

    /// Passing commands into the main loop.  Instantiated when main
    /// loop kicks off
    stepper_tx: Option<mpsc::Sender<StepCommand>>,
}

impl Debug for Engine {
    fn fmt(&self, _f: &mut Formatter) -> fmt::Result {
        _f.debug_struct("Engine")
            .field("client", &self.client)
            .field("receivers", &self.receivers)
            .field("senders", &self.senders)
            .field("run_jack_f", &self.run_jack_f)
            .field("running_f", &self.running_f)
            .field("set_f", &self.set_f)
            .finish()
    }
}

impl Engine {
    /// Create a new Engine instance.
    ///
    /// The engine is initialized in a stopped state with no audio ports.
    ///
    /// # Errors
    /// Returns `Qzn3tError` if the engine's internal state cannot be initialized.
    /// This is rare but may occur with memory allocation failures.
    pub fn new() -> Self {
        Self {
            client: None,
            senders: vec![],
            receivers: vec![],
            run_jack_f: Arc::new(AtomicBool::new(true)),
            run_f: Arc::new(AtomicBool::new(true)),
            running_f: Arc::new(AtomicBool::new(false)),
            set_f: Arc::new(AtomicBool::new(false)),
            stepper: None,
            stepper_tx: None,
        }
    }

    /// Create a JACK client with the given ports and blocks until the client
    /// is fully initialized and ready to process audio.
    ///
    /// Precondition: The engine must not already have an active
    /// client.
    ///
    /// # Parameters
    /// - `session`: Configuration specifying ports and backing file path
    ///
    /// # Errors
    /// Returns `Qzn3tError` if:
    /// - JACK client creation fails
    /// - Port registration fails
    /// - Timeout occurs waiting for client initialization
    pub fn start_session(
        &mut self,
        session: Session,
    ) -> Result<(), Qzn3tError> {
        assert!(self.client.is_none());
        self.add_client(session.in_ports, session.out_ports)?;
        while !self.set_f.load(Ordering::Relaxed) {
            thread::sleep(Duration::from_millis(3));
        }
        dbg!(self.get_sample_rate());
        Ok(())
    }

    /// Set up a new JACK Client with specified ports.
    ///
    /// If a client already exists, it is deactivated and replaced.
    pub fn add_client(
        &mut self,
        in_p: &[&str],
        out_p: &[&str],
    ) -> Result<(), Qzn3tError> {
        if let Some(c) = self.client.take() {
            c.deactivate()?;
        }
        let name = "Qzn3t";
        let (client, _status) =
            Client::new(name, ClientOptions::NO_START_SERVER)?;
        let process_data = Self::create_process(
            &client,
            in_p,
            out_p,
            self.run_jack_f.clone(),
            self.running_f.clone(),
            self.set_f.clone(),
        )?;
        let process_audio = process_data.process;
        let senders = process_data.senders;
        let receivers = process_data.receivers;
        let async_client =
            client.activate_async(Notifications, process_audio)?;
        self.client = Some(async_client);
        self.senders = senders;
        self.receivers = receivers;
        Ok(())
    }

    /// Shut down the engine
    pub fn shut_down(&mut self) -> Result<(), Qzn3tError> {
        if let Some(c) = self.client.take() {
            c.deactivate()?;
        }
        self.senders.clear();
        self.receivers.clear();
        self.run_jack_f.store(false, Ordering::Relaxed);
        self.run_f.store(false, Ordering::Relaxed);
        Ok(())
    }

    pub fn is_running(&self) -> bool {
        self.running_f.load(Ordering::Relaxed)
    }

    pub fn get_client(&self) -> Option<&Client> {
        match &self.client {
            Some(c) => Some(c.as_client()),
            None => None,
        }
    }

    /// This is the name by which this engine's Jack client is known
    pub fn name(&self) -> Option<&str> {
        if let Some(c) = self.get_client() {
            Some(c.name())
        } else {
            None
        }
    }

    /// Add stepper
    pub fn add_stepper(&mut self, stepper: Box<dyn Stepper>) {
        self.stepper = Some(stepper);
    }

    pub fn send_to_loop(&self, cmd: StepCommand) -> Result<(), Qzn3tError> {
        if let Some(sender) = self.stepper_tx.as_ref() {
            if let Err(err) = sender.send(cmd) {
                Err(Qzn3tError::SendError(format!("send_to_loop: {err}")))
            } else {
                Ok(())
            }
        } else {
            Err(Qzn3tError::NoCommandChannel)
        }
    }

    /// Start the main event loop in a separate thread.
    ///
    /// The loop runs in real-time, processing audio frames.
    ///
    /// # Returns
    /// Returns a `JoinHandle` to the spawned thread.
    pub fn run(
        &mut self,
    ) -> Result<thread::JoinHandle<Result<(), Qzn3tError>>, Qzn3tError> {
        // Timing for the loop, in micro-seconds and samples
        const US_PER_LOOP: u64 = 100_000;
        let samples_per_loop = (US_PER_LOOP
            * self.get_sample_rate().unwrap() as u64
            / 1_000_000) as usize;
        assert_eq!(
            samples_per_loop as u64 * 1_000_000 / get_sample_rate() as u64,
            US_PER_LOOP,
            "The sampling rate does not divide nicely"
        );

        // For maintaining timing in the loop
        let mut now = Instant::now();

        // Communitation with loop
        let (stepper_tx, stepper_rx) = mpsc::channel::<StepCommand>();
        self.stepper_tx = Some(stepper_tx);

        // Handle being paused
        let mut paused = false;

        let run_f = self.run_f.clone();
        let mut stepper = None; 
        let handle = thread::spawn(move || -> Result<(), Qzn3tError> {
            let mut timing: Vec<u128> = vec![];

            loop {
                match stepper_rx.try_recv() {
                    Ok(cmd) => {
                        match cmd {
                            StepCommand::Pause => paused = true,
                            StepCommand::Start => paused = false,
                            StepCommand::NewStepper(mut new_stepper) => {
                                new_stepper.step(samples_per_loop)?;
                                stepper = Some(new_stepper)
                            }
                            StepCommand::Exit => break,
                        };
                    }
                    Err(mpsc::TryRecvError::Empty) => (),
                    Err(mpsc::TryRecvError::Disconnected) => {
                        // This is a non-standard way to break
                        // from the main loop.
                        eprintln!("Command channel disconnected");
                        break;
                    }
                };
                if !run_f.load(Ordering::Relaxed) {
                    break;
                }
                if !paused && let Some(s) = stepper.as_mut() {
                    match s.step(samples_per_loop) {
                        Ok(StepResult::Complete) => {
                            // Finished.  Get rid of the stepper
                            stepper = None;
                        }
                        Ok(StepResult::Continue) => (),
                        Err(err) => panic!("{err}"),
                    };
                }
                let elapsed = now.elapsed();
                let elapsed = elapsed.as_micros() as u64;
                if elapsed < US_PER_LOOP {
                    let sleep_us = US_PER_LOOP - elapsed;
                    thread::sleep(Duration::from_micros(sleep_us));
                } else {
                    eprintln!(
                        "Qzn3t Xrun: {:?}",
                        Duration::from_millis(elapsed - US_PER_LOOP)
                    );
                }
                timing.push(now.elapsed().as_micros());
                now = Instant::now();
            }
            dbg!(timing.iter().sum::<u128>() as f32 / timing.len() as f32);
            Ok(())
        });
        // self.stepper = None;
        // while self.running_f.load(Ordering::Relaxed) {
        //     thread::sleep(Duration::from_secs_f32(1.0));
        // }
        Ok(handle)
    }

    /// Make connections to engine
    pub fn connect_outputs(
        &self,
        dst_ports: &[String],
    ) -> Result<(), Qzn3tError> {
        let source_port_names = self.all_ports();
        assert_eq!(dst_ports.len(), source_port_names.len());
        for (source, sink) in source_port_names.iter().zip(dst_ports.iter()) {
            if let Err(err) = self
                .get_client()
                .unwrap()
                .connect_ports_by_name(source, sink)
            {
                panic!("{err}");
            }
        }
        Ok(())
    }
    /// Steppers.
    pub fn get_player(&self, audio_buffer: AudioBuffer) -> Player {
        Player::new(audio_buffer, self.senders.clone())
    }
}

/// Result for `create_process`.  Contains the `jack::ProcessHandler`
/// for the Jack client and `mpsc::Sender<f32>` channels to pass audio
/// data to and from the main loop
struct ProcessData {
    process: ProcessAudio,
    senders: Vec<mpsc::Sender<f32>>,
    receivers: Vec<mpsc::Receiver<f32>>,
}

impl Engine {
    /// Create the data needed to create a Jack client
    #[allow(clippy::type_complexity)]
    fn create_process(
        client: &Client,
        in_p: &[&str],
        out_p: &[&str],
        run_jack_f: Arc<AtomicBool>,
        running_f: Arc<AtomicBool>,
        set_f: Arc<AtomicBool>,
    ) -> Result<ProcessData, Qzn3tError> {
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
        let process = ProcessAudio::new(
            run_jack_f,
            running_f,
            set_f,
            // Send data to or get data from the owner
            ports_senders,
            ports_receivers,
        );
        Ok(ProcessData {
            process, // For owner to use to send/reveive data
            senders,
            receivers,
        })
    }

    /// List all the names of all Jack ports the engine
    /// # Errors:
    ///
    fn all_ports(&self) -> Vec<String> {
        if let Some(client) = self.get_client() {
            client.ports(
                Some(format!("{}:", client.name()).as_str()),
                None,
                PortFlags::empty(),
            )
        } else {
            vec![]
        }
    }

    fn get_sample_rate(&self) -> Option<u32> {
        self.client
            .as_ref()
            .map(|c| c.as_client().sample_rate() as u32)
    }
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

pub struct Session<'a> {
    pub in_ports: &'a [&'a str],
    pub out_ports: &'a [&'a str],
    pub path: &'a Path,
}

impl<'a> Session<'a> {
    pub fn new(
        in_ports: &'a [&'a str],
        out_ports: &'a [&'a str],
        path: &'a Path,
    ) -> Self {
        Self {
            in_ports,
            out_ports,
            path,
        }
    }

    pub fn default_stereo_play(path: &'a Path) -> Session<'a> {
        Self {
            in_ports: &[],
            out_ports: &["system:playback_1", "system:playback_2"],
            path,
        }
    }
    pub fn default_mono_play(path: &'a Path) -> Session<'a> {
        Self {
            in_ports: &[],
            out_ports: &["system:playback_1"],
            path,
        }
    }

    // To do this need to change typf of `out_ports` to Vec<String>
    // pub fn default_channels(path: &'a Path, nchan:usize,) -> Session<'a> {
    //	let out_ports:Vec<String> = (0..nchan).map(|c| format!("system:playpack_{c}")).collect();
    //	Self {
    //	    in_ports: &[],
    //	    out_ports: &out_ports,
    //	    path,
    //	}
    // }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::path::{Path, PathBuf};
    #[test]
    fn engine_creation() {
        let engine = Engine::new();
        assert!(engine.client.is_none()); // Ensures the client is initially None
        assert!(engine.receivers.is_empty()); // Receivers should be empty
        assert!(engine.senders.is_empty()); // Senders should be None
    }

    #[test]
    fn client() {
        let engine = Engine::new();
        let client = engine.get_client();
        assert!(client.is_none());
    }

    #[test]
    fn start_session() {
        let mut engine = Engine::new();
        assert!(engine.get_client().is_none());
        assert!(engine.receivers.is_empty());
        assert!(engine.senders.is_empty());
        assert!(engine.run_jack_f.load(Ordering::Relaxed));
        assert!(engine.name().is_none());

        let session = Session::new(
            &["input_port"],
            &["output_port"],
            Path::new("output.wav"),
            // SessionMode::Recording,
        );

        let result = engine.start_session(session);
        assert!(result.is_ok()); // The session should start without error
        assert!(engine.client.is_some()); // Client should be created
        assert_eq!(engine.receivers.len(), 1); // One receiver for the input port
        assert_eq!(engine.senders.len(), 1); // One sender for the output port
    }

    #[test]
    fn add_client() {
        let mut engine = Engine::new();
        assert!(engine.get_client().is_none());
        let session = Session::new(
            &["input_port"],
            &["output_port"],
            Path::new("output.wav"),
            // SessionMode::Recording,
        );

        let result = engine.start_session(session);

        assert!(result.is_ok()); // It should add the client without errors
        // assert!(engine.name().is_some());
        // assert!(engine.client().is_some());
    }

    #[test]
    fn session_default_stereo_play() {
        let path = PathBuf::new();
        let session = Session::default_stereo_play(&path);
        assert_eq!(
            &["system:playback_1", "system:playback_2"],
            session.out_ports
        );
        assert!(session.in_ports.is_empty());
        assert_eq!(&path, session.path);
    }
}
