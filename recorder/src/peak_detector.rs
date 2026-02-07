// Copyright (c) 2026 Worik Turei Stanton
// License: GPL-3.0

//! Detecting "peaks" in Audio.
//! Thankyou Claude Code for helping with this.
use std::collections::VecDeque;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WarningLevel {
    Normal,
    Warning,
    Critical,
}

#[derive(Debug, Clone, Copy)]
pub struct PeakDetectorConfig {
    pub critical_threshold: f32, // e.g., 0.95
    pub debounce_ms: u32,        // minimum time between audible warnings
    pub sample_rate: u32,        // samples per second
    pub warning_threshold: f32,  // e.g., 0.8
    pub window_ms: u32,          // RMS window size in milliseconds
}

impl Default for PeakDetectorConfig {
    fn default() -> Self {
        Self {
            window_ms: 200,
            warning_threshold: 0.8,
            critical_threshold: 0.95,
            debounce_ms: 2_000,
            sample_rate: 48_000,
        }
    }
}

pub struct PeakDetector {
    buffer: VecDeque<f32>,
    config: PeakDetectorConfig,
    current_state: WarningLevel,
    last_warning_time: Option<Instant>,
    sum_of_squares: f64,
    window_samples: usize,
}

impl PeakDetector {
    pub fn new(config: PeakDetectorConfig) -> Self {
        let window_samples =
            (config.sample_rate as f32 * config.window_ms as f32 / 1000.0) as usize;

        Self {
            config,
            buffer: VecDeque::with_capacity(window_samples),
            sum_of_squares: 0.0,
            last_warning_time: None,
            current_state: WarningLevel::Normal,
            window_samples,
        }
    }

    /// Process a single sample and return the current warning level
    pub fn process_sample(&mut self, sample: f32) -> WarningLevel {
        let sample_f64 = sample as f64;
        let square = sample_f64 * sample_f64;

        // Add new sample
        self.buffer.push_back(sample);
        self.sum_of_squares += square;

        // Remove oldest sample if buffer is full
        if self.buffer.len() > self.window_samples
            && let Some(old_sample) = self.buffer.pop_front()
        {
            let old_square = old_sample as f64 * old_sample as f64;
            self.sum_of_squares -= old_square;
        }

        // Calculate RMS if we have enough samples
        if self.buffer.len() >= self.window_samples / 2 {
            // Start measuring with half window
            let rms = (self.sum_of_squares / self.buffer.len() as f64).sqrt() as f32;

            // Update state
            self.current_state = if rms >= self.config.critical_threshold {
                WarningLevel::Critical
            } else if rms >= self.config.warning_threshold {
                WarningLevel::Warning
            } else {
                WarningLevel::Normal
            };
        }

        self.current_state
    }

    /// Process multiple samples at once (more efficient for real-time audio)
    pub fn process_buffer(&mut self, samples: &[f32]) -> WarningLevel {
        for &sample in samples {
            self.process_sample(sample);
        }
        self.current_state
    }

    /// Get current RMS value
    pub fn current_rms(&self) -> Option<f32> {
        if self.buffer.is_empty() {
            return None;
        }
        Some((self.sum_of_squares / self.buffer.len() as f64).sqrt() as f32)
    }

    /// Get current level in dBFS (0 dB = maximum)
    pub fn current_dbfs(&self) -> Option<f32> {
        self.current_rms().map(|rms| 20.0 * rms.log10())
    }

    /// Check if we should issue a warning (with debounce)
    pub fn should_play_warning(&mut self) -> bool {
        if self.current_state != WarningLevel::Critical {
            return false;
        }

        match self.last_warning_time {
            None => {
                self.last_warning_time = Some(Instant::now());
                true
            }
            Some(last_time) => {
                if last_time.elapsed() >= Duration::from_millis(self.config.debounce_ms as u64) {
                    self.last_warning_time = Some(Instant::now());
                    true
                } else {
                    false
                }
            }
        }
    }

    /// Get a visual representation of the level
    pub fn visual_meter(&self, width: usize) -> String {
        let rms = self.current_rms().unwrap_or(0.0);
        let level_width = (rms * width as f32) as usize;
        let level_width = level_width.min(width);

        let meter = if self.current_state == WarningLevel::Critical {
            format!("[{}⚠]", "#".repeat(level_width))
        } else {
            format!(
                "[{}{}]",
                "#".repeat(level_width),
                " ".repeat(width - level_width)
            )
        };

        if let Some(dbfs) = self.current_dbfs() {
            format!("{} {:.1} dBFS", meter, dbfs)
        } else {
            meter
        }
    }

