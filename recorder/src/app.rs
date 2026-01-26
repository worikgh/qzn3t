// Copyright (c) 2025 Worik Turei Stanton
// License: GPL-3.0

use crate::errors::RecorderError;
use crate::io::{AudioBuffers, FileManager, JackPipes};
use crate::structs::Command;
use crate::utils::get_sample_rate;
use jack_rec;
use std::error::Error;
use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::sync::Once;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::TryRecvError;
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::{Duration, Instant};

static ONCE: Once = Once::new();

/// Hold the code that runs the programme. The data is in [`AppData`]
pub struct App;
impl App {
    /// The user interface.  Starts a thread that waits for commands, and....  Return the handle
    pub fn run(
        &mut self,
        mut config_app: AppData,
    ) -> Result<thread::JoinHandle<Result<(), RecorderError>>, Box<dyn Error>> {
        // The version of `self` used inside the loop
        let app_handle = thread::spawn(move || -> Result<(), RecorderError> {
            // Main loop frequency
            let poll = Duration::from_millis(10);

            loop {
                // Keep real-time
                let now = Instant::now();

                // Get command from front end, if there is a command
                let command = match config_app.command_rx.try_recv() {
                    Ok(s) => Some(s),
                    Err(TryRecvError::Empty) => None,
                    Err(TryRecvError::Disconnected) => {
                        // TODO: This should be an error
                        eprintln!("Error recorder: Disconnected getting command");
                        break;
                    }
                };
                if let Some(command) = command {
                    // There was a command.  Carry it out
                    match command {
                        Command::Continue => (),
                        Command::DubAccept => config_app.handle_dub_accept()?,
                        Command::DubReview => config_app.handle_dub_review()?,
                        Command::Dubing => config_app.handle_dubing()?,
                        Command::Play => config_app.handle_play()?,
                        Command::Quit => {
                            config_app.quit();
                            break;
                        }
                        Command::Record => config_app.handle_record()?,
                        Command::ReviewRecord => config_app.handle_review_record()?,
                        Command::Stop => config_app.handle_audio_stop()?,
                    }
                }

                // The FileManger must keep going otherwise there is
                // nothing that can be done with the recording
                if !config_app.check_audio_file_manager()? {
                    return Err(RecorderError::FileManager(
                        "File manager has shut down - main loop".into(),
                    ));
                }

                // Keep to real-time constraints
                let delay = now.elapsed();
                if delay < poll {
                    let sleep: u64 = match (poll.as_nanos() - delay.as_nanos()).try_into() {
                        Ok(d) => d,
                        Err(err) => {
                            return Err(RecorderError::MainLoopTiming(format!(
                                "When poll is {poll:?} and the main loop took {delay:?} - {} could not be converted to u64: {err:?}",
                                poll.as_nanos() - delay.as_nanos()
                            )));
                        }
                    };
                    thread::sleep(Duration::from_nanos(sleep));
                } else {
                    eprintln!(
                        "Error qzn3t/recorder: Main loop xrun: {}ns",
                        delay.as_nanos() - poll.as_nanos()
                    );
                }
            }
            Ok(())
        }); // Closure
        Ok(app_handle)
    }

    /// Create AppData
    pub fn initialise(
        &mut self,
        audio_txs: Vec<mpsc::Sender<f32>>,
        command_rx: mpsc::Receiver<Command>,
        inputs: JackPipes,
        outputs: JackPipes,
        file_path: &Path,
    ) -> Result<AppData, Box<dyn Error>> {
        // Flag to start and stop the recorder
        let run_f = Arc::new(AtomicBool::new(true));
        let ui_run_f = Arc::new(AtomicBool::new(true));
        let run_f_ctl_c = run_f.clone();

        // Ctl-c handlers must only be set once.  Not a problem for
        // normal use, but tests are often run in parallel, so this is
        // done for testing
        ONCE.call_once(|| {
            if let Err(err) = ctrlc::set_handler(move || {
                run_f_ctl_c.store(false, Ordering::Relaxed);
            }) {
                panic!("Error qzn3t/recorder: setting Ctrl-C handler: {err}");
            }
        });
        let channels = inputs.ports().len() as u32;
        let file_manager = FileManager::new(channels, file_path)?;
        Ok(AppData {
            recorded_audio: AudioBuffers::new(),
            audio_handle: None,
            audio_txs,
            command_rx,
            run_f: run_f.clone(),
            ui_run_f,
            inputs,
            _output: outputs,
            file_manager,
        })
    }

