use crate::lpx_ctl_error::LpxCtlError;
use serde::{Deserialize, Serialize};
/// A `Section` is a collection of pads on a LPX that is grouped".
/// All the pads in it are one colour and emit the same note
use std::error::Error;
#[allow(unused)]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Section {
    pads: Vec<u8>, // 11-88
    pub main_colour: [u8; 3],
    pub active_colour: [u8; 3],
    pub midi_note: u8,
}

impl Section {
    /// FIXME: This must validate and return an error for invalid values
    #[allow(unused)]
    pub fn new(
        pads: Vec<u8>,
        main_colour: [u8; 3],
        active_colour: [u8; 3],
        midi_note: u8,
    ) -> Result<Self, LpxCtlError> {
        // -> Result<Self, LpxCtlError>
        let result = Self {
            pads,
            main_colour,
            active_colour,
            midi_note,
        };
        if result.valid() {
            // Ok(result)
            Ok(result)
        } else {
            // Err(LpxCtlError::invalid_section)
            Err(LpxCtlError::InvalidSection)
        }
    }

    /// Check the constraints on a `Section`:
    /// `pad` must be valid
    /// Width and height MUST BE VALID    
    fn valid(&self) -> bool {
        !self.pads.iter().any(|&x| {
            !(x < 11 // smallest...
                || x > 88 // biggest
                || self // No repeats
                    .pads
                    .iter()
                    .filter(|&&y| x == y)
                    .collect::<Vec<&u8>>()
                    .len()
                    != 1)
        }) && self.pads.len() <= 64
    }

    // Check that a `pad` is valid
    #[allow(dead_code)]
    fn valid_pad(pad: u8) -> bool {
        (11..=88).contains(&pad) && pad % 10 != 0 && pad % 10 != 9
    }

    // Check a set of `Section` to see if they are valid as a grouop
    pub fn check_sections(sections: &Vec<Section>) -> Result<(), LpxCtlError> {
        for i in 0..sections.len() {
            for j in (i + 1)..sections.len() {
                if sections[i].intersect(&sections[j]) {
                    return Err(LpxCtlError::IntersectingSections);
                }
                if sections[i].main_colour == sections[j].main_colour {
                    return Err(LpxCtlError::DuplicateMainColour);
                }
                if sections[i].midi_note == sections[j].midi_note {
                    return Err(LpxCtlError::DuplicateMIDI);
                }
            }
        }
        Ok(())
    }
    #[allow(dead_code)]
    pub fn intersect(&self, other: &Self) -> bool {
        for i in self.pads.iter() {
            for j in other.pads.iter() {
                if i == j {
                    return true;
                }
            }
        }
        return false;
    }

    pub fn parse_json(input: &str) -> Result<Vec<Section>, Box<dyn Error>> {
        let result: Vec<Section> = serde_json::from_str(input)?;
        match Self::check_sections(&result) {
            Ok(()) => Ok(result),
            Err(err) => Err(Box::new(err)),
        }
    }
    pub fn row_col_to_pad(row: u8, col: u8) -> Result<u8, LpxCtlError> {
        let result = row * 10 + col;
        Ok(result)
    }
    pub fn pad_to_row(pad: u8) -> u8 {
        pad / 10
    }
    pub fn pad_to_col(pad: u8) -> u8 {
        pad % 10
    }
    // pub fn row(&self) -> u8 {
    //     Self::pad_to_row(self.pad)
    // }
    // pub fn col(&self) -> u8 {
    //     Self::pad_to_col(self.pad)
    // }
    /// Detect if a pad on the LPX is in this section
    pub fn pad_in(&self, pad: u8) -> bool {
        self.pads.contains(&pad)
    }

    // Return all pad indexes in this section
    pub fn pads(&self) -> &Vec<u8> {
        &self.pads
    }
}

use std::fmt;
#[allow(unused)]
impl fmt::Display for Section {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Section - Pads: {:?}  Main Colour:[{}, {}, {}], Active Colour: [{}, {}, {}]",
            self.pads,
            self.main_colour[0],
            self.main_colour[1],
            self.main_colour[2],
            self.active_colour[0],
            self.active_colour[1],
            self.active_colour[2]
        )
    }
}
#[cfg(test)]
mod tests {

    use super::*;

    // #[test]
    // fn test_valid() {
    //     // this is an invalid pad (1).
    //     let test = move || -> Result<Section, LpxCtlError> {
    //         let section = Section::new(1, 1, 1, [0, 0, 0], [0, 0, 0], 22)?;
    //         Ok(section)
    //     };
    //     assert!(test().is_err());

    //     // this is an invalid main_colour (128).
    //     let test = move || -> Result<Section, LpxCtlError> {
    //         let section = Section::new(11, 1, 1, [128, 0, 0], [0, 0, 0], 23)?;
    //         Ok(section)
    //     };
    //     let test = test();
    //     assert!(test.is_err());

    //     // this is an invalid activate_colour (128).
    //     let test = move || -> Result<Section, LpxCtlError> {
    //         let section = Section::new(11, 1, 1, [0, 0, 0], [0, 128, 0], 24)?;
    //         Ok(section)
    //     };
    //     assert!(test().is_err());
    //     // this is a valid section
    //     let test = move || -> Result<Section, LpxCtlError> {
    //         let section = Section::new(11, 1, 1, [0, 0, 0], [0, 12, 0], 25)?;
    //         Ok(section)
    //     };
    //     assert!(test().is_ok());
    // }

    // #[test]
    // fn test_intersect() {
    //     // Test two sections that intersect
    //     let test = move || -> Result<bool, LpxCtlError> {
    //         let section_1 = Section::new(11, 8, 8, [0, 0, 0], [0, 0, 0], 26)?;
    //         let section_2 = Section::new(11, 8, 8, [0, 0, 0], [0, 0, 0], 27)?;
    //         Ok(section_1.intersect(&section_2))
    //     };
    //     let test = test();
    //     assert!(test.is_ok());
    //     assert!(test.unwrap());

    //     // Two that do not
    //     let test = move || -> Result<bool, LpxCtlError> {
    //         let section_1 = Section::new(11, 4, 3, [0, 0, 0], [0, 0, 0], 28)?;
    //         let section_2 = Section::new(15, 3, 3, [0, 0, 0], [0, 0, 0], 29)?;
    //         Ok(section_1.intersect(&section_2))
    //     };
    //     let test = test();
    //     assert!(test.is_ok());
    //     assert!(!test.unwrap());
    // }

    #[test]
    fn test_json() {
        let json: &str = r#"
[
    {
        "pads": [11, 12, 13, 14],
        "main_colour": [1, 1, 127],
        "active_colour": [1, 127, 1],
        "midi_note": 25
    },
    {
        "pads": [15, 16, 17],
        "main_colour": [1, 127, 127],
        "active_colour": [1, 127, 1],
        "midi_note": 26
    }
]
"#
        .trim();
        let sections: Vec<Section> = Section::parse_json(json).unwrap();
        assert!(Section::check_sections(&sections).is_ok());
        // Check each pad is in at most one section
        for pad in 11..89 {
            if pad % 10 == 0 || pad % 10 == 9 {
                continue;
            }
            let mut flag = false;
            for section in sections.iter() {
                if section.pad_in(pad) {
                    if flag {
                        // More than one section
                        panic!("Pad: {pad} is in more than one section");
                    }
                    flag = true;
                }
            }
        }
        // Check there are pads in each section
        let pads = sections[0].pads();
        eprintln!("Pads: {pads:?}");
        assert!(pads.len() == 1);
        assert!(sections[1].pads().len() == 4);
    }
}