    /// Reset the detector
    pub fn reset(&mut self) {
        self.buffer.clear();
        self.sum_of_squares = 0.0;
        self.last_warning_time = None;
        self.current_state = WarningLevel::Normal;
    }
}

// Example usage
#[cfg(test)]
mod tests {
    use std::thread;

    use super::*;

    #[test]
    /// Test the calculation of rms
    fn test_current_rms_dbs() {
        let config = PeakDetectorConfig::default();
        let mut detector = PeakDetector::new(config);

        // The buffer must be full to calculate a predictable RMS and db
        let sample_count = config.sample_rate * config.window_ms / 1_000 + 1;

        // Test with silence
        let quiet_samples = (0..sample_count).map(|_| 0.0).collect::<Vec<f32>>();
        detector.process_buffer(&quiet_samples);

        // rms should be zero
        let rms = detector.current_rms().unwrap();
        assert_eq!(rms, 0.0);
        // dbfs is inf
        let dbfs = detector.current_dbfs().unwrap();
        assert!(dbfs.is_infinite());

        // Test with maximum volume
        let quiet_samples = (0..sample_count).map(|_| 1.0).collect::<Vec<f32>>();
        detector.process_buffer(&quiet_samples);

        // rms should be 1.0
        let rms = detector.current_rms().unwrap();
        assert_eq!(rms, 1.0);

        // dbfs should be zero
        let dbfs = detector.current_dbfs().unwrap();
        assert_eq!(dbfs, 0.0);

        // Test with minimum volume
        let quiet_samples = (0..sample_count).map(|_| -1.0).collect::<Vec<f32>>();
        detector.process_buffer(&quiet_samples);

        // rms should be 1.0
        let rms = detector.current_rms().unwrap();
        assert_eq!(rms, 1.0);

        // dbfs should be zero
        let dbfs = detector.current_dbfs().unwrap();
        assert_eq!(dbfs, 0.0);
    }
    #[test]
    fn test_peak_detection() {
        let config = PeakDetectorConfig::default();
        let mut detector = PeakDetector::new(config);

        // The buffer must be full to detect peaks.
        let sample_count = config.sample_rate * config.window_ms / 1_000 + 1;

        // Test with quiet signal
        let quiet_samples = (0..sample_count).map(|_| 0.1).collect::<Vec<f32>>();
        detector.process_buffer(&quiet_samples);
        assert_eq!(detector.current_state, WarningLevel::Normal);

        // Test with loud signal (above warning threshold)
        detector.reset();
        let loud_samples = (0..sample_count).map(|_| 0.9).collect::<Vec<f32>>();
        detector.process_buffer(&loud_samples);
        assert_eq!(detector.current_state, WarningLevel::Warning);

        // Test with clipping signal (above critical threshold)
        detector.reset();
        let clipping_samples = (0..sample_count).map(|_| 0.99).collect::<Vec<f32>>();
        detector.process_buffer(&clipping_samples);
        assert_eq!(detector.current_state, WarningLevel::Critical);
    }

    #[test]
    #[allow(clippy::field_reassign_with_default)]
    fn test_debounce() {
        let mut config = PeakDetectorConfig::default();
        config.debounce_ms = 100; // Short debounce for testing
        let mut detector = PeakDetector::new(config);

        // The buffer must be full to detect peaks.
        let sample_count = config.sample_rate * config.window_ms / 1_000 + 1;

        // Initialise the buffer with a lot of quiet data data.
        let samples = (0..sample_count).map(|_| 0.49).collect::<Vec<f32>>();
        detector.process_buffer(&samples);

        // Another mild sample should not trigger a warning
        detector.process_sample(0.59);
        assert!(!detector.should_play_warning());

        // Initialise the buffer with a lot of loud data.
        let samples = (0..sample_count).map(|_| 0.99).collect::<Vec<f32>>();
        detector.process_buffer(&samples);

        // First critical sample should trigger warning
        detector.process_sample(1.0);
        assert!(detector.should_play_warning());

        // Immediately after, shouldn't trigger again
        detector.process_sample(1.0);
        assert!(!detector.should_play_warning());

        // After debounce, should trigger warning
        thread::sleep(Duration::from_millis(config.debounce_ms as u64 + 1));
        detector.process_sample(1.0);
        assert!(detector.should_play_warning());
    }
}
