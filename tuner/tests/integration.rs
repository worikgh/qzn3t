// Copyright (c) 2025 Worik Turei Stanton
// License: GPL-3.0

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};
use tuner::{TunerArgs, inner_main};
#[test]
/// Make the data that can be used to find good settings for the
/// parameters.  Produce audio from Yoshimi, into Jackd, into tuner
/// then check if it has correct note
fn make_paramater_data() {
    // Yoshimi
    let yoshimi_exe = Path::new("/usr/bin/yoshimi");
    if !yoshimi_exe.exists() || !yoshimi_exe.is_file() {
	panic!("Yoshimi cannot be found at: {yoshimi_exe:?}");
    }
    if fs::metadata(yoshimi_exe)
	.expect("Cannot get metadata for {yoshimi_exe:?}")
	.permissions()
	.mode()
	& 0o111
	== 0
    {
	panic!("Yoshimi at {yoshimi_exe:?} is not exeutable");
    }

    // To be predictable Yoshimi requires instrument files.  A variety
    // of instruments for more interesting data....
    let inst_root = "/usr/share/yoshimi/banks";
    let instruments = vec![
	"Will_Godfrey_Collection/0095-Overdrive 3.xiz",
	"Bass/0001-Bass 1.xiz",
	"Guitar/0045-acoustic guitar.xiz",
	"Organ/0001-Organ 1.xiz",
	"Plucked/0001-Plucked 1.xiz",
	"Rhodes/0001-DX Rhodes 1.xiz",
    ]
    .iter()
    .map(|i| PathBuf::from(format!("{inst_root}/{i}")))
    .collect::<Vec<PathBuf>>();

    // Check the instruments all exist and are readable
    for p in instruments.iter() {
	match fs::metadata(p.as_path()) {
	    Ok(md) => {
		if md.permissions().mode() & 0o444 == 0 {
		    panic!("Path {p:?} not readable");
		}
	    }
	    Err(err) => panic!("Cannot get meta data for {p:?}.  {err}"),
	}
    }

    // Output file
    let out_fn = "tuner_parameter_data.txt";
    let mut fh = File::create(out_fn).unwrap();
    fh.write_all("# Testing Tuner Parameters\n".as_bytes())
	.unwrap();

    // The MIDI notes to use
    let mut midi_note_map: HashMap<u8, (String, u8)> = HashMap::new();
    let note_names = [
	"C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    for octave in 3_u8..8 {
	for (i, &note_name) in note_names.iter().enumerate() {
	    let midi_note = i as u8 + 12 * octave;
	    midi_note_map.insert(midi_note as u8, (note_name.to_string(), octave));
	}
    }
    let mut midi_notes: Vec<&u8> = midi_note_map.keys().collect();
    midi_notes.sort_by(|a, b| a.cmp(b));
    for midi_note in midi_notes.iter() {
	let (note_name, octave) = midi_note_map.get(*midi_note).unwrap();
	fh.write_all(
	    format!(
		"# MIDI Note: {}, Note Name: {}, Octave: {}\n",
		midi_note, note_name, octave
	    )
	    .as_bytes(),
	)
	.unwrap();
    }

    for interval in (100..600).step_by(25) {
	let mut count = 16;
	while count < (2048000 * 2 + 1) {
	    let tuner_args = TunerArgs {
		interval,
		count,
		max_vol_min: 0.0,
		mean_min: 1.0,
	    };
	    fh.write_all(format!("# Test Case: {interval} {count}\n").as_bytes())
		.unwrap();
	    for inst in instruments.iter() {
		// Start Yoshimi using Jack audio and Jack MIDI
		let inst_arg = format!("-L {:?}", inst.as_path());
		let mut child = Command::new(yoshimi_exe)
		    .arg("-i")
		    .arg("-J")
		    .arg("-c")
		    .arg("-K")
		    .arg(inst_arg)
		    .arg("-R 4800")
		    .spawn()
		    .expect("Failed to start yoshimi");

		// Create a channel for Strings send/receive
		// Call `tuner::get_results` with `send` String channel
		let (sender, receiver) = mpsc::channel::<String>();
		let jh = tuner::get_results(&tuner_args, sender);
		// Connect Yoshimi to `qzn3t_tuner:input`
		for midi_note in midi_notes.iter() {
		    // Use `rmidi` to send note to Yoshimi

		    // call `try_recv` of the receive end of channel
		    // passed to `get_results` and write any strings
		    // received to output with the note and octave
		    let mut call_instant = Instant::now();
		    loop {
			match receiver.try_recv() {
			    Ok(res) => {
				call_instant = Instant::now();
				()
			    },
			    Err(_) {
				let foo =  Instant::now() - call_instant;
				thread::sleep(Duration::from_millis(100))
			    }
			};
		    // If `try_recv` does not receive anything for a
		    // whole second (?configurable?) send the next note
		}
		// Kill the Yoshimi instance and stop the
		// `get_results` thread
		child.kill().expect("Failed to kill yoshimi");
		let _ = child.wait();
	    }
	    count = count * 2;
	}
    }
}
