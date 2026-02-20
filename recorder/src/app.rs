// Copyright (c) 2025 Worik Turei Stanton
// License: GPL-3.0

use crate::errors::RecorderError;
use crate::io::{AudioBuffers, FileManager, JackPipes, read_f32_vec_from_file, read_file_metadata};
use crate::send_audio_to_jack;
use crate::structs::Command;
use crate::test_utils::common::describe_linear_buffer;
use jack_rec;
use std::error::Error;
use std::io::{self};
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
    /// Construction.
    pub fn initialise(
        inputs: JackPipes,
        outputs: JackPipes,
        file_path: &Path,
        silent: bool,
    ) -> Result<AppData, Box<dyn Error>> {
        Self::initialise_inner(None, inputs, outputs, file_path, silent)
    }
    pub fn initialise_ui(
        command_rx: mpsc::Receiver<Command>,
        inputs: JackPipes,
        outputs: JackPipes,
        file_path: &Path,
        silent: bool,
    ) -> Result<AppData, Box<dyn Error>> {
        Self::initialise_inner(Some(command_rx), inputs, outputs, file_path, silent)
    }

    /// The user interface.  Starts a thread that waits for commands.
    /// Returns the handle
    pub fn run_ui(
        mut config_app: AppData,
    ) -> Result<thread::JoinHandle<Result<(), RecorderError>>, Box<dyn Error>> {
        let app_handle = thread::spawn(move || -> Result<(), RecorderError> {
            // Main loop frequency
            let poll = Duration::from_millis(100);

            // Channel to receive commands on
            let command_rx = match config_app.command_rx.take() {
                Some(rx) => rx,
                None => {
                    return Err(RecorderError::Generic(
                        "No command rx passed to run_ui".into(),
                    ));
                }
            };

            loop {
                // Keep real-time
                let now = Instant::now();

                // Get command from front end, if there is a command
                let command = match command_rx.try_recv() {
                    Ok(s) => Some(s),
                    Err(TryRecvError::Empty) => None,
                    Err(TryRecvError::Disconnected) => {
                        return Err(RecorderError::Generic(
                            "Command channel disconnected".to_string(),
                        ));
                    }
                };
                if let Some(command) = command {
                    // There was a command.  Carry it out
                    match command {
                        Command::Continue => (),
                        Command::DubAccept => config_app.handle_dub_accept()?,
                        Command::DubReview => config_app.handle_dub_review()?,
                        Command::Dubing => config_app.handle_dubing()?,
                        Command::Play => {
                            // Do not block when called from UI
                            config_app.handle_play()?;
                        }
                        Command::Quit => {
                            config_app.quit();
                            break;
                        }
                        Command::Record => config_app.handle_record()?,
                        Command::ReviewRecord => config_app.handle_review_record()?,
                        Command::Stop => config_app.handle_audio_stop()?,
                    }
                    // For recording and playback the FileManger must keep
                    // going otherwise there is nothing that can be done
                    // with the recording
                    match command {
                        Command::Record | Command::Play => {
                            if !config_app.check_audio_file_manager()? {
                                return Err(RecorderError::FileManager(
                                    "File manager has shut down - main loop".into(),
                                ));
                            }
                        }
                        _ => (),
                    };
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

    /// Construction code in common to running with and without a UI
    fn initialise_inner(
        command_rx: Option<mpsc::Receiver<Command>>,
        inputs: JackPipes,
        outputs: JackPipes,
        file_path: &Path,
        silent: bool,
    ) -> Result<AppData, Box<dyn Error>> {
        // Flag to start and stop recording/playback
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
            command_rx,
            run_f: run_f.clone(),
            ui_run_f,
            inputs,
            output: outputs,
            file_manager,
            silent,
        })
    }
}

/// Hold the data for the programme.
pub struct AppData {
    pub recorded_audio: AudioBuffers,
    pub audio_handle: Option<thread::JoinHandle<Result<AudioBuffers, RecorderError>>>,
    command_rx: Option<mpsc::Receiver<Command>>,
    pub run_f: Arc<AtomicBool>,
    pub ui_run_f: Arc<AtomicBool>,
    inputs: JackPipes,
    output: JackPipes,
    pub file_manager: FileManager,
    pub silent: bool, // Suppress all stdout
}

impl AppData {
    /// Stop all the processes
    fn quit(&mut self) {
        _ = self.handle_audio_stop();
        self.ui_run_f.store(false, Ordering::Relaxed);
    }

    /// Spawn a thread to get audio data from Jack.  The thread's
    /// existence defines a "recording session".  The thread will send
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
                active_2.store(true, Ordering::Relaxed);

                // Main loop getting data from Jack
                loop {
                    if !Self::inner_audio_jack_loop(
                        &mut channels_connected,
                        &audio_rxs,
                        &fm_tx,
                        &mut audio_buffers,
                        channel_count,
                    )? {
                        break;
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
        while !active_1.load(Ordering::Relaxed) {
            thread::sleep(Duration::from_millis(SLEEP));
            stuck_guard += 1;
            if stuck_guard == STUCK_LIMIT {
                panic!("qzn3t/recorder Timed out creating Jack client");
            }
        }
        result
    }

    fn get_audio_from_file(&self) -> Result<AudioBuffers, RecorderError> {
        let (audio_path, metadata_path) = self.file_manager.make_paths()?;
        // Get number of channels from metadata
        let channels = read_file_metadata(metadata_path)?.channels;

        if channels != self.output.len() as u32 {
            return Err(RecorderError::Generic(format!(
                "Cannot handle play: {channels} audio channels and {} outputs.",
                self.output.len()
            )));
        }
        // Get the audio data
        let audio_buffers = read_f32_vec_from_file(&audio_path, channels)?;
        Ok(audio_buffers)
    }

    /// Spawn a thread to send audio data to Jack.  The thread's
    /// existence defines a "playback session".  The thread will send
    /// data in real time to Jack from `self.audio_buffers`, and when
    /// finished returns [`AudioBuffers`] with the entire session's
    /// audio data
    fn start_sending_audio(
        &mut self,
    ) -> Result<thread::JoinHandle<Result<AudioBuffers, RecorderError>>, RecorderError> {
        // Get the audio data
        let audio_buffers = self.recorded_audio.clone();
        // Tested good
        // dbg!(describe_linear_buffer(audio_buffers.get_buffer_idx(0).unwrap()));
        // dbg!(describe_linear_buffer(audio_buffers.get_buffer_idx(1).unwrap()));

        let run_f = self.run_f.clone();
        let mut data_channel_port_names = Vec::with_capacity(audio_buffers.channels() as usize);

        // Need a `mpsc` channel for each audio channel to send to
        // Jack, and pair them with the audio ports
        let mut senders = vec![];
        let channels = audio_buffers.channels();
        for i in 0..channels as usize {
            let (tx, rx) = mpsc::channel::<f32>();
            senders.push(tx);
            let port_name = self.output.ports()[i].clone();
            data_channel_port_names.push((rx, port_name));
        }

        let handle = thread::spawn(move || -> Result<AudioBuffers, RecorderError> {
            let _a = send_audio_to_jack::send_audo_to_jack(data_channel_port_names, run_f.clone())?;
            run_f.store(true, Ordering::Relaxed);
            for (c, sender) in senders.iter().enumerate().take(channels as usize) {
                let buffer = audio_buffers.get_buffer_idx(c)?;
                for s in buffer.iter() {
                    if let Err(err) = sender.send(*s) {
                        return Err(RecorderError::Generic(format!(
                            "Cannot send sample {s} to jack.  {err}"
                        )));
                    }
                }
            }
            senders.clear();
            while run_f.load(Ordering::Relaxed) {
                thread::sleep(Duration::from_millis(100));
            }
            // Tested good
            // dbg!(describe_linear_buffer(audio_buffers.get_buffer_idx(0).unwrap()));
            // dbg!(describe_linear_buffer(audio_buffers.get_buffer_idx(1).unwrap()));
            Ok(audio_buffers)
        });
        Ok(handle)
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
    ) -> Result<bool, RecorderError> {
        {
            if channels_connected.iter().all(|&c| !c) {
                return Ok(false);
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
        Ok(true)
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

    /// Get the audio from the FileManager and output it through the
    /// outputs.  The file manager must know about a raw audio file
    /// and the JSON metadata There must be at least as many audio
    /// outputs specified (`-o` on command line) as there are audio
    /// channels.  Return the flag that is reset when playing is over
    /// so the caller can block
    pub fn handle_play(&mut self) -> Result<(), Box<dyn Error>> {
        self.audio_handle = match self.start_sending_audio() {
            Ok(h) => Some(h),
            Err(err) => return Err(err.into()),
        };
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
    fn handle_review_record(&self) -> Result<(), RecorderError> {
        Err(RecorderError::Unimplemented(
            "handle_review_record".to_string(),
        ))
    }

    /// Play the contents of `recorded_audio` while recording separately
    fn handle_dubing(&mut self) -> Result<(), RecorderError> {
        Err(RecorderError::Unimplemented("handle_dubing".to_string()))
    }

    /// Mix `recorded_audio` and `recorded_dub` and play it back
    fn handle_dub_review(&mut self) -> Result<(), RecorderError> {
        // Mix together `recorded_audio` and `recorded_dub` and play it back
        Err(RecorderError::Unimplemented(
            "handle_dub_review".to_string(),
        ))
    }

    fn handle_dub_accept(&mut self) -> Result<(), RecorderError> {
        Err(RecorderError::Unimplemented(
            "handle_dub_accept".to_string(),
        ))
    }

    /// When a command is passed into the programme by `-k`.  This will block until the command is complete
    pub fn handle_kommand(&mut self, k: Command) -> Result<(), Box<dyn Error>> {
        match k {
            Command::Record => {
                if !self.silent {
                    println!("<enter> to stop");
                }
                self.run_f.store(true, Ordering::Relaxed);
                self.handle_record()?;
                let mut input = String::new();
                io::stdin()
                    .read_line(&mut input)
                    .expect("Failed to read line");
                self.run_f.store(false, Ordering::Relaxed);
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
            Command::Play => {
                // This is set when the audio buffers loaded and audio
                // is ready to play
                self.run_f.store(false, Ordering::Relaxed);
                self.recorded_audio = self.get_audio_from_file()?;
                // These tested good
                // dbg!(describe_linear_buffer(self.recorded_audio.get_buffer_idx(0).unwrap()));
                // dbg!(describe_linear_buffer(self.recorded_audio.get_buffer_idx(1).unwrap()));
                self.handle_play()?;
                let h = self.audio_handle.take();

                // Wait for recording to get started
                while !self.run_f.load(Ordering::Relaxed) {
                    // FIXME: Add a time out to this incase of failure,
                    // to stop a hang
                    thread::sleep(Duration::from_millis(10));
                }

                if !self.silent {
                    // A busy loop to detect early quit by user, if
                    // not using `silent`, (<enter> or EOF from
                    // keyboard).  The thread is used because Rust
                    // does not have non-blocking IO on std library.
                    // Grrr...
                    println!("Press <enter> to stop playback");
                    let run_flag = self.run_f.clone();
                    thread::spawn(move || {
                        let mut input = String::new();
                        io::stdin()
                            // Blocks only this thread
                            .read_line(&mut input)
                            .expect("Failed to read line");
                        run_flag.store(false, Ordering::Relaxed);
                    });
                }
                match h {
                    Some(h) => {
                        while !h.is_finished() {
                            thread::sleep(Duration::from_millis(10));
                        }
                        if let Err(err) = h.join() {
                            Err(
                                format!("Error qzn3t/recorder: Failed sending audio: {err:?}")
                                    .into(),
                            )
                        } else {
                            Ok(())
                        }
                    }
                    None => panic!("This cannot happen!!"),
                }
            }
            _ => panic!("Error recorder: -k {k:?} is not handled"),
        }
    }
}
#[cfg(test)]
mod tests {

    use jack::PortFlags;

    use super::*;
    use crate::io::{AudioBuffers, JackPipes};
    use crate::test_utils::common::{
        WaveForm, generate_test_audio, play_test_audio, setup_test_dir,
    };
    use std::sync::mpsc;

    #[test]
    fn test_audio_buffers_operations() {
        let mut buffers = AudioBuffers::new();

        // Test adding buffers
        assert!(buffers.add_buffer(vec![1.0, 2.0, 3.0]).is_ok());
        assert!(buffers.add_buffer(vec![4.0, 5.0, 6.0]).is_ok());

        assert_eq!(buffers.channels(), 2);

        // Test getting buffer
        let buffer = buffers.get_buffer_idx(0).unwrap();
        assert_eq!(buffer.len(), 3);
        assert_eq!(buffer[0], 1.0);

        // Test reset
        buffers.reset();
        assert!(
            buffers
                .names()
                .iter()
                .all(|n| buffers.get_named_buffer(n).unwrap().is_empty())
        );
    }

    #[test]
    fn test_inner_audio_jack_loop_all_disconnected() {
        let mut channels_connected = vec![false, false];
        let (tx1, rx1) = mpsc::channel();
        let (tx2, rx2) = mpsc::channel();
        let audio_rxs = vec![rx1, rx2];
        let (fm_tx1, _fm_rx1) = mpsc::channel();
        let (fm_tx2, _fm_rx2) = mpsc::channel();
        let fm_tx = vec![fm_tx1, fm_tx2];
        let mut audio_buffers = AudioBuffers::new();
        audio_buffers.add_buffer(vec![]).unwrap();
        audio_buffers.add_buffer(vec![]).unwrap();

        let result = AppData::inner_audio_jack_loop(
            &mut channels_connected,
            &audio_rxs,
            &fm_tx,
            &mut audio_buffers,
            2,
        );

        assert!(result.is_ok());
        assert!(!result.unwrap()); // Should return false when all disconnected

        drop(tx1);
        drop(tx2);
    }

    #[test]
    fn test_inner_audio_jack_loop_with_data() {
        let mut channels_connected = vec![true, true];
        let (tx1, rx1) = mpsc::channel();
        let (tx2, rx2) = mpsc::channel();
        let audio_rxs = vec![rx1, rx2];
        let (fm_tx1, fm_rx1) = mpsc::channel();
        let (fm_tx2, fm_rx2) = mpsc::channel();
        let fm_tx = vec![fm_tx1, fm_tx2];
        let mut audio_buffers = AudioBuffers::new();
        audio_buffers.add_buffer(vec![]).unwrap();
        audio_buffers.add_buffer(vec![]).unwrap();

        // Send some test data
        tx1.send(1.0).unwrap();
        tx1.send(2.0).unwrap();
        tx2.send(3.0).unwrap();

        let result = AppData::inner_audio_jack_loop(
            &mut channels_connected,
            &audio_rxs,
            &fm_tx,
            &mut audio_buffers,
            2,
        );

        assert!(result.is_ok());
        assert!(result.unwrap()); // Should continue

        // Verify data was received by file manager
        assert_eq!(fm_rx1.try_recv().unwrap(), 1.0);
        assert_eq!(fm_rx1.try_recv().unwrap(), 2.0);
        assert_eq!(fm_rx2.try_recv().unwrap(), 3.0);

        // Verify data was added to buffers
        assert_eq!(audio_buffers.get_buffer_idx(0).unwrap().len(), 2);
        assert_eq!(audio_buffers.get_buffer_idx(1).unwrap().len(), 1);

        drop(tx1);
        drop(tx2);
    }

    #[test]
    fn test_clone_audio_buffers() {
        let mut buffers = AudioBuffers::new();
        buffers.add_buffer(vec![1.0, 2.0, 3.0]).unwrap();

        let cloned = buffers.clone();
        assert_eq!(cloned.channels(), 1);
        assert_eq!(
            cloned.get_buffer_idx(0).unwrap(),
            buffers.get_buffer_idx(0).unwrap()
        );
    }

    // Below are tests that require a jack client running with the
    // pipes that `create_jack_pipes` needs to use.  FIXME: (1) Make
    // that client.  (2) Ensure the pipes being created are all pipes
    // of it (3) Rename `create_jack_pipes` to drop the word "create"
    // as it attaches
    #[test]
    fn test_handle_audio_stop() {
        let temp_dir = setup_test_dir();
        let inputs = JackPipes::new(true);
        let outputs = JackPipes::new(false);

        let mut app_data = App::initialise(inputs, outputs, temp_dir.path(), true).unwrap();

        // Set run flag to true
        app_data.run_f.store(true, Ordering::Relaxed);

        let result = app_data.handle_audio_stop();
        assert!(result.is_ok());
        assert!(!app_data.run_f.load(Ordering::Relaxed));
    }

    #[test]
    fn test_check_audio_file_manager() {
        let temp_dir = setup_test_dir();
        let inputs = JackPipes::new(true);
        let outputs = JackPipes::new(false);

        let mut app_data = App::initialise(inputs, outputs, temp_dir.path(), true).unwrap();

        let result = app_data.check_audio_file_manager();
        assert!(result.is_ok());
    }

    #[test]
    //#[ignore] // Adding pipes to `JackPipes` involves validatng them with a Jack client
    fn test_jack_pipes_add() {
        let ports = vec!["port1", "port2"];
        let length_audio = 100u32; // MS
        let buf1 = generate_test_audio(110, 0.42, length_audio, WaveForm::Square);
        let buf2 = generate_test_audio(100, 0.82, length_audio, WaveForm::Triangle);
        let (ac, _flag, _, _) =
            play_test_audio("test_jack_pipes_add", ports.clone(), vec![&buf1, &buf2]);
        let ports = ac
            .as_client()
            .ports(Some("port[12]"), None, PortFlags::empty());
        let mut jack_pipes = JackPipes::new(true);
        for p in ports.iter() {
            if let Err(err) = jack_pipes.add(p) {
                panic!("{err}");
            }
        }
        let ports_test = jack_pipes.ports();

        assert_eq!(ports_test.len(), 2);
        assert_eq!(ports_test, ports);
    }

    #[test]
    #[should_panic(expected = "Error recorder: -k Continue is not handled")]
    fn test_handle_kommand_unimplemented() {
        let temp_dir = setup_test_dir();
        let inputs = JackPipes::new(true);
        let outputs = JackPipes::new(false);

        let mut app_data = App::initialise(inputs, outputs, temp_dir.path(), true).unwrap();

        // This should panic for unimplemented commands
        let _ = app_data.handle_kommand(Command::Continue);
    }

    #[test]
    fn test_multiple_instances() {
        // Test that multiple initializations don't panic
        let temp_dir = setup_test_dir();

        let inputs1 = JackPipes::new(true);
        let outputs1 = JackPipes::new(false);
        let inputs2 = JackPipes::new(true);
        let outputs2 = JackPipes::new(false);

        let result1 = App::initialise(inputs1, outputs1, temp_dir.path(), true);
        let result2 = App::initialise(inputs2, outputs2, temp_dir.path(), true);

        assert!(result1.is_ok());
        assert!(result2.is_ok());
    }

    #[test]
    fn test_run_ui_quit_command() {
        let temp_dir = setup_test_dir();
        let (tx, rx) = mpsc::channel();
        let inputs = JackPipes::new(true);
        let outputs = JackPipes::new(false);

        let app_data = App::initialise_ui(rx, inputs, outputs, temp_dir.path(), true).unwrap();

        let handle = App::run_ui(app_data).unwrap();

        // Send quit command
        tx.send(Command::Quit).unwrap();

        // Wait for thread to finish
        let result = handle.join();
        assert!(result.is_ok());
        assert!(result.unwrap().is_ok());
    }

    #[test]
    fn test_run_ui_stop_command() {
        let temp_dir = setup_test_dir();
        let (tx, rx) = mpsc::channel();
        let inputs = JackPipes::new(true);
        let outputs = JackPipes::new(false);

        let app_data = App::initialise_ui(rx, inputs, outputs, temp_dir.path(), true).unwrap();

        let handle = App::run_ui(app_data).unwrap();

        // Send stop then quit
        tx.send(Command::Stop).unwrap();
        thread::sleep(Duration::from_millis(50));
        tx.send(Command::Quit).unwrap();

        let result = handle.join();
        assert!(result.is_ok());
    }

    #[test]
    fn test_run_ui_continue_command() {
        let temp_dir = setup_test_dir();
        let (tx, rx) = mpsc::channel();
        let inputs = JackPipes::new(true);
        let outputs = JackPipes::new(false);

        let app_data = App::initialise_ui(rx, inputs, outputs, temp_dir.path(), true).unwrap();

        let handle = App::run_ui(app_data).unwrap();

        // Send continue then quit
        tx.send(Command::Continue).unwrap();
        thread::sleep(Duration::from_millis(50));
        if let Err(err) = tx.send(Command::Continue) {
            panic!("Cannot send command: {err}");
        }
        if let Err(err) = tx.send(Command::Quit) {
            panic!("Cannot send command: {err}");
        }

        let result = handle.join();
        assert!(result.is_ok());
    }

    #[test]
    fn test_command_channel_disconnect() {
        let temp_dir = setup_test_dir();
        let inputs = JackPipes::new(true);
        let outputs = JackPipes::new(false);
        let (command_tx, command_rx) = mpsc::channel::<Command>();
        let app_data =
            App::initialise_ui(command_rx, inputs, outputs, temp_dir.path(), true).unwrap();
        let handle = App::run_ui(app_data).unwrap();
        // Drop sender to disconnect
        drop(command_tx);
        let result = handle.join().unwrap();

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), RecorderError::Generic(_)));
    }

    #[test]
    fn test_run_ui_no_command_receiver() {
        let temp_dir = setup_test_dir();
        let inputs = JackPipes::new(true);
        let outputs = JackPipes::new(false);

        let mut app_data = App::initialise(inputs, outputs, temp_dir.path(), true).unwrap();

        // Remove command_rx
        app_data.command_rx = None;

        let handle = App::run_ui(app_data).unwrap();
        let result = handle.join().unwrap();

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), RecorderError::Generic(_)));
    }
    #[test]
    fn test_app_initialise() {
        let temp_dir = setup_test_dir();
        let inputs = JackPipes::new(true);
        let outputs = JackPipes::new(false);

        let result = App::initialise(inputs, outputs, temp_dir.path(), true);

        assert!(result.is_ok());
        let app_data = result.unwrap();
        assert_eq!(app_data.recorded_audio.channels(), 0);
        assert!(app_data.audio_handle.is_none());
    }

    #[test]
    fn test_app_initialise_ui() {
        let temp_dir = setup_test_dir();
        let (tx, rx) = mpsc::channel();
        let inputs = JackPipes::new(true);
        let outputs = JackPipes::new(false);

        let result = App::initialise_ui(rx, inputs, outputs, temp_dir.path(), true);

        assert!(result.is_ok());
        let app_data = result.unwrap();
        assert!(app_data.command_rx.is_some());
        drop(tx); // Clean up
    }

    #[test]
    fn test_app_data_quit() {
        let temp_dir = setup_test_dir();
        let inputs = JackPipes::new(true);
        let outputs = JackPipes::new(false);

        let mut app_data = App::initialise(inputs, outputs, temp_dir.path(), true).unwrap();

        app_data.quit();
        assert!(!app_data.ui_run_f.load(Ordering::Relaxed));
    }

    // Test the unimplemented methods to get full test coverage
    #[test]
    fn test_unimplemented_methods() {
        let temp_dir = setup_test_dir();
        let (tx, rx) = mpsc::channel();
        let inputs = JackPipes::new(true);
        let outputs = JackPipes::new(false);

        let app_data = App::initialise_ui(rx, inputs, outputs, temp_dir.path(), true).unwrap();
        let handle = App::run_ui(app_data).unwrap();
        tx.send(Command::DubAccept).unwrap();
        let result = handle.join().unwrap();
        assert!(result.is_err());
        assert_eq!(
            result,
            Err(RecorderError::Unimplemented(
                "handle_dub_accept".to_string()
            ))
        );

        let temp_dir = setup_test_dir();
        let (tx, rx) = mpsc::channel();
        let inputs = JackPipes::new(true);
        let outputs = JackPipes::new(false);
        let app_data = App::initialise_ui(rx, inputs, outputs, temp_dir.path(), true).unwrap();
        let handle = App::run_ui(app_data).unwrap();
        tx.send(Command::ReviewRecord).unwrap();
        let result = handle.join().unwrap();
        assert!(result.is_err());
        assert_eq!(
            result,
            Err(RecorderError::Unimplemented(
                "handle_review_record".to_string()
            ))
        );

        let temp_dir = setup_test_dir();
        let (tx, rx) = mpsc::channel();
        let inputs = JackPipes::new(true);
        let outputs = JackPipes::new(false);
        let app_data = App::initialise_ui(rx, inputs, outputs, temp_dir.path(), true).unwrap();
        let handle = App::run_ui(app_data).unwrap();
        tx.send(Command::Dubing).unwrap();
        let result = handle.join().unwrap();
        assert!(result.is_err());
        assert_eq!(
            result,
            Err(RecorderError::Unimplemented("handle_dubing".to_string()))
        );

        let temp_dir = setup_test_dir();
        let (tx, rx) = mpsc::channel();
        let inputs = JackPipes::new(true);
        let outputs = JackPipes::new(false);
        let app_data = App::initialise_ui(rx, inputs, outputs, temp_dir.path(), true).unwrap();
        let handle = App::run_ui(app_data).unwrap();
        tx.send(Command::DubReview).unwrap();
        let result = handle.join().unwrap();
        assert!(result.is_err());
        assert_eq!(
            result,
            Err(RecorderError::Unimplemented(
                "handle_dub_review".to_string()
            ))
        );
    }
}
