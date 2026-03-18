// Copyright (c) 2026 Worik Turei Stanton
// License: GPL-3.0

use jack::{AsyncClient, AudioIn, AudioOut, Client, ClientOptions};

use qzn3t_audio_buffer::AudioBuffer;
use qzn3t_audio_buffer::get_sample_rate;
use qzn3terror::Qzn3tError;

use std::{
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
};
use std::{
    sync::mpsc::TryRecvError,
    thread::{self, JoinHandle, spawn},
    time::{Duration, Instant},
};

use crate::process_audio::{Notifications, ProcessAudio};
mod process_audio;

#[derive(Debug)]
pub struct Engine {
    /// The Jack Client
    client: Option<AsyncClient<Notifications, ProcessAudio>>,

    /// Audio data being sent into the engine.
    receivers: Vec<mpsc::Receiver<f32>>,

    mode: Option<SessionMode>,

    /// Audio data output from engine
    senders: Vec<mpsc::Sender<f32>>,

    /// "Power" switch.  When reset the client shuts down
    run_f: Arc<AtomicBool>,

    /// The "pause button".  When set no recording or playing back
    /// happens.  When reset it does
    pause: Arc<AtomicBool>,

    /// When `Engine` is active it will have an `AudioBuffer`
    audio_buffer_play: Option<AudioBuffer>,
    audio_buffer_record: Option<AudioBuffer>,

    /// Base time for this engine
    base_time: Instant,
}
impl Engine {
    /// The `Engine` constructor.
    pub fn new() -> Result<Self, Qzn3tError> {
        // The "power switch" and "pause button".  Start with "power
        // on" and "paused"
        let run_f = Arc::new(AtomicBool::new(true));
        let pause = Arc::new(AtomicBool::new(true));

        Ok(Self {
            client: None,
            senders: vec![],
            receivers: vec![],
            run_f,
            pause,
            audio_buffer_play: None,
            audio_buffer_record: None,
            base_time: Instant::now(),
            mode: None,
        })
    }

    /// A session defines input and output ports, the path to file
    /// backing and the mode of the session: recording or playing
    /// audio.
    pub fn start_session(&mut self, session: Session) -> Result<(), Qzn3tError> {
        self.shut_down()?; // If there is a session already end it
        self.add_client(session.in_ports, session.out_ports)?;
        self.audio_buffer_play = Some(AudioBuffer::new(session.in_ports.len())?);
        match session.mode {
            SessionMode::FullDuplex => (), // TODO!!
            SessionMode::Playing => (),    // TODO!!
            SessionMode::Recording => self.add_path(session.path)?,
        };
        Ok(())
    }

    /// The source of time for the engine
    pub fn age(&self) -> Duration {
        self.base_time.elapsed()
    }

    /// Set up a new AsyncClient.
    pub fn add_client(&mut self, in_p: &[&str], out_p: &[&str]) -> Result<(), Qzn3tError> {
        if let Some(c) = self.client.take() {
            c.deactivate()?;
        }
        let name = "qzn3t";
        let (client, _status) = Client::new(name, ClientOptions::NO_START_SERVER)?;
        let (process_audio, senders, receivers) =
            Self::create_process(&client, in_p, out_p, self.run_f.clone(), self.pause.clone())?;
        let async_client = client.activate_async(Notifications, process_audio)?;
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
        self.audio_buffer_play = None;
        Ok(())
    }

    /// Pause the engine
    pub fn pause(&mut self) {
        self.pause.store(true, Ordering::Relaxed);
    }

    /// Restart (unpause) the engine
    pub fn unpause(&mut self) {
        self.pause.store(false, Ordering::Relaxed);
    }

