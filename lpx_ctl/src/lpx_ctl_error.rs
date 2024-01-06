use crate::section::Section;
use std::error::Error;
use std::fmt;
/// The errors that can be generated in LpxCtl

#[derive(Debug)]
pub enum LpxCtlError {
    InvalidSection,
    IntersectingSections(Section, Section), // Sections intersect
    DuplicateMainColour(Section, Section), // > 1 section same colour
    DuplicateMIDI(Section, Section), // >1 section same MIDI
}

impl fmt::Display for LpxCtlError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            LpxCtlError::InvalidSection => write!(f, "invalid section"),
            LpxCtlError::IntersectingSections(s1, s2) => {
                write!(f, "intersecting sections: {} {}", s1, s2)
            }
            LpxCtlError::DuplicateMainColour(s1, s2) => {
                write!(f, "duplicate main colour: {} {}", s1, s2)
            }
            LpxCtlError::DuplicateMIDI(s1, s2) => {
                write!(f, "duplicate MIDI: {} {}", s1, s2)
            }
        }
    }
}

impl Error for LpxCtlError {}
