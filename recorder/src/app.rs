// Copyright (c) 2025 Worik Turei Stanton
// License: GPL-3.0

//! The main container for the programme
use crate::errors::RecorderError;
use crate::io::{AudioBuffers, FileManager, JackPipes};
use crate::structs::Command;
use crate::utils::get_sample_rate;
use jack_rec;
use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Once;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::TryRecvError;
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::Duration;

static ONCE: Once = Once::new();

/// Hold the code that runs the programme.
pub struct App;
impl App {
    /// Start a thread that waits for commands.  Return the handle
    pub fn run(
        &mut self,
        mut config_app: AppData,
    ) -> Result<thread::JoinHandle<Result<(), RecorderError>>, Box<dyn Error>> {
        // The version of `self` used inside the loop
        let app_handle = thread::spawn(move || -> Result<(), RecorderError> {
            // Main loop frequency
            let poll = Duration::from_millis(10);

            loop {
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
                    match command {
                        Command::Record => config_app.handle_record()?,
                        Command::Stop => config_app.handle_audio_stop()?,
                        Command::ReviewRecord => config_app.handle_review_record()?,
                        Command::Dubing => config_app.handle_dubing()?,
                        Command::DubReview => config_app.handle_dub_review()?,
                        Command::DubAccept => config_app.handle_dub_accept()?,
                        Command::Continue => (),
                        Command::Quit => {
                            config_app.quit();
                            break;
                        }
                    }
                }
                if !config_app.check_file_manager()? {
                    panic!("File manager has shut down");
                }
                thread::sleep(poll);
            }
            eprintln!("DBG recorder: Broken from main loop");
            Ok(())
        }); // Closure
        Ok(app_handle)
    }

    /// Set up AppData
    pub fn initialise(
        &mut self,
        audio_tx: mpsc::Sender<f32>,
        command_rx: mpsc::Receiver<Command>,
        inputs: JackPipes,
        outputs: JackPipes,
        directory: &PathBuf,
    ) -> Result<AppData, Box<dyn Error>> {
        // Flag to start and stop the recorder
        let run_f = Arc::new(AtomicBool::new(true));
        let ui_run_f = Arc::new(AtomicBool::new(true));
        let run_f_ctl_c = run_f.clone();

        // Ctl-c handlers must only be set once.  Not a problem for
        // normal use, but tests are often run in parallel, so this is done for testing
        ONCE.call_once(|| {
            if let Err(err) = ctrlc::set_handler(move || {
                run_f_ctl_c.store(false, Ordering::Relaxed);
            }) {
                panic!("Error qzn3t/recorder: setting Ctrl-C handler: {err}");
            }
        });

        let file_manager = FileManager::new(inputs.names(), directory)?;
        Ok(AppData {
            recorded_audio: AudioBuffers::new(),
            audio_handle: None,
            audio_tx: vec![audio_tx],
            command_rx,
            run_f: run_f.clone(),
            ui_run_f,
            inputs,
            outputs,
            save_dir: directory.clone(),
            file_manager,
        })
    }

    /// Read audio data from a file into a buffer
    pub fn read_f32_vec_from_file(file_path: &PathBuf) -> Result<Vec<f32>, RecorderError> {
        // Step 1: Read the file into a Vec<u8>
        let data = fs::read(file_path).map_err(|err| RecorderError::Generic(err.to_string()))?;

        // Step 2: Convert Vec<u8> to Vec<f32>
        if data.len() % std::mem::size_of::<f32>() != 0 {
            return Err(RecorderError::Generic(
                "File size is not a multiple of f32 size".to_string(),
            ));
        }

        // Create a Vec<f32> from the Vec<u8>
        let float_count = data.len() / std::mem::size_of::<f32>();
        let float_vec: Vec<f32> = unsafe {
            std::slice::from_raw_parts(data.as_ptr() as *const f32, float_count).to_vec()
        };

        Ok(float_vec)
    }

    /// `file` is open for, and ready to, append Write the contents of
    /// `buffer` to `file` as binary data Return the number of bytes
    /// written to the file
    #[allow(clippy::manual_slice_size_calculation)]
    pub fn write_f32_to_file(file: &mut fs::File, buffer: &[f32]) -> Result<usize, RecorderError> {
        let sz_f32 = std::mem::size_of::<f32>();
        let bytes = unsafe {
            std::slice::from_raw_parts(
                buffer.as_ptr() as *const u8,
                buffer.len() * std::mem::size_of::<f32>(),
            )
        };
        assert_eq!(buffer.len() * sz_f32, bytes.len());
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
    audio_tx: Vec<mpsc::Sender<f32>>,
    command_rx: mpsc::Receiver<Command>,
    pub run_f: Arc<AtomicBool>,
    pub ui_run_f: Arc<AtomicBool>,
    save_dir: PathBuf,
    inputs: JackPipes,
    outputs: JackPipes,
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

    /// Spawn a thread to get audio data from a Jack port.  Return the handle
    fn get_audio_from_jack(
        &mut self,
    ) -> Result<thread::JoinHandle<Result<AudioBuffers, RecorderError>>, RecorderError> {
        // Copy of the switch to turn the recorder off
        let run_f = self.run_f.clone();

        // Local switch that is set when recorder is ready and recording
        let active_1 = Arc::new(AtomicBool::new(false));
        let active_2 = active_1.clone();

        let jack_ports = self.inputs.named_ports();
        // Start up the file manager for saving recorded audio.  Rtuns
        // a Hash of name => tx for saving data
        let fm_tx = self.file_manager.start()?;

        let result = Ok(thread::spawn(
            move || -> Result<AudioBuffers, RecorderError> {
                // Buffer and channel to get data on

                let mut audio_data = AudioBuffers::new();
                let mut audio_input_channels = HashMap::new();
                let mut buf_txs = Vec::new();
                let mut inputs = Vec::new();
                for np in jack_ports.iter() {
                    let name = np.0.as_str();
                    let input = np.1.clone();
                    inputs.push(input);
                    audio_data.add_buffer(name, Vec::new())?;

                    // Channels for moving audio data from Jack into this programme
                    let (buf_tx, buf_rx) = mpsc::channel::<f32>();
                    audio_input_channels.insert(name.to_string(), buf_rx);
                    buf_txs.push(buf_tx);
                }

                let client = "qzn3t/recorder".to_string();
                let ac = match jack_rec::read_port(client, inputs, buf_txs, run_f.clone()) {
                    Ok(p) => p,
                    Err(err) => {
                        return Err(err.into());
                    }
                };

                // Signal that this is running to caller (parent)
                active_2.store(true, Ordering::SeqCst);

                // Keep track of disconnected channelss and when they are
                // all disconnected exit the loop normally
                let mut channels_connected: HashMap<String, bool> = HashMap::new();
                for k in audio_input_channels.keys() {
                    channels_connected.insert(k.to_string(), true);
                }

                // Main loop getting data from Jack
                loop {
                    match Self::inner_audio_jack_loop(
                        &mut channels_connected,
                        &audio_input_channels,
                        &fm_tx,
                        &mut audio_data,
                    ) {
                        Ok(ctl) => match ctl {
                            InnerJackLoopCtl::Continue => (),
                            InnerJackLoopCtl::Quit => break,
                        },
                        Err(err) => return Err(err),
                    }
                    thread::sleep(Duration::from_millis(10));
                }

                if let Err(err) = ac.deactivate() {
                    Err(RecorderError::DeactivateClientFailed(format!("{err}")))
                } else {
                    Ok(audio_data)
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
    /// be saved.
    /// `channels_connected` is indexed by the channel names,
    /// and the value is reset when the audio channel to that Jack
    /// source is disconnected.
    fn inner_audio_jack_loop(
        channels_connected: &mut HashMap<String, bool>,
        audio_input_channels: &HashMap<String, mpsc::Receiver<f32>>,
        fm_tx: &HashMap<String, mpsc::Sender<f32>>,
        audio_data: &mut AudioBuffers,
    ) -> Result<InnerJackLoopCtl, RecorderError> {
        {
            if channels_connected.iter().all(|(_, c)| !c) {
                return Ok(InnerJackLoopCtl::Quit);
            }

            // Debugging code.  Should never see this message.
            // Channels should be disconnected all in the same moment
            if channels_connected.iter().any(|(_, c)| !c) {
                eprintln!(
                    "DBG: !!! Only some channels disconnected, should not happen: {}",
                    channels_connected
                        .iter()
                        .fold("".to_string(), |a, (n, v)| format!("{a} {n}:{v}"))
                );
            }

            for (name, receiver) in audio_input_channels.iter() {
                if channels_connected.iter().all(|(_, c)| !c) {
                    break;
                }
                let tx = match fm_tx.get(name) {
                    Some(tx) => tx,
                    None => {
                        return Err(RecorderError::FileManager(format!(
                            "No Sender for sending data to FileManager.  For: {name}"
                        )));
                    }
                };

                loop {
                    let buffer: &mut Vec<f32> = audio_data.get_mut(name).unwrap();
                    match receiver.try_recv() {
                        Ok(n) => {
                            buffer.push(n);
                            if let Err(err) = tx.send(n) {
                                return Err(RecorderError::FileManager(format!(
                                    "Error sending data to FileManager: {err}"
                                )));
                            }
                        }
                        Err(TryRecvError::Disconnected) => {
                            channels_connected.insert(name.clone(), false);
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
    pub fn check_file_manager(&mut self) -> Result<bool, RecorderError> {
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
        self.recorded_audio.reset();

        self.run_f.store(true, Ordering::SeqCst);

        match self.get_audio_from_jack() {
            Ok(handle) => {
                self.audio_handle = Some(handle);
            }
            Err(err) => {
                return Err(err.into());
            }
        };
        Ok(())
    }

    /// The command: stop
    pub fn handle_audio_stop(&mut self) -> Result<(), Box<dyn Error>> {
        // This ends the main loop
        self.run_f.store(false, Ordering::Relaxed);

        // Allow all the thrads to stop
        thread::sleep(Duration::from_millis(100));

        if let Some(handle) = self.audio_handle.take() {
            assert!(handle.is_finished());
            let j = handle.join();
            match j {
                Ok(Ok(mut audio_data)) => {
                    for (n, b) in audio_data.iter_mut() {
                        self.recorded_audio.add_buffer(n, b.to_vec())?;
                    }
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
    fn handle_review_record(&mut self) -> Result<(), Box<dyn Error>> {
        self.run_f.store(true, Ordering::Relaxed);
        // This is possibly quite a big copy.  FIXME: There must be a
        // better way
        self.play_audio(self.recorded_audio.clone())
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

    /// Save the audio from the `recorded_audio`
    pub fn handle_save(&mut self) -> Result<(), RecorderError> {
        for (name, audio_data) in self.recorded_audio.iter() {
            let data = unsafe {
                std::slice::from_raw_parts(
                    audio_data.as_ptr() as *const u8,
                    audio_data.len() * std::mem::size_of::<f32>(),
                )
            }
            .to_vec();

            // `self.save_dir` is not `None`
            let dest: PathBuf = self.save_dir.join(name);
            fs::write(dest, &data).map_err(|err| RecorderError::Generic(err.to_string()))?;
        }
        Ok(())
    }

    /// When a command is passed into the programme by `-k`
    pub fn handle_kommand(&mut self, k: Command) -> Result<(), Box<dyn Error>> {
        match k {
            Command::Record => {
                println!("Recording.  C-c to stop");
                self.handle_record()?;
                if let Some(h) = self.audio_handle.take() {
                    match h.join() {
                        Ok(Ok(data)) => {
                            self.recorded_audio = data;
                            self.handle_save()?;
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
            _ => panic!("Error recorder: -k {k:?} is not handled"),
        }
    }

    /// Send `recorded_audio` to the backend to play.
    fn play_audio(&self, audio: AudioBuffers) -> Result<(), Box<dyn Error>> {
        let tx = self.audio_tx.clone();

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
                for (j, d) in i.1.iter().enumerate() {
                    if let Err(e) = tx[j].send(*d) {
                        // TODO: This should be an error
                        eprintln!(
                            "recorder Error sending data in play_audio {e}
"
                        );
                        break;
                    }
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
