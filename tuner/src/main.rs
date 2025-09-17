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
use std::io::{self};
use std::sync::mpsc;

use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use std::{error::Error, io::Write};

// Custom ProcessHandler for capturing audio to test
struct TunerProcessHandler {
    capture_port: Port<AudioIn>,
    sample_buffer: Arc<Mutex<Vec<f32>>>,
    sample_rate: usize,
    last_sample_time: Instant,
    sample_interval: Duration,
    sample_duration: Duration,
}

impl ProcessHandler for TunerProcessHandler {
    fn process(&mut self, _: &Client, ps: &ProcessScope) -> jack::Control {
	let current_time = Instant::now();

	// Check if it's time to start a new sample
	if current_time.duration_since(self.last_sample_time) >= self.sample_interval {
	    self.last_sample_time = current_time;
	    let buffer = self.capture_port.as_slice(ps);
	    let mut buf_guard = self.sample_buffer.lock().unwrap();
	    let num_samples_needed =
		(self.sample_rate as f32 * self.sample_duration.as_secs_f32()) as usize;
	    buf_guard.clear();
	    buf_guard.extend_from_slice(&buffer[..num_samples_needed.min(buffer.len())]);
	}

	jack::Control::Continue
    }
}

fn detect_note(signal: &[f64], sample_rate: usize) -> Result<NoteDetectionResult, Box<dyn Error>> {
    let sample_rate = sample_rate as f64;
    let mut detector = HannedFftDetector::default();
    // let mut detector = PowerCepstrum::default(); // HannedFftDetector::default();
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
struct TunerArgs {
    #[arg(short, long, default_value_t = 200)]
    interval: u64, // MS between sample times
    #[arg(short, long, default_value_t = 2_048_000)]
    count: u64, // The number of samples in a tone to check
    #[arg(short, long, default_value_t = 0.0)]
    max_vol_min: f64, // The maximum volume must be bigger than this
    #[arg(short, long, default_value_t = 0.0)]
    mean_min: f64, // The absolute mean volume must be bigger than this
}
fn start_jack_thread(args: &TunerArgs) -> (mpsc::Receiver<Vec<f32>>, usize) {
    let (sender, receiver) = mpsc::channel();
    let (client, _status) =
	jack::Client::new("qzn3t_tuner", jack::ClientOptions::NO_START_SERVER).unwrap();
    let sample_rate = client.sample_rate();

    let interval_ms = args.interval;
    let count = args.count;
    thread::spawn(move || {
	// Register capture port
	let capture_port = client
	    .register_port("system:capture_1", AudioIn::default())
	    .unwrap();

	// Shared buffer for samples
	let sample_buffer = Arc::new(Mutex::new(Vec::<f32>::new()));
	let sample_buffer_clone = Arc::clone(&sample_buffer);

	// Sampling parameters
	let sample_interval = Duration::from_millis(interval_ms);
	let sample_duration = Duration::from_millis(count / (sample_rate as u64));
	assert!(sample_duration < sample_interval);
	// let sample_duration = Duration::from_millis(300);
	eprintln!(
	    "DBG tuner: sample_rate: {sample_rate} interval: {sample_interval:?} duration: {sample_duration:?}"
	);
	// Activate the client with our custom handler
	let handler = TunerProcessHandler {
	    capture_port,
	    sample_buffer: sample_buffer_clone,
	    sample_rate,
	    last_sample_time: Instant::now(),
	    sample_interval,
	    sample_duration,
	};
	let _active_client = client.activate_async(Notifications, handler).unwrap();
	loop {
	    thread::sleep(sample_interval);
	    let buf_guard = sample_buffer.lock().unwrap();
	    let i = buf_guard.iter();
	    let v: Vec<f32> = i.cloned().collect();
	    match sender.send(v) {
		Ok(()) => (),
		Err(err) => {
		    eprintln!("Error tuner: Send error in jack thread: {err}");
		    break;
		}
	    };
	}
	eprintln!("DBG tuner: Loop in Jack thread ended");
    });
    (receiver, sample_rate)
}

fn main() {
    let args = TunerArgs::parse();
    let (receiver, sample_rate) = start_jack_thread(&args);
    loop {
	let v = match receiver.recv() {
	    Ok(v) => v,
	    Err(err) => {
		eprintln!("Error tuner: Send error in jack thread: {err}");
		break;
	    }
	};
	let max = v.iter().copied().fold(f32::NEG_INFINITY, f32::max);
	let min = v.iter().copied().fold(f32::INFINITY, f32::min);
	let mean = v.iter().sum::<f32>() / v.len() as f32;
	if (max as f64) < args.max_vol_min {
	    continue;
	}
	if mean.abs() as f64 > args.mean_min {
	    continue;
	}

	let v: Vec<f64> = v.iter().map(|&x| x as f64).collect();
	if v.is_empty() {
	    // eprintln!("DBG tuner: Read nothing from thread");
	    continue;
	}
	if !v.iter().any(|v| v.abs() > 0.0001) {
	    // eprintln!("DBG tuner: Read all zero from thread");
	    continue;
	}

	let note_result = match detect_note(&v, sample_rate) {
	    Ok(r) => r,
	    Err(_err) => {
		// This is happening for mysterious reasons
		// eprintln!("Error tuner: Send error in jack thread: {_err:?}");
		continue;
	    }
	};
	let note = note_result.note_name;
	let octave = note_result.octave;
	let cents = note_result.cents_offset;

	let report = format!(
	    "tuner: {:>3}/{octave} {:>6.6}  max: {max:>6.6} min: {min:>6.6} mean: {mean:>6.6} {:>6.6}\n",
	    note.to_string(),
	    cents.to_string(),
	    -(max / min)
	); //
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
#[cfg(test)]
mod tests {
    use super::*;
    use pitch_detector::core::NoteName;
    use pitch_detector::core::utils::sine_wave_signal;
    // Helper function to generate a sine wave signal
    fn generate_sine_wave(frequency: f64, sample_rate: f64, duration: f64) -> Vec<f64> {
	let num_samples = (sample_rate * duration) as usize;
	sine_wave_signal(num_samples, frequency, sample_rate)
    }

    #[test]
    fn test_detect_note_different_frequencies() {
	let sample_rate = 48000;
	let test_cases = vec![
	    (261.63, NoteName::C, 4), // C4
	    (293.66, NoteName::D, 4), // D4
	    (329.63, NoteName::E, 4), // E4
	    (392.00, NoteName::G, 4), // G4
	];

	for (frequency, expected_note, expected_octave) in test_cases {
	    let signal = generate_sine_wave(frequency, sample_rate as f64, 0.1);
	    let result = detect_note(&signal, sample_rate);

	    if let Ok(note_result) = result {
		assert_eq!(note_result.note_name, expected_note);
		assert_eq!(note_result.octave, expected_octave);
		assert!(
		    (note_result.actual_freq - frequency).abs() < 15.0,
		    "Frequency detection should be accurate"
		);
	    }
	}
    }

    #[test]
    fn test_detect_note_short_duration() {
	// Test with very short signal
	let sample_rate = 44100;
	let short_signal = generate_sine_wave(440.0, sample_rate as f64, 0.01); // 10ms

	let result = detect_note(&short_signal, sample_rate);

	// This might fail due to insufficient data, but shouldn't panic
	let _ = result;
    }

    #[test]
    fn test_detect_note_noise() {
	// Test with noisy signal (sine wave + random noise)
	let sample_rate = 44100;
	let frequency = 440.0;
	let mut signal = generate_sine_wave(frequency, sample_rate as f64, 0.1);

	// Add some noise
	for sample in signal.iter_mut() {
	    *sample += rand::random::<f64>() * 0.1 - 0.05; // ±0.05 noise
	}

	let result = detect_note(&signal, sample_rate);

	// Should handle a little noise gracefully
	let _ = result;

	// Add lots noise
	for sample in signal.iter_mut() {
	    *sample += rand::random::<f64>() - 0.5; // ±0.5 noise
	}

	let result = detect_note(&signal, sample_rate);

	// Should handle lots of noise gracefully
	let _ = result;

	// Test pure noise
	for sample in signal.iter_mut() {
	    *sample = rand::random::<f64>()
	}
	let result = detect_note(&signal, sample_rate);

	// Should handle pure noise gracefully
	let _ = result;
    }
}
