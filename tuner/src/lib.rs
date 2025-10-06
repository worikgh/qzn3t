// Copyright (c) 2025 Worik Turei Stanton
// License: GPL-3.0

use clap::Parser;
use jack::{AudioIn, Client, Port, ProcessHandler, ProcessScope};
#[allow(unused_imports)]
use pitch_detector::{
    core::NoteName,
    note::{NoteDetectionResult, detect_note as abc_detect_note},
    pitch::HannedFftDetector,
    pitch::PowerCepstrum,
};
use ringbuf::HeapRb;
use ringbuf::traits::{Observer, consumer::Consumer, producer::Producer};
use std::sync::mpsc;
use std::{
    io::{self},
    thread::JoinHandle,
};

use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use std::{error::Error, io::Write};

#[derive(Debug, Clone, Eq, PartialEq, PartialOrd, Ord)]
pub enum TunerNote {
    A,
    ASharp,
    B,
    C,
    CSharp,
    D,
    DSharp,
    E,
    F,
    FSharp,
    G,
    GSharp,
}

impl From<NoteName> for TunerNote {
    fn from(n: NoteName) -> Self {
        match n {
            NoteName::A => TunerNote::A,
            NoteName::ASharp => TunerNote::ASharp,
            NoteName::B => TunerNote::B,
            NoteName::C => TunerNote::C,
            NoteName::CSharp => TunerNote::CSharp,
            NoteName::D => TunerNote::D,
            NoteName::DSharp => TunerNote::DSharp,
            NoteName::E => TunerNote::E,
            NoteName::F => TunerNote::F,
            NoteName::FSharp => TunerNote::FSharp,
            NoteName::G => TunerNote::G,
            NoteName::GSharp => TunerNote::GSharp,
        }
    }
}
/// Data to return from tuner::get_results
#[derive(Debug)]
pub struct TunerData {
    pub note: TunerNote,
    pub octave: i32,
    pub cents_offset: f64,
}

// Custom ProcessHandler for capturing audio using ringbuf
struct TunerProcessHandler {
    capture_port: Port<AudioIn>,
    ring_buffer: Arc<Mutex<HeapRb<f32>>>,
}

impl ProcessHandler for TunerProcessHandler {
    fn process(&mut self, _: &Client, ps: &ProcessScope) -> jack::Control {
        let buffer = self.capture_port.as_slice(ps);

        // Push all available samples to the ring buffer
        let mut rb_guard = self.ring_buffer.lock().unwrap();
        for &sample in buffer {
            rb_guard.try_push(sample).unwrap();
        }

        jack::Control::Continue
    }
}

fn detect_note(signal: &[f64], sample_rate: usize) -> Result<NoteDetectionResult, Box<dyn Error>> {
    let sample_rate = sample_rate as f64;
    let mut detector = HannedFftDetector::default();
    let note = abc_detect_note(signal, &mut detector, sample_rate);
    if let Some(note) = note {
        Ok(note)
    } else {
        Err("Failed".into())
    }
}

struct Notifications;

impl jack::NotificationHandler for Notifications {}

#[derive(Parser)]
#[command(version, about, long_about = None)]
pub struct TunerArgs {
    #[arg(short, long, default_value_t = 200)]
    pub interval: u64, // MS between sample times
    #[arg(short, long, default_value_t = 2_048_000)]
    pub count: u64, // The number of samples in a tone to check
    #[arg(short, long, default_value_t = 0.0)]
    pub max_vol_min: f64, // The maximum volume must be bigger than this
    #[arg(short = 'n', long, default_value_t = 1.0)]
    pub mean_min: f64, // The absolute mean volume must be smaller than this
}

