// Copyright (c) 2025 Worik Turei Stanton
// License: GPL-3.0

use clap::Parser;
use qzn3t_pitch_detection::note_detection_result::NoteDetectionResult;
use qzn3t_pitch_detection::note_detection_result::NoteName;
use qzn3t_pitch_detection::runner::{Detector, DetectorCfg, pitch_detection_run};
use std::io::Write;
use std::sync::{Arc, atomic::AtomicBool, atomic::Ordering, mpsc};
use std::thread::spawn;
use std::{
    io::{self},
    thread::JoinHandle,
};

#[derive(Debug, Clone, Eq, PartialEq, PartialOrd, Ord)]
pub enum TunerNote {
    A,
    ASharp,
    B,
    C,
    CSharp,
    D,
    DSharp,
    E,
    F,
    FSharp,
    G,
    GSharp,
}

impl From<NoteName> for TunerNote {
    fn from(n: NoteName) -> Self {
        match n {
            NoteName::A => TunerNote::A,
            NoteName::ASharp => TunerNote::ASharp,
            NoteName::B => TunerNote::B,
            NoteName::C => TunerNote::C,
            NoteName::CSharp => TunerNote::CSharp,
            NoteName::D => TunerNote::D,
            NoteName::DSharp => TunerNote::DSharp,
            NoteName::E => TunerNote::E,
            NoteName::F => TunerNote::F,
            NoteName::FSharp => TunerNote::FSharp,
            NoteName::G => TunerNote::G,
            NoteName::GSharp => TunerNote::GSharp,
        }
    }
}
/// Data to return from tuner::get_results
#[derive(Debug)]
pub struct TunerData {
    pub note: TunerNote,
    pub octave: i32,
    pub cents_offset: f32,
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct TunerArgs {
    #[arg(short = 'p', long)]
    pub connect_port: Option<String>, // If specified connct this port to the tuner
}

pub fn get_results(
    args: &TunerArgs,
    sender: mpsc::Sender<TunerData>,
    kill_switch: Arc<AtomicBool>,
) -> JoinHandle<()> {
    let port = match &args.connect_port {
        Some(p) => p.clone(),
        None => "system:capture_1".to_string(),
    };
    // Channel for note data from pitech detector
    let (tx, rx) = mpsc::channel::<NoteDetectionResult>();
    //  Channel for audio data from Jack to pitch detector
    let (tx_f32, rx_f32) = mpsc::channel::<f32>();

    // Set up the pitch detection Jack client
    let audio_dst_client = match qzn3t_pitch_detection::runner::start_jack(tx_f32, &port) {
        Ok(ac) => ac,
        Err(err) => panic!(
            "Error pitch_detectiopn tester: Cannot create Jack clent to receive audio: {err}"
        ),
    };
    let detector_cfg = DetectorCfg {
        sample_rate: audio_dst_client
            .as_client()
            .sample_rate()
            .try_into()
            .unwrap(),
        size: 16384,
        padding: 1024,
        power_threshold: 0.1,
        clarity_threshold: 0.5,
        detector: Detector::McLeod,
    };

    let pd_handle = pitch_detection_run(tx, rx_f32, &detector_cfg, Some(kill_switch.clone()));
    spawn(move || {
        // Move the client into the thread so it is not shut down
        let _audio_dst_client = audio_dst_client;
        let sender = sender.clone();
        loop {
            if kill_switch.load(Ordering::SeqCst) {
                // Tuner disabled
                _ = pd_handle.join();
                break;
            }

            match rx.recv() {
                Ok(ndr) => {
                    let td = TunerData {
                        octave: ndr.octave,
                        note: ndr.note_name.into(),
                        cents_offset: ndr.cents,
                    };
                    if let Err(err) = sender.send(td) {
                        eprintln!(
                            "Error qzn3t/tuner: Note detection loop failed sending results: {err}"
                        );
                        break;
                    }
                }
                Err(err) => {
                    eprintln!(
                        "Error qzn3t/tuner: Note detection loop failed receiving results: {err}"
                    );
                    break;
                }
            }
        }
    })
}

pub fn inner_main(args: &TunerArgs) {
    let kill_switch = Arc::new(AtomicBool::new(false));
    let (sender, receiver) = mpsc::channel::<TunerData>();
    _ = get_results(args, sender, kill_switch.clone());
    loop {
        let ndr = match receiver.recv() {
            Ok(r) => r,
            Err(err) => {
                eprintln!("DBG tuner: get_results send error: {err}");
                break;
            }
        };
        let report = format!(
            "Tuner> {:?}/{} {: >-6.2}\n",
            ndr.note, ndr.octave, ndr.cents_offset
        );
        if let Err(err) = my_write(report) {
            eprintln!("Error tuner: inner main. {err}");
        }
    }
}

fn my_write(report: String) -> io::Result<()> {
    let mut v = io::stdout().lock();
    v.write_all(report.as_bytes())?;
    v.flush()?;
    Ok(())
}