    /// Read audio data from a file into a buffer TODO: This needs to
    /// have a parameter for the number of audio channels in the file.
    /// It should then return `AudioBuffers`.  Perhaps an optional
    /// vector of names for the channels?
    /// `file` is open for, and ready to, append Write the contents of
    /// `buffer` to `file` as binary data Return the number of bytes
    /// written to the file.  TODO: Pass a `AudioBuffers` structure,
    /// and write one or more channel to the file.
    pub fn write_f32_to_file(file: &mut fs::File, buffer: &[f32]) -> Result<usize, RecorderError> {
        let bytes = unsafe {
            std::slice::from_raw_parts(buffer.as_ptr() as *const u8, std::mem::size_of_val(buffer))
        };
        assert_eq!(std::mem::size_of_val(buffer), bytes.len());
        if let Err(err) = file.write_all(bytes) {
            return Err(RecorderError::FileManager(format!(
                "FileManager::thread_fn: Write error for file {:?}.   Error: {err} ",
                file
            )));
        }
        if let Err(err) = file.sync_data() {
            return Err(RecorderError::FileManager(format!(
                "FileManager::thread_fn: Sync error for file {file:?} Error: {err}"
            )));
        }
        Ok(bytes.len())
    }
}

/// Hold the data for the programme.
pub struct AppData {
    pub recorded_audio: AudioBuffers,
    pub audio_handle: Option<thread::JoinHandle<Result<AudioBuffers, RecorderError>>>,
    audio_txs: Vec<mpsc::Sender<f32>>,
    command_rx: mpsc::Receiver<Command>,
    pub run_f: Arc<AtomicBool>,
    pub ui_run_f: Arc<AtomicBool>,
    inputs: JackPipes,
    _output: JackPipes,
    file_manager: FileManager,
}

enum InnerJackLoopCtl {
    Continue,
    Quit,
}
impl AppData {
    /// Stop all the processes
    fn quit(&mut self) {
        _ = self.handle_audio_stop();
        self.ui_run_f.store(false, Ordering::SeqCst);
    }

    /// Spawn a thread to get audio data from a Jack port.  The
    /// thread's existence defines a "session".  The thread will send
    /// data in real time to be saved to file, and when finished
    /// returns [`AudioBuffers`] with the entire session's audio data
    fn start_getting_audio(
        &mut self,
    ) -> Result<thread::JoinHandle<Result<AudioBuffers, RecorderError>>, RecorderError> {
        // Copy of the switch to turn the recorder off
        let run_f = self.run_f.clone();

        // The names of ports (implicitly indexed by channel number,
        // 0-based)
        let jack_ports = self.inputs.ports();

        // Start up the file manager for saving recorded audio.
        // Returns a Hash of `u32` => `mpsc::Sender<f32>`.  A `Sender`
        // for each channel of audio
        self.file_manager.start()?;
        let fm_tx = self.file_manager.drain_senders();
        let channel_count = self.file_manager.channels;

        // Local switch that is set when recorder is ready and
        // recording, so this function does not return the thread
        // handle until its initialisation is over
        let active_1 = Arc::new(AtomicBool::new(false));
        let active_2 = active_1.clone();

        let result = Ok(thread::spawn(
            move || -> Result<AudioBuffers, RecorderError> {
                // Buffer and channel to get data on

                // Collect the `mpsc::Sender<f32>`s for passing to
                // Jack client to send audio data on.
                let mut buf_txs = Vec::new();

                // Collect the `mpsc::Receiver<f32>`s for receiving
                // data from the Jack client.  It will be appended to
                // the `AudioBuffer` returned from this thread and
                // sent to the `FileManager` for saving.
                let mut audio_rxs = Vec::new();

                // Names of Jack ports that inputs are going to be
                // received from
                let mut inputs = Vec::new();

                // Keep track of disconnected channels and when they
                // are all disconnected exit the loop normally
                let mut channels_connected: Vec<bool> = Vec::new();

                // To return.  Holds all recorded audio data
                let mut audio_buffers = AudioBuffers::new();

                for np in jack_ports.iter() {
                    inputs.push(np.as_str());

                    // Channels for moving audio data from Jack into this programme
                    let (buf_tx, buf_rx) = mpsc::channel::<f32>();
                    audio_rxs.push(buf_rx);
                    buf_txs.push(buf_tx);

                    channels_connected.push(true);
                    audio_buffers.add_buffer(Vec::new())?;
                }

                // Name of the Jack client that receives data from the inputs
                let client = "Qzn3t/Recorder".to_string();

                let ac = match jack_rec::read_port(client, inputs, buf_txs, run_f.clone()) {
                    Ok(p) => p,
                    Err(err) => {
                        return Err(err.into());
                    }
                };

                // Signal that this is running to caller (parent)
                active_2.store(true, Ordering::SeqCst);

                // Main loop getting data from Jack
                loop {
                    match Self::inner_audio_jack_loop(
                        &mut channels_connected,
                        &audio_rxs,
                        &fm_tx,
                        &mut audio_buffers,
                        channel_count,
                    ) {
                        Ok(ctl) => match ctl {
                            InnerJackLoopCtl::Continue => (),
                            InnerJackLoopCtl::Quit => {
                                break;
                            }
                        },
                        Err(err) => return Err(err),
                    }
                    thread::sleep(Duration::from_millis(10));
                }
                if let Err(err) = ac.deactivate() {
                    Err(RecorderError::DeactivateClientFailed(format!("{err}")))
                } else {
                    Ok(audio_buffers)
                }
            },
        ));

        // Do not return until Jack client is active.  About 5ms in testing
        let mut stuck_guard = 0;
        const STUCK_LIMIT: u32 = 100;
        const SLEEP: u64 = 10;
        while !active_1.load(Ordering::SeqCst) {
            thread::sleep(Duration::from_millis(SLEEP));
            stuck_guard += 1;
            if stuck_guard == STUCK_LIMIT {
                panic!("qzn3t/recorder Timed out creating Jack client");
            }
        }
        result
    }