pub fn start_jack_thread(args: &TunerArgs) -> (mpsc::Receiver<Vec<f32>>, usize, JoinHandle<()>) {
    let (sender, receiver) = mpsc::channel();
    let (client, _status) =
        jack::Client::new("qzn3t_tuner", jack::ClientOptions::NO_START_SERVER).unwrap();
    let sample_rate = client.sample_rate();

    let interval_ms = args.interval;
    let buffer_size = args.count as usize;

    let jh = thread::spawn(move || {
        // Create ring buffer with specified capacity
        let ring_buffer = Arc::new(Mutex::new(HeapRb::<f32>::new(buffer_size)));
        let ring_buffer_clone = Arc::clone(&ring_buffer);

        // Register capture port
        let capture_port = client.register_port("input", AudioIn::default()).unwrap();

        // Activate the client with our custom handler
        let handler = TunerProcessHandler {
            capture_port,
            ring_buffer: ring_buffer_clone,
        };

        let _active_client = client.activate_async(Notifications, handler).unwrap();

        let mut sleep_ms = interval_ms;
        loop {
            let sample_interval = Duration::from_millis(sleep_ms);
            thread::sleep(sample_interval);
            let now = Instant::now();

            // Get available samples from the ring buffer
            let mut rb_guard = ring_buffer.lock().unwrap();
            let available = (*rb_guard).occupied_len();
            // if buffer_size != available {
            //	eprintln!(
            //	    "DBG tuner: Available: {available} != buffer_size: {buffer_size}.  Vacant length: {}",
            //	    (*rb_guard).vacant_len()
            //	);
            // }

            let mut samples = Vec::with_capacity(available);
            while let Some(sample) = (*rb_guard).try_pop() {
                samples.push(sample);
            }

            drop(rb_guard);

            if !samples.is_empty() {
                match sender.send(samples) {
                    Ok(()) => (),
                    Err(err) => {
                        eprintln!("Error tuner: Send error in jack thread: {err}");
                        break;
                    }
                };
            }
            let elapsed_ms = now.elapsed().as_millis();
            sleep_ms = if elapsed_ms > interval_ms.into() {
                eprintln!("Error tuner: xrun {} ms", elapsed_ms - interval_ms as u128);
                0
            } else {
                (interval_ms as u128 - elapsed_ms) as u64
            };
        }
        eprintln!("DBG tuner: Loop in Jack thread ended");
    });

    (receiver, sample_rate, jh)
}

pub fn get_results(args: &TunerArgs, sender: mpsc::Sender<TunerData>) -> JoinHandle<()> {
    let (receiver, sample_rate, jh) = start_jack_thread(args);
    let max_vol_min = args.max_vol_min;
    let mean_min = args.mean_min;
    thread::spawn(move || {
        loop {
            let v = match receiver.recv() {
                Ok(v) => v,
                Err(err) => {
                    eprintln!("Error tuner: Receive error in main thread: {err}");
                    break;
                }
            };

            // Skip processing if we don't have enough samples
            if v.len() < 1024 {
                // Minimum reasonable sample size for pitch detection
                continue;
            }
            let max = v.iter().copied().fold(f32::NEG_INFINITY, f32::max);
            let _min = v.iter().copied().fold(f32::INFINITY, f32::min);
            let mean = v.iter().sum::<f32>() / v.len() as f32;

            if (max as f64) < max_vol_min {
                continue;
            }

            // This is odd.  Seems to be necessary
            if (mean.abs() as f64) > mean_min {
                continue;
            }

            let v: Vec<f64> = v.iter().map(|&x| x as f64).collect();

            let note_result = match detect_note(&v, sample_rate) {
                Ok(r) => r,
                Err(_err) => {
                    continue;
                }
            };

            let note = note_result.note_name;
            let octave = note_result.octave;
            let cents = note_result.cents_offset;

            let tuner_data = TunerData {
                octave,
                cents_offset: cents,
                note: TunerNote::from(note),
            };
            sender.send(tuner_data).unwrap();
        }
    });
    jh
}

pub fn inner_main(args: &TunerArgs) {
    let (sender, receiver) = mpsc::channel::<TunerData>();
    _ = get_results(args, sender);
    loop {
        let tuner_data = match receiver.recv() {
            Ok(r) => r,
            Err(err) => {
                eprintln!("DBG tuner: get_results send error: {err}");
                break;
            }
        };
        let report = format!(
            "Tuner> {:?}/{} {:0.2}\n",
            tuner_data.note, tuner_data.octave, tuner_data.cents_offset
        );
        if let Err(err) = io::stdout().lock().write_all(report.as_bytes()) {
            eprintln!("Error tuner: IO error on write_all: {err}");
            break;
        }
        if let Err(err) = io::stdout().lock().flush() {
            eprintln!("Error tuner: IO error on flush: {err}");
            break;
        }
    }
}