    /// Set up the file backing for the audio buffer
    pub fn add_path(&mut self, path: &Path) -> Result<(), Qzn3tError> {
        // Get/set up the input device
        if self.receivers.is_empty() {
            return Err(Qzn3tError::EngineNotReady(
                "No receivers to get audio on".into(),
            ));
        }

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

    /// Main event loop.  This does not do any setup.
    ///
    /// The number of input channels in `self.receivers` and the
    /// number of output channels in `self.sendes` are related to the
    /// channels in `self.audio_buffer_play` and
    /// `self.audio_buffer_record` and the mode in [`self.mode`].
    pub fn run(mut self) -> Result<JoinHandle<Result<(), Qzn3tError>>, Qzn3tError> {
        // Preconditions I/O and audio buffers
        match self.mode {
            Some(SessionMode::Playing) => {
                assert_eq!(
                    self.senders.len(),
                    self.audio_buffer_play.as_ref().unwrap().channels()
                );
                assert!(!self.senders.is_empty());
            }
            Some(SessionMode::FullDuplex) => {
                assert_eq!(
                    self.senders.len(),
                    self.audio_buffer_play.as_ref().unwrap().channels() + self.receivers.len()
                );
                assert!(!self.receivers.is_empty());
            }
            Some(SessionMode::Recording) => {
                assert_eq!(
                    self.receivers.len(),
                    self.audio_buffer_record.as_ref().unwrap().channels()
                );
                assert!(!self.receivers.is_empty());
            }
            None => (),
        };

        // Timing for the loop, in nano-seconds and samples
        const NANO_SEC_LOOP: u128 = 10_000_000;
        let samples_per_loop = (NANO_SEC_LOOP * get_sample_rate() as u128 / 1_000_000_000) as usize;
        assert_eq!(
            samples_per_loop as u128 * 1_000_000_000 / get_sample_rate() as u128,
            NANO_SEC_LOOP,
            "The sampling rate does not divide nicely"
        );
        let ret = spawn(move || -> Result<(), Qzn3tError> {
            // For maintaining timeing in the loop
            let mut now = Instant::now();

            // the index used to play back audio
            let mut play_idx = 0usize;

            // In case of full duplex keep track of the what has been recorded but not yet played back
            let mut fd_idx = 0;

            // Dodging the borrow checker get lengths of
            // self.receivers and self.receivers here before mutably
            // borrowed in the closure below
            let receivers_len = self.receivers.len();

            loop {
                // Closure to do recording for full-duplex or recording modes
                let mut record = || -> Result<bool, Qzn3tError> {
                    for (c, r) in self.receivers.iter_mut().enumerate() {
                        match r.try_recv() {
                            Ok(s) => {
                                self.audio_buffer_record
                                    .as_mut()
                                    .unwrap()
                                    .add_samples(c, &[s])?;
                                // TODO: If full-duplex then send
                                // sample to full-duplex outputs
                            }
                            Err(err) => match err {
                                TryRecvError::Empty => continue,
                                TryRecvError::Disconnected => return Ok(false),
                            },
                        };
                    }
                    Ok(true)
                };
                let senders_cnt_play: usize; // The number of senders used for playing recorded audio

                match self.mode {
                    Some(SessionMode::Playing) => {
                        senders_cnt_play = self.senders.len();

                        let next_idx = play_idx + samples_per_loop;
                        while play_idx < self.audio_buffer_play.as_ref().unwrap().len()
                            && play_idx < next_idx
                        {
                            for (c, s) in self.senders.iter_mut().enumerate().take(senders_cnt_play)
                            {
                                let sample = self
                                    .audio_buffer_play
                                    .as_ref()
                                    .unwrap()
                                    .get_sample_play(c, play_idx)?;
                                if let Err(err) = s.send(sample) {
                                    panic!("{err}");
                                }
                            }
                        }
                        play_idx = next_idx;
                    }
                    Some(SessionMode::Recording) => {
                        if !record()? {
                            break;
                        }
                    }
                    Some(SessionMode::FullDuplex) => {
                        // TODO: Deprecate this.  Simpler to play it
                        // directly from `record` closure
                        let senders_cnt_fd = receivers_len;
                        // Do the recording first
                        if !record()? {
                            break;
                        }

                        // Play back what was just recorded from the buffer
                        for idx in fd_idx..self.audio_buffer_record.as_ref().unwrap().len() {
                            // The senders to use for full-duplex play
                            // back are after the senders for normal
                            // playback
                            let len = self.senders.len();
                            let (_, last_n) = self.senders.split_at_mut(len - senders_cnt_fd);
                            for (c, s) in last_n.iter_mut().enumerate() {
                                let sample = self
                                    .audio_buffer_record
                                    .as_ref()
                                    .unwrap()
                                    .get_sample_play(c, idx)?;
                                if let Err(err) = s.send(sample) {
                                    panic!("{err}");
                                }
                            }
                        }
                    }
                    None => (),
                };

                fd_idx = self.audio_buffer_record.as_ref().unwrap().len();

                let elapsed = now.elapsed();
                if elapsed.as_nanos() < NANO_SEC_LOOP {
                    thread::sleep(Duration::from_nanos_u128(
                        NANO_SEC_LOOP - elapsed.as_nanos(),
                    ));
                } else {
                    eprintln!(
                        "Xrun: {:?}",
                        Duration::from_nanos_u128(elapsed.as_nanos() - NANO_SEC_LOOP)
                    );
                }
                now = Instant::now();
            }
            Ok(())
        });
        Ok(ret)
    }
}

/// Private methods
impl Engine {
    /// Set up the Process structure for Jack
    #[allow(clippy::type_complexity)]
    fn create_process(
        client: &Client,
        in_p: &[&str],
        out_p: &[&str],
        run_f: Arc<AtomicBool>,
        pause: Arc<AtomicBool>,
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
                pause,
                // Send data to or get data from the owner
                ports_senders,
                ports_receivers,
            ),
            // For owner to use to send/reveive data
            senders,
            receivers,
        ))
    }
}

