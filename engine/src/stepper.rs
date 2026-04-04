// Copyright (c) 2026 Worik Turei Stanton
// License: GPL-3.0

//! Trait for processing audio one step at a time

use std::fmt::Debug;

use qzn3terror::Qzn3tError;

/// Used to pass commands into the main loop that control the stepper
pub enum StepCommand {
    /// Pause playback or record
    Pause,

    /// Start or restart playback or record
    Start,

    /// Set a stepper.  Will replace previous if one is already in place
    NewStepper(Box<dyn Stepper>),

    /// Exit the main loop
    Exit,
}

impl Debug for StepCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NewStepper(_) => write!(f, "NewStepper(...)"),
            Self::Pause => write!(f, "Pause"),
            Self::Start => write!(f, "Start"),
            Self::Exit => write!(f, "Exit"),
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum StepResult {
    Complete,
    Continue,
}
pub trait Stepper: Send + Debug {
    fn step(&mut self, sample_cnt: usize) -> Result<StepResult, Qzn3tError>;
    fn channels(&self) -> usize;
}