    /// Called from the inner loop of `[get_audio_from_jack]`.  Loops
    /// over all channels reads any data from the channel, adds the
    /// data to the audio buffer and sends it to the file manager to
    /// be saved.  `channels_connected` is used to decide when to
    /// quit.
    fn inner_audio_jack_loop(
        channels_connected: &mut [bool],
        audio_input_channels: &[mpsc::Receiver<f32>],
        fm_tx: &[mpsc::Sender<f32>],
        audio_buffers: &mut AudioBuffers,
        _channels: u32,
    ) -> Result<InnerJackLoopCtl, RecorderError> {
        {
            if channels_connected.iter().all(|&c| !c) {
                return Ok(InnerJackLoopCtl::Quit);
            }

            // Debugging code.  Should never see this message.
            // Channels should be disconnected all in the same moment
            if channels_connected.iter().any(|&c| !c) {
                eprintln!(
                    "Error: !!! Only some channels disconnected, should not happen: {}",
                    channels_connected
                        .iter()
                        .fold("".to_string(), |a, v| format!("{a} {v}"))
                );
            }

            for (channel, receiver) in audio_input_channels.iter().enumerate() {
                if channels_connected.iter().all(|&c| !c) {
                    break;
                }
                let tx = &fm_tx[channel];

                loop {
                    match receiver.try_recv() {
                        Ok(n) => {
                            audio_buffers.get_buffer_mut(channel as u32)?.push(n);

                            if let Err(err) = tx.send(n) {
                                return Err(RecorderError::FileManager(format!(
                                    "Error sending data to FileManager: {err}"
                                )));
                            }
                        }
                        Err(TryRecvError::Disconnected) => {
                            channels_connected[channel] = false;
                            break;
                        }
                        Err(TryRecvError::Empty) => break,
                    }
                }
            }
            thread::sleep(Duration::from_millis(10));
        }
        Ok(InnerJackLoopCtl::Continue)
    }

    /// Periodically call this from the main loop.  Return Ok(true) if
    /// running properly, Ok(false) if it has stopped otherwise an
    /// error
    pub fn check_audio_file_manager(&mut self) -> Result<bool, RecorderError> {
        match self.file_manager.check() {
            Ok(f) => Ok(f),
            Err(v) => {
                for err in v.iter() {
                    eprintln!("recorder: {err}");
                }
                Err(RecorderError::FileManager(format!(
                    "FileManager has reported {} errors",
                    v.len()
                )))
            }
        }
    }

    pub fn handle_record(&mut self) -> Result<(), Box<dyn Error>> {
        // Truncate all the audio buffers
        self.recorded_audio.reset();

        self.run_f.store(true, Ordering::SeqCst);

        match self.start_getting_audio() {
            Ok(handle) => {
                self.audio_handle = Some(handle);
            }
            Err(err) => {
                return Err(err.into());
            }
        };
        Ok(())
    }

