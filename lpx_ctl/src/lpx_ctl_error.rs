use std::error::Error;
use std::fmt;
/// The errors that can be generated in LpxCtl

#[derive(Debug)]
pub enum LpxCtlError {
    // InvalidSection,
    // InvalidSections,
    // IntersectingSections, // Sections intersect
    // DuplicateMainColour,  // > 1 section same colour NOT AN ERROR FIXME
    // DuplicateMIDI,        // >1 section same MIDI NOT AN ERROR FIXME
}

impl fmt::Display for LpxCtlError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            // LpxCtlError::InvalidSections => write!(f, "invalid sections"),
            // LpxCtlError::InvalidSection => write!(f, "invalid section"),
            // LpxCtlError::IntersectingSections => {
            //     write!(f, "intersecting sections")
            // }
            // LpxCtlError::DuplicateMainColour => {
            //     write!(f, "duplicate main colour")
            // }
            // LpxCtlError::DuplicateMIDI => {
            //     write!(f, "duplicate MIDI")
            // }
        }
    }
}

impl Error for LpxCtlError {}