// Session code: TODO: move this to its own unit
#[derive(Debug, PartialEq, Eq)]
pub enum SessionMode {
    Playing,
    Recording,
    FullDuplex,
}
pub struct Session<'a> {
    in_ports: &'a [&'a str],
    out_ports: &'a [&'a str],
    path: &'a Path,
    mode: SessionMode,
}
impl<'a> Session<'a> {
    pub fn new(
        in_ports: &'a [&'a str],
        out_ports: &'a [&'a str],
        path: &'a Path,
        mode: SessionMode,
    ) -> Self {
        Self {
            in_ports,
            out_ports,
            path,
            mode,
        }
    }
}

/// Passed to `Engine`
#[derive(Debug)]
#[allow(unused)]
enum EngineCommand {
    Play,
    Record,
    FullDuplex,
}
#[derive(Debug)]
#[allow(unused)]
struct EngineCtl {
    cmd: EngineCommand,
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn engine_creation() {
        let engine = Engine::new().expect("Failed to create Engine");
        assert!(engine.client.is_none()); // Ensures the client is initially None
        assert!(engine.receivers.is_empty()); // Receivers should be empty
        assert!(engine.senders.is_empty()); // Senders should be empty
    }

    #[test]
    fn age() {
        let engine = Engine::new().expect("Failed to create Engine");
        let d1 = engine.age();
        let d2 = engine.age();
        assert!(d2.saturating_sub(d1).as_nanos() > 0);
    }
    #[test]
    fn client() {
        let engine = Engine::new().unwrap();
        let client = engine.client();
        assert!(client.is_none());
    }
    // #[test]
    // /// Basic test for Session
    // fn session(){
    //	let session = Session::new(&["input_port"], &["output_port"], Path::new("output.wav"));
    //	assert_eq!(!session.in_ports.len(), 1);
    //	assert_eq!(!session.out_ports.len(), 1);
    //	let mut engine = Engine::new().expect("Failed to create Engine");
    //	let result = engine.start_session(session);
    // }
    #[test]
    fn start_session() {
        let mut engine = Engine::new().expect("Failed to create Engine");
        assert!(engine.client().is_none());
        assert!(engine.receivers.is_empty());
        assert!(engine.senders.is_empty());
        assert!(engine.run_f.load(Ordering::Relaxed));
        assert!(engine.name().is_none());
        assert!(engine.audio_buffer_play.is_none());

        let session = Session::new(
            &["input_port"],
            &["output_port"],
            Path::new("output.wav"),
            SessionMode::Recording,
        );

        let result = engine.start_session(session);
        assert!(result.is_ok()); // The session should start without error
        assert!(engine.client.is_some()); // Client should be created
        assert_eq!(engine.receivers.len(), 1); // One receiver for the input port
        assert_eq!(engine.senders.len(), 1); // One sender for the output port
    }

    #[test]
    fn shut_down() {
        let mut engine = Engine::new().expect("Failed to create Engine");
        let session = Session::new(
            &["input_port"],
            &["output_port"],
            Path::new("output.wav"),
            SessionMode::Recording,
        );
        engine
            .start_session(session)
            .expect("Failed to start session");

        engine.shut_down().expect("Failed to shut down engine");

        assert!(engine.client.is_none()); // Client should be None after shutdown
        assert!(engine.receivers.is_empty()); // Ensure receivers are cleared
        assert!(engine.senders.is_empty()); // Ensure senders are cleared
        assert!(engine.audio_buffer_play.is_none()); // Verify audio buffer is cleared
    }

    #[test]
    fn start_saving_without_receivers() {
        let mut engine = Engine::new().expect("Failed to create Engine");
        let path = Path::new("output.wav");

        let result = engine.add_path(path);
        assert!(result.is_err()); // Should return an error since receivers are empty
        assert_eq!(
            result.unwrap_err(),
            Qzn3tError::EngineNotReady("No receivers to get audio on".into())
        ); // Check for specific error
    } //

    #[test]
    fn add_client() {
        let mut engine = Engine::new().expect("Failed to create Engine");
        assert!(engine.client().is_none());
        let session = Session::new(
            &["input_port"],
            &["output_port"],
            Path::new("output.wav"),
            SessionMode::Recording,
        );

        engine
            .start_session(session)
            .expect("Failed to start session");
        let result = engine.add_client(&["new_input_port"], &["new_output_port"]);
        assert!(engine.name().is_some());
        assert!(result.is_ok()); // It should add the client without errors
        assert!(engine.client().is_some());
    }
}
