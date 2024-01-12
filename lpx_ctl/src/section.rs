use std::collections::HashSet;
// use crate::lpx_ctl_error::LpxCtlError;
use serde::{Deserialize, Serialize};
/// A `Section` is a collection of pads on a LPX that is grouped".
/// All the pads in it are one colour and emit the same note
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
    ) -> Self {
        // -> Result<Self, LpxCtlError>
        let result = Self {
            pads,
            main_colour,
            active_colour,
            midi_note,
        };
        if result.valid() {
            // Ok(result)
            result
        } else {
            // Err(LpxCtlError::invalid_section)
            panic!("Invalid section");
        }
    }

    /// Check the constraints on a `Section`:
    /// Each pad in `pads` must be valid
    /// There must be no repeats
    /// There can be zero pads
    fn valid(&self) -> bool {
        !self.pads.iter().any(|x| {
            !Self::valid_pad(*x)
                || self // No repeats
                    .pads
                    .iter()
                    .filter(|y| &x == y)
                    .collect::<Vec<&u8>>()
                    .len()
                    != 1
        }) && self.pads.len() <= 64
    }

    // Check that a `pad` is valid
    #[allow(dead_code)]
    fn valid_pad(pad: u8) -> bool {
        (11..=88).contains(&pad) && pad % 10 != 0 && pad % 10 != 9
    }

    // Check a set of `Section` to see if they are valid as a grouop
    pub fn check_sections(sections: &Vec<Section>) -> bool {
        // Can only be one section with no pads.  It is the default section
        let default_section_count = sections
            .iter()
            .filter(|x| x.pads.len() == 0)
            .collect::<Vec<&Section>>()
            .len();
	let a = if default_section_count < 2 {
	    true
	}else{
	    eprintln!("Too many ({default_section_count}) default sections");
	    false
	};
        // No intersections
        let mut b = true;
        for i in 0..(sections.len() - 1) {
            for j in (i + 1)..sections.len() {
                if sections[i].intersect(&sections[j]) {
                    eprintln!(
                        "section intersection: Sections:\n\t{}\n\t{}",
                        sections[i], sections[j]
                    );
                    b = false;
                }
            }
        }
        // There is a default section (with no pads) or every pad is
        // in a section, exactly once
	let c = default_section_count == 1 || {
	    let mut hs:HashSet<u8> = HashSet::new();
	    let mut v:Vec<u8> = Vec::new();
	    for s in sections.iter() {
		for p in s.pads.iter() {
		    hs.insert(*p);
		    v.push(*p);
		}
	    }
	    if hs.len() == v.len() && v.len() == 64 {
		true
	    }else{
		eprintln!("There are some pads in more than one section");
		false
	    }
	};
        a && b && c
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

    pub fn parse_json(input: &str) -> Option<Vec<Section>>{
        let result: Vec<Section> = match 
	    serde_json::from_str(input){
		Ok(r) => r,
		Err(err) => panic!("{err}"),
	    };
        match Self::check_sections(&result) {
            true => Some(result),
            false => {
		eprintln!("Sections check failed");
		None
	    },
        }
    }
    pub fn row_col_to_pad(row: u8, col: u8) -> u8{
        row * 10 + col
    }
    pub fn pad_to_row(pad: u8) -> u8 {
        pad / 10
    }
    pub fn pad_to_col(pad: u8) -> u8 {
        pad % 10
    }
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
