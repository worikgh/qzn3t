// Copyright (c) 2025 Worik Turei Stanton
// License: GPL-3.0

use jack::{AudioIn, Client, Port, ProcessHandler, ProcessScope};
use pitch_detector::{
    core::NoteName,
    note::{NoteDetectionResult, detect_note as abc_detect_note},
    pitch::HannedFftDetector,
};
use rtrb::RingBuffer;
use std::error::Error;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

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
        let buffer = self.capture_port.as_slice(ps);
        let current_time = Instant::now();

        // Check if it's time to start a new sample
        if current_time.duration_since(self.last_sample_time) >= self.sample_interval {
            self.last_sample_time = current_time;
            let num_samples_needed =
                (self.sample_rate as f32 * self.sample_duration.as_secs_f32()) as usize;
            match self.sample_buffer.lock() {
                Ok(mut buf_guard) => {
                    buf_guard.clear();
                    buf_guard.extend_from_slice(&buffer[..num_samples_needed.min(buffer.len())]);
                }
                Err(err) => eprintln!(
                    "Error tuner: Getting buffer lock TunerProcessHandler::process.  Error: {err}"
                ),
            };
        }
        jack::Control::Continue
    }
}

fn detect_note(signal: &[f64], sample_rate: f64) -> Result<NoteDetectionResult, Box<dyn Error>> {
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

fn main() {
    let (client, _status) =
        jack::Client::new("qzn3t_tuner", jack::ClientOptions::NO_START_SERVER).unwrap();

    // Register capture port
    let capture_port = client.register_port("input", AudioIn::default()).unwrap();

    // Shared buffer for samples
    let sample_buffer = Arc::new(Mutex::new(Vec::<f32>::new()));
    let sample_buffer_clone = Arc::clone(&sample_buffer);

    // Sampling parameters
    let sample_rate = client.sample_rate();
    let sample_interval = Duration::from_millis(20);
    let sample_duration = Duration::from_millis(10);

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
    // Main thread: Periodically print the sampled buffer
    let buffer_sz = 10;
    let (mut producer, mut consumer) = RingBuffer::<NoteDetectionResult>::new(buffer_sz);
    let mut note_name = NoteName::A;
    let mut dbga = 0;
    let mut dbgb = 0;
    loop {
        thread::sleep(sample_interval);
        let buf_guard = sample_buffer.lock().unwrap();
        if buf_guard.is_empty() {
            println!("DBG tuner:No samples");
            continue;
        }
        let note = detect_note(
            &buf_guard.iter().map(|&x| x as f64).collect::<Vec<f64>>(),
            sample_rate as f64,
        );
        if let Ok(note) = note {
            _ = producer.push(note);
            dbga += 1;
        } else {
            dbgb += 1;
        }
        if (dbga + dbgb) % 100 == 0 {
            eprintln!(
                "DBG tuner: Ok: {dbga} Err: {dbgb}: consumer.slots() {} buffer_sz {}",
                consumer.slots(),
                buffer_sz
            );
        }
        if consumer.slots() == buffer_sz {
            if let Ok(chunk) = consumer.read_chunk(buffer_sz) {
                let (notes, _) = chunk.as_slices();
                // chunk.commit_all();
                let in_tune = !notes.iter().any(|n| !n.in_tune);
                if in_tune {
                    let nn = notes[0].note_name.clone();
                    let no = notes[0].octave;
                    if notes.iter().any(|n| n.note_name != nn || no != n.octave) {
                        eprintln!("Error tuner: Inconsistent note data");
                        continue;
                    }
                    if note_name != nn {
                        note_name = nn;
                        let octave = no;
                        eprintln!("DBG tuner: {note_name}/{octave}");
                    }
                } else {
                    let cents_offset = notes.iter().fold(0_f64, |a, b| a + b.cents_offset);
                    let cents_offset = cents_offset / notes.len() as f64;
                    eprintln!("DBG tuner: Mean cents: {cents_offset}");
                }
            };
            if let Ok(chunk) = consumer.read_chunk(buffer_sz) {
                chunk.commit_all();
            }
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
        let sample_rate = 48000.0;
        let test_cases = vec![
            (261.63, NoteName::C, 4), // C4
            (293.66, NoteName::D, 4), // D4
            (329.63, NoteName::E, 4), // E4
            (392.00, NoteName::G, 4), // G4
        ];

        for (frequency, expected_note, expected_octave) in test_cases {
            let signal = generate_sine_wave(frequency, sample_rate, 0.1);
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
        let sample_rate = 44100.0;
        let short_signal = generate_sine_wave(440.0, sample_rate, 0.01); // 10ms

        let result = detect_note(&short_signal, sample_rate);

        // This might fail due to insufficient data, but shouldn't panic
        let _ = result;
    }

    #[test]
    fn test_detect_note_noise() {
        // Test with noisy signal (sine wave + random noise)
        let sample_rate = 44100.0;
        let frequency = 440.0;
        let mut signal = generate_sine_wave(frequency, sample_rate, 0.1);

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