    /// Get the audio from the FileManager and output it through the outputs.
    /// The file manager must know about a raw audio file and the JSON metadata
    /// There must be at least as many auido outputs specified (`-o` on command line) as there are audio channels
    pub fn handle_play(&mut self) -> Result<(), Box<dyn Error>> {
        let (audio_path, metadata_path) = self.file_manager.make_paths()?;
        Ok(())
    }

    /// The command: stop
    pub fn handle_audio_stop(&mut self) -> Result<(), Box<dyn Error>> {
        // This ends the main loop
        self.run_f.store(false, Ordering::Relaxed);

        // Allow all the threads to stop
        thread::sleep(Duration::from_millis(100));

        if let Some(handle) = self.audio_handle.take() {
            let j = handle.join();
            match j {
                Ok(Ok(audio_buffers)) => {
                    self.recorded_audio = audio_buffers;
                    Ok(())
                }
                Ok(Err(recorder_error)) => Err(recorder_error.into()),
                Err(err) => Err(format!("Error {err:?}").into()),
            }
        } else {
            Ok(())
        }
    }

    /// Play back the audio data
    fn handle_review_record(&self) -> Result<(), Box<dyn Error>> {
        self.run_f.store(true, Ordering::Relaxed);
        // FIXME: This is possibly quite a big copy, and only does one channel.
        let buffer: Vec<f32> = self.recorded_audio.get_buffer(0)?;
        self.play_audio(buffer)
    }

    /// Play the contents of `recorded_audio` while recording separately
    fn handle_dubing(&mut self) -> Result<(), Box<dyn Error>> {
        unimplemented!();
    }

    /// Mix `recorded_audio` and `recorded_dub` and play it back
    fn handle_dub_review(&mut self) -> Result<(), Box<dyn Error>> {
        // Mix together `recorded_audio` and `recorded_dub` and play it back
        unimplemented!();
    }

    fn handle_dub_accept(&mut self) -> Result<(), RecorderError> {
        unimplemented!();
    }

    /// When a command is passed into the programme by `-k`
    pub fn handle_kommand(&mut self, k: Command) -> Result<(), Box<dyn Error>> {
        match k {
            Command::Record => {
                eprintln!("<enter> to stop");
                self.handle_record()?;
                let mut input = String::new();
                io::stdin()
                    .read_line(&mut input)
                    .expect("Failed to read line");
                self.run_f.store(false, Ordering::SeqCst);
                if let Some(h) = self.audio_handle.take() {
                    match h.join() {
                        Ok(Ok(audio_buffers)) => {
                            self.recorded_audio = audio_buffers;
                            // self.handle_save()?;
                            Ok(())
                        }
                        Ok(Err(recorder_error)) => Err(recorder_error.into()),
                        Err(err) => Err(format!(
                            "Error qzn3t/recorder: Failed getting data: {err:?}"
                        )
                        .into()),
                    }
                } else {
                    Err("Error qzn3t/recorder: Failed to take record_handle".into())
                }
            }
            Command::Play => self.handle_play(),
            _ => panic!("Error recorder: -k {k:?} is not handled"),
        }
    }

    /// Send `recorded_audio` to the backend to play.  Consumes the
    /// passed data. FIXME: Only does one channel.
    fn play_audio(&self, audio: Vec<f32>) -> Result<(), Box<dyn Error>> {
        let txs = self.audio_txs.clone();

        // Flag to shut down playback from the UI
        let audio_run = self.run_f.clone();

        // Must copy the data so the playback is independant of the
        // original buffer remaining
        // let data = data.to_vec();
        thread::spawn(move || {
            let sample_rate = get_sample_rate();

            // Send a block of data every 100ms
            let blk_sz = sample_rate / 10;
            let mut k = 0;

            for i in audio.iter() {
                // Allow stopping play back before end of buffer
                if !audio_run.load(Ordering::Relaxed) {
                    break;
                }
                if let Err(e) = txs[0].send(*i) {
                    // TODO: This should be an error
                    eprintln!("recorder Error sending data in play_audio {e}");
                    break;
                }

                k += 1;
                if k == blk_sz {
                    thread::sleep(std::time::Duration::from_millis(100));
                    k = 0;
                }
            }
        });
        Ok(())
    }
}
