// Copyright (c) 2025 Worik Turei Stanton
// License: GPL-3.0

use std::collections::HashSet;
/// A `Section` is a collection of pads that are grouped".
/// All the pads in it are one colour and emit the same note
#[allow(unused)]
#[derive(Debug, Clone)]
pub struct Section {
    pub pads: Vec<u8>, // 11-88
    pub main_colour: [u8; 3],
    pub active_colour: [u8; 3],
    pub midi_note: u8,
}

impl Section {
    /// FIXME: This must validate and return an error for invalid values
    #[allow(unused)]
    pub fn new(pads: Vec<u8>, main_colour: [u8; 3], active_colour: [u8; 3], midi_note: u8) -> Self {
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
    fn valid_pad(pad: u8) -> bool {
        (11..=88).contains(&pad) && pad % 10 != 0 && pad % 10 != 9
    }

    // Check a set of `Section` to see if they are valid as a grouop
    #[allow(dead_code)]
    pub fn check_sections(sections: &[Section]) -> bool {
        // Can only be one section with no pads.  It is the default section
        let default_section_count = sections
            .iter()
            .filter(|x| x.pads.is_empty())
            .collect::<Vec<&Section>>()
            .len();
        let a = if default_section_count < 2 {
            true
        } else {
            eprintln!("Error qzn3t_pad: Too many ({default_section_count}) default sections");
            return false;
        };
        // No intersections
        let b = true;
        for i in 0..(sections.len() - 1) {
            for j in (i + 1)..sections.len() {
                if sections[i].intersect(&sections[j]) {
                    eprintln!(
                        "DBG qzn3t_pad: section intersection: Sections:\n\t{}\n\t{}",
                        sections[i], sections[j]
                    );
                    return false;
                }
            }
        }
        // There is a default section (with no pads) or every pad is
        // in a section, exactly once
        let c = default_section_count == 1 || {
            let mut hs: HashSet<u8> = HashSet::new();
            let mut v: Vec<u8> = Vec::new();
            for s in sections.iter() {
                for p in s.pads.iter() {
                    hs.insert(*p);
                    v.push(*p);
                }
            }
            if hs.len() == v.len() && v.len() == 64 {
                true
            } else {
                eprintln!("Error qzn3t_pad: There are some pads in more than one section");
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
        false
    }

    // pub fn row_col_to_pad(row: u8, col: u8) -> u8 {
    //     row * 10 + col
    // }
    // pub fn pad_to_row(pad: u8) -> u8 {
    //     pad / 10
    // }
    // pub fn pad_to_col(pad: u8) -> u8 {
    //     pad % 10
    // }
    /// Detect if a pad is in this section
    pub fn pad_in(&self, pad: u8) -> bool {
        self.pads.contains(&pad)
    }

    // Return all pad indexes in this section
    pub fn pads(&self) -> &Vec<u8> {
        &self.pads
    }
}

pub fn default_sections() -> Vec<Section> {
    vec![
        Section {
            pads: vec![
                11, 12, 13, 14, 21, 22, 23, 24, 31, 32, 33, 34, 41, 42, 43, 44,
            ],
            main_colour: [127, 0, 0],
            active_colour: [0, 127, 127],
            midi_note: 33,
        },
        Section {
            pads: vec![77, 78, 87, 88],
            main_colour: [127, 0, 64],
            active_colour: [0, 127, 63],
            midi_note: 42,
        },
        Section {
            pads: vec![
                15, 16, 17, 18, 25, 26, 27, 28, 35, 36, 37, 38, 45, 46, 47, 48,
            ],
            main_colour: [127, 32, 0],
            active_colour: [0, 95, 127],
            midi_note: 34,
        },
        Section {
            pads: vec![51, 52, 61, 62],
            main_colour: [127, 96, 0],
            active_colour: [0, 31, 127],
            midi_note: 35,
        },
        Section {
            pads: vec![53, 54, 63, 64],
            main_colour: [32, 127, 0],
            active_colour: [95, 0, 127],
            midi_note: 36,
        },
        Section {
            pads: vec![55, 56, 65, 66],
            main_colour: [0, 127, 32],
            active_colour: [127, 0, 95],
            midi_note: 37,
        },
        Section {
            pads: vec![57, 58, 67, 68],
            main_colour: [32, 0, 64],
            active_colour: [95, 127, 63],
            midi_note: 38,
        },
        Section {
            pads: vec![71, 72, 81, 82],
            main_colour: [96, 64, 64],
            active_colour: [31, 63, 63],
            midi_note: 39,
        },
        Section {
            pads: vec![73, 74, 83, 84],
            main_colour: [96, 32, 0],
            active_colour: [31, 95, 127],
            midi_note: 40,
        },
        Section {
            pads: vec![75, 76, 85, 86],
            main_colour: [0, 0, 127],
            active_colour: [127, 127, 0],
            midi_note: 41,
        },
    ]
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
