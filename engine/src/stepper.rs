// Copyright (c) 2026 Worik Turei Stanton
// License: GPL-3.0

//! Trait for processing audio one step at a time

use qzn3terror::Qzn3tError;
#[derive(Debug, Eq, PartialEq)]
pub enum StepResult {
    Complete,
    Continue,
}
pub trait Stepper {
    fn step(&mut self, step_ns: u128) -> Result<StepResult, Qzn3tError>;
    fn channels(&self) -> usize;
}
