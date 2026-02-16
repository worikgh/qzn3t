// Copyright (c) 2025 Worik Turei Stanton
// License: GPL-3.0

use qzn3t_recorder::test_utils::common::{
    WaveForm, dst_dir, find_zeros, generate_test_audio, make_test_play_client, play_test_audio,
    set_up_recorder, trim_audio,
};

use jack::PortFlags;
use qzn3t_recorder::{
    app::App,
    io::{JackPipes, Metadata, read_f32_vec_from_file, read_file_metadata},
    structs::Command,
    utils::get_sample_rate,
};

use std::{
    sync::{Arc, Mutex, atomic::Ordering},
    thread,
    time::Duration,
};

/// Audio durations (milliseconds) to use in testing
const AUDIO_DURATION: [u32; 4] = [1, 10, 100, 1_000];

// Tests todo:
// `get_audio_from_jack` when the pipe is disconnected.  Test the error

/// The buffers recorded by the Jack sink are (sometimes) 1024 bytes
/// longer than the buffer sent.  Of the two buffers passed here if
/// they are different length return the extra data pretty printed as
/// text I have found is long runs of zeros in the extra data in the
/// longer buffer
fn dbg_buffers(left: &[f32], right: &[f32]) -> Option<String> {
    let left_len = left.len();
    let right_len = right.len();
    if left_len != right_len {
        let (short, long) = if left_len > right_len {
            (right, left)
        } else {
            (left, right)
        };
        let len = long.len() - short.len();
        let start = long.len().saturating_sub(2 * len);
        let err_vec = long[start..].to_vec();
        Some(pretty_buffer(&err_vec))
    } else {
        None
    }
}

/// Prepare a buffer of f32 to be printed
fn pretty_buffer(input: &[f32]) -> String {
    let chunked = input.chunks(4);
    chunked.fold("".to_string(), |a, b| {
        format!(
            "{a}{}\n",
            b.iter().fold("".to_string(), |a, b| format!("{a}{b:>8.4}"))
        )
    })
}

/// Generate two identical audio tracks (WaveForm::Geometric).  Record
/// both simultaneously and save them both to disc files.  Use the
/// generated audio file to test the command line interface for
/// playing back files.
#[test]
fn record_two_channels_and_play_back() {
    // The test audio
    for audio_duration in AUDIO_DURATION.iter() {
        let audio_duration_ms = audio_duration;

        let audio_buffer_sine =
            generate_test_audio(220, 0.25, *audio_duration, WaveForm::Geometric);
        let audio_buffer_sine = trim_audio(&audio_buffer_sine);

        let audio_buffer_tri = generate_test_audio(220, 0.25, *audio_duration, WaveForm::Geometric);
        let audio_buffer_tri = trim_audio(&audio_buffer_tri);

        // Directory recorded audio is sent to
        let output_path = dst_dir().join("two_channels");

        // Client to play the output:
        let port_sine = "sine-wave";
        let port_tri = "tri-wave";

        let client_name = "integration_test";
        let (ac, play_audio_f, zeros, non_zeros) = play_test_audio(
            client_name,
            vec![port_sine, port_tri],
            vec![&audio_buffer_sine, &audio_buffer_tri],
        );

        // Set up the recorder
        // The inputs (Jack pipes to record)
        let mut inputs = JackPipes::new(true);
        let port_tri = format!("{}:{port_tri}", ac.as_client().name());
        if let Err(err) = inputs.add(&port_tri) {
            panic!("{err}");
        }
        let port_sine_complete = format!("{}:{port_sine}", ac.as_client().name());
        if let Err(err) = inputs.add(&port_sine_complete) {
            panic!("{err}");
        }
        let mut recorder = set_up_recorder(vec![port_sine_complete, port_tri], &output_path);

        // Start the recorder.
        if let Err(err) = recorder.handle_record() {
            panic!("Called handle_recording(): {err}");
        }

        // Start the test signal
        play_audio_f.store(true, Ordering::SeqCst);

        // Wait for audio to stop
        thread::sleep(Duration::from_millis(*audio_duration_ms as u64));
        let mut loop_cnt = 0_u64;
        let delay_ms = 100;
        const LOOP_LIM: u64 = 10;
        loop {
            loop_cnt += 1;
            if !play_audio_f.load(Ordering::SeqCst) {
                break;
            }
            if loop_cnt >= LOOP_LIM {
                panic!(
                    "Audio has not stopped playing: {}ms elapsed",
                    loop_cnt * delay_ms
                );
            }
            thread::sleep(Duration::from_millis(100));
        }
        if let Err(err) = recorder.handle_audio_stop() {
            panic!("Could not stop audio: {err}");
        }
        // Get two recorded buffers
        let rec_sine = recorder.recorded_audio.get_buffer(0).unwrap();
        let rec_sine = trim_audio(&rec_sine);
        let rec_tri = recorder.recorded_audio.get_buffer(1).unwrap();
        let rec_tri = trim_audio(&rec_tri);

        // The two recorded buffers must be identical to the source buffers
        if let Some(dbg_msg) = dbg_buffers(&rec_sine, &audio_buffer_sine) {
            eprint!("Error: Sine:\n{dbg_msg} ");
            dbg!()
        }
        if let Some(dbg_msg) = dbg_buffers(&rec_tri, &audio_buffer_tri) {
            eprint!("Drror: Tri:\n{dbg_msg} ");
            dbg!()
        }
        assert_eq!(rec_sine.len(), audio_buffer_sine.len());
        assert_eq!(rec_tri.len(), audio_buffer_tri.len());
        for i in 0..audio_buffer_sine.len() {
            assert!((rec_sine[i] - audio_buffer_sine[i]).abs() < f32::EPSILON);
        }
        for i in 0..audio_buffer_tri.len() {
            assert!((rec_tri[i] - audio_buffer_tri[i]).abs() < f32::EPSILON);
        }

        // Good so far.  The recorded buffers match the input.  Now
        // check the recorded files are the same

        let audio_path = recorder.file_manager.make_paths().unwrap().0;
        let metadata_path = recorder.file_manager.make_paths().unwrap().1;
        let metadata: Metadata = match read_file_metadata(metadata_path) {
            Ok(p) => p,
            Err(err) => panic!("{err}"),
        };
        let channels = metadata.channels;
        assert_eq!(channels, 2);
        assert_eq!(metadata.sample_rate, get_sample_rate());
        match read_f32_vec_from_file(&audio_path, channels) {
            Ok(audio_buffer) => {
                let recovered_sine = audio_buffer.get_buffer(0).unwrap();
                let recovered_sine = trim_audio(&recovered_sine);
                let recovered_tri = trim_audio(&audio_buffer.get_buffer(1).unwrap());
                assert_eq!(recovered_tri.len(), rec_tri.len());
                assert_eq!(recovered_sine.len(), rec_sine.len());
                for i in 0..rec_tri.len() {
                    assert!((rec_tri[i] - recovered_tri[i]).abs() < f32::EPSILON);
                }

                for i in 0..rec_sine.len() {
                    assert!((rec_sine[i] - recovered_sine[i]).abs() < f32::EPSILON);
                }
            }
            Err(err) => {
                panic!(
                    "* Error: Cannot read data from {audio_path:?}.  Error: {err} audio_duration: {audio_duration}"
                );
            }
        };

        // Test playing back the audio files from the previous test
        let buffers = vec![
            Arc::new(Mutex::new(Vec::<f32>::new())),
            Arc::new(Mutex::new(Vec::<f32>::new())),
        ];
        let buffers_new = buffers.to_vec();
        let port_names = vec!["playback_1".to_string(), "playback_2".to_string()];

        let client_name = "test_play_client";
        let ac = make_test_play_client(client_name, port_names, buffers).unwrap();
        let client_name = ac.as_client().name();
        let port_names = ac
            .as_client()
            .ports(Some(client_name), None, PortFlags::IS_INPUT);
        // dbg!(&port_names);
        let mut outputs = JackPipes::new(false);
        for p in port_names.iter() {
            outputs.add(p).unwrap();
        }

        let mut app_data = match App::initialise(JackPipes::new(true), outputs, &output_path, true)
        {
            Ok(a) => a,
            Err(err) => panic!("Cannot initalise AppData: {err}"),
        };

        app_data.handle_kommand(Command::Play).unwrap();
        let eq = |v: &[u32]| -> bool {
            match v.first() {
                None => true, // empty: consider all-equal
                Some(&first) => v.iter().all(|&x| x == first),
            }
        };
        if !eq(&zeros.lock().unwrap()) {
            dbg!(&zeros);
        }
        if !eq(&non_zeros.lock().unwrap()) {
            dbg!(&non_zeros);
        }
        // Check the buffers are the same
        let file_path = match app_data.file_manager.make_paths() {
            Ok(pp) => pp.0,
            Err(err) => panic!("FileManager.make_paths(): {err}"),
        };
        let metadata_path = recorder.file_manager.make_paths().unwrap().1;
        let metadata: Metadata = match read_file_metadata(metadata_path) {
            Ok(p) => p,
            Err(err) => panic!("{err}"),
        };
        let channels = metadata.channels;
        assert_eq!(channels, 2);
        assert_eq!(metadata.sample_rate, get_sample_rate());

        let audio_buffers = match read_f32_vec_from_file(&file_path, channels) {
            Ok(p) => p,
            Err(err) => panic!("{err}"),
        };
        let ab_0 = trim_audio(audio_buffers.get_buffer_idx(0).unwrap());
        let ab_1 = trim_audio(audio_buffers.get_buffer_idx(1).unwrap());
        let bf_0 = trim_audio(&buffers_new[0].lock().unwrap());
        let bf_1 = trim_audio(&buffers_new[1].lock().unwrap());
        for buf in [
            (&ab_0, "&ab_0"),
            (&ab_1, "&ab_1"),
            (&bf_0, "&bf_0"),
            (&bf_1, "&bf_1"),
        ] {
            let zeros = find_zeros(buf.0);
            if !zeros.is_empty() {
                eprint!("Zeros: {} {:?} @ {audio_duration}", buf.1, &zeros);
                dbg![];
            }
        }

        if let Some(dbg_msg) = dbg_buffers(&ab_0, &bf_0) {
            eprint!("Error: ab_0/bf_0:\n{dbg_msg} ");
            dbg!()
        }
        // dbg!(&bf_0.len());
        if let Some(dbg_msg) = dbg_buffers(&ab_1, &bf_1) {
            eprint!("Error: ab_1/bf_1:\n{dbg_msg} ");
            dbg!()
        }
        assert_eq!(ab_0.len() as i32 - bf_0.len() as i32, 0);
        for idx in 0..ab_0.len() {
            let a = ab_0[idx];
            let b = bf_0[idx];
            assert!((a - b).abs() < f32::EPSILON);
        }
        // dbg!(ab_1.len(), bf_1.len());
        assert_eq!(ab_1.len() as i32 - bf_1.len() as i32, 0);
        for idx in 0..ab_1.len() {
            let a = ab_1[idx];
            let b = bf_1[idx];
            assert!((a - b).abs() < f32::EPSILON);
        }
    }
}

#[test]
fn test_playback() {
    let args: Vec<String> = std::env::args().collect();

    let audio_duration_ms = match args.get(2) {
        Some(s) => s,
        None => &"100".to_string(),
    };

    let audio_duration_ms = match audio_duration_ms.parse::<u32>() {
        Ok(u) => u,
        Err(err) => {
            panic!("Failed do get duration ms: {err}");
        }
    };
    dbg!(&audio_duration_ms);
    let audio_buffer_geo = generate_test_audio(220, 0.25, audio_duration_ms, WaveForm::Geometric);
    let audio_buffer_geo = trim_audio(&audio_buffer_geo);

    // Directory recorded audio is sent to
    let output_path = dst_dir().join("geometric");

    // Client to play the output:
    let client_name = "integration_test";
    // Output port
    let port_name = "geometric";
    let (ac, play_audio_f, _, _) =
        play_test_audio(client_name, vec![port_name], vec![&audio_buffer_geo]);
    assert!(!play_audio_f.load(Ordering::Relaxed));

    // Set up the recorder
    let mut inputs = JackPipes::new(true);
    let port_name_complete = format!("{}:{port_name}", ac.as_client().name());
    if let Err(err) = inputs.add(&port_name_complete) {
        panic!("{err}");
    }
    let mut app_data = set_up_recorder(vec![port_name_complete], &output_path);
    if let Err(err) = app_data.handle_record() {
        panic!("Called handle_recording(): {err}");
    }

    // Start the test signal
    play_audio_f.store(true, Ordering::SeqCst);

    // Wait for audio to stop
    thread::sleep(Duration::from_millis(audio_duration_ms as u64));
    let mut loop_cnt = 0;
    let delay_ms = 100;
    const LOOP_LIM: u32 = 10;
    loop {
        if !play_audio_f.load(Ordering::SeqCst) {
            break;
        }
        if loop_cnt >= LOOP_LIM {
            panic!(
                "Audio has not stopped playing: {}ms elapsed",
                loop_cnt * delay_ms
            );
        }
        loop_cnt += 1;
        thread::sleep(Duration::from_millis(100));
    }
    if let Err(err) = app_data.handle_audio_stop() {
        panic!("Could not stop audio: {err}");
    }

    // Get two recorded buffers
    let rec_geo = app_data.recorded_audio.get_buffer(0).unwrap();
    let rec_geo = trim_audio(&rec_geo);

    // The two recorded buffers must be identical to the source buffers
    if let Some(dbg_msg) = dbg_buffers(&rec_geo, &audio_buffer_geo) {
        eprint!("Error: geo:\n{dbg_msg} ");
        dbg!()
    }

    let zero_runs_rec_geo = find_zeros(&rec_geo);
    let zero_runs_audio_buffer_geo = find_zeros(&audio_buffer_geo);
    if !zero_runs_rec_geo.is_empty() || !zero_runs_audio_buffer_geo.is_empty() {
        dbg!(zero_runs_rec_geo);
        dbg!(zero_runs_audio_buffer_geo);
        panic!("rec_geo or audio_buffer_geo have zero runs");
    }

    assert_eq!(rec_geo.len(), audio_buffer_geo.len());

    for i in 0..audio_buffer_geo.len() {
        assert!((rec_geo[i] - audio_buffer_geo[i]).abs() < f32::EPSILON);
    }

    // Good so far.  The recorded buffer matches the input.  Now
    // check the recorded files are the same

    let audio_path = app_data.file_manager.make_paths().unwrap().0;
    let metadata_path = app_data.file_manager.make_paths().unwrap().1;
    let metadata: Metadata = match read_file_metadata(metadata_path) {
        Ok(p) => p,
        Err(err) => panic!("{err}"),
    };
    let channels = metadata.channels;
    assert_eq!(channels, 1);
    assert_eq!(metadata.sample_rate, get_sample_rate());
    if let Ok(audio_buffer) = read_f32_vec_from_file(&audio_path, channels) {
        let recovered_geo = audio_buffer.get_buffer(0).unwrap();
        let recovered_geo = trim_audio(&recovered_geo);
        assert_eq!(recovered_geo.len(), rec_geo.len());
        for i in 0..rec_geo.len() {
            assert!((rec_geo[i] - recovered_geo[i]).abs() < f32::EPSILON);
        }
    } else {
        panic![];
    }

    // Test playing back the audio files from the previous test.
    // Playback is simulated by having the Jack client sink audio int
    // a shared buffer
    let shared = Arc::new(Mutex::new(Vec::<f32>::new()));
    // The client takes it as a vector
    let buffers = vec![shared];
    // The copy kept here (`.to_vec()` calls `clone` on cotents)
    let buffers_shared = buffers.to_vec();

    let port_names = vec!["playback_1".to_string()];
    let client_name = "test_play_client";
    let ac = make_test_play_client(client_name, port_names, buffers).unwrap();
    let client_name = ac.as_client().name();
    let port_names = ac
        .as_client()
        .ports(Some(client_name), None, PortFlags::IS_INPUT);

    // The pipes from the player being tested to the Jack client
    // sinking audio in the shared buffers
    let mut outputs = JackPipes::new(false);
    for p in port_names.iter() {
        outputs.add(p).unwrap();
    }

    let mut app_data = match App::initialise(JackPipes::new(true), outputs, &output_path, true) {
        Ok(a) => a,
        Err(err) => panic!("Cannot initalise AppData: {err}"),
    };

    // This blocks.
    app_data.handle_kommand(Command::Play).unwrap();

    // Check the buffers are the same
    let file_path = match app_data.file_manager.make_paths() {
        Ok(pp) => pp.0,
        Err(err) => panic!("FileManager.make_paths(): {err}"),
    };
    let metadata_path = app_data.file_manager.make_paths().unwrap().1;
    let metadata: Metadata = match read_file_metadata(metadata_path) {
        Ok(p) => p,
        Err(err) => panic!("{err}"),
    };
    let channels = metadata.channels;
    assert_eq!(channels, 1);
    assert_eq!(metadata.sample_rate, get_sample_rate());

    let audio_buffers = match read_f32_vec_from_file(&file_path, channels) {
        Ok(p) => p,
        Err(err) => panic!("{err}"),
    };
    let ab_0 = trim_audio(audio_buffers.get_buffer_idx(0).unwrap());
    let zero_runs = find_zeros(&ab_0);
    if !zero_runs.is_empty() {
        panic!("ab_0 has zero runs: {zero_runs:?}");
    }
    {
        let bf_0 = trim_audio(&buffers_shared[0].lock().unwrap());
        let zero_runs = find_zeros(&bf_0);
        if !zero_runs.is_empty() {
            panic!("bf_0 has zero runs: {zero_runs:?}");
        }
        dbg!(&bf_0.len());
        assert_eq!(ab_0.len() as i32 - bf_0.len() as i32, 0);
        for idx in 0..ab_0.len() {
            let a = ab_0[idx];
            let b = bf_0[idx];
            assert!((a - b).abs() < f32::EPSILON);
        }
    }
}

#[test]
/// Record one channel of audio.
/// Save it to disc.
fn record_audio() {
    for d in AUDIO_DURATION.iter() {
        let audio_duration = *d;

        // The test audio
        let audio_buffer = generate_test_audio(220, 0.25, audio_duration, WaveForm::Sine);
        let audio_buffer = trim_audio(&audio_buffer);

        // The Jack client playing the test audio to be recorded
        let port_name = "record_audio";
        let client_name = "integration_test";
        let (ac, play_audio_flag, _, _) =
            play_test_audio(client_name, vec![port_name], vec![&audio_buffer]);

        // The port to record audio data from
        let port_name_complete = format!("{}:{port_name}", ac.as_client().name());

        // File path for recorded audio.  There will be two files with
        // suffixes "raw" and "json" for audio data and metadata
        // respectively
        let output_path = dst_dir().join("record_audio");

        // Set up recorder
        let mut recorder = set_up_recorder(vec![port_name_complete.clone()], &output_path);

        // Record data from `port_name`
        if let Err(err) = recorder.handle_record() {
            panic!("Called handle_recording(): {err}");
        }

        // Start the test signal
        play_audio_flag.store(true, Ordering::SeqCst);

        // Wait for audio to stop
        thread::sleep(Duration::from_millis(audio_duration as u64));

        let mut loop_cnt = 0_u64;
        let delay_ms = 100;
        const LOOP_LIM: u64 = 10;
        loop {
            loop_cnt += 1;
            if !play_audio_flag.load(Ordering::SeqCst) {
                break;
            }
            if loop_cnt >= LOOP_LIM {
                panic!(
                    "Audio has not stopped playing: {}ms elapsed",
                    loop_cnt * delay_ms
                );
            }
            thread::sleep(Duration::from_millis(delay_ms));
        }

        if let Err(err) = recorder.handle_audio_stop() {
            panic!("Could not stop audio: {err}");
        }

        // Get data out of the recorder
        let new_buffer = trim_audio(&recorder.recorded_audio.get_buffer(0).unwrap());
        // The buffers should be the same length
        assert_eq!(audio_buffer.len(), new_buffer.len());

        // The buffers should be the same exactly
        for i in 0..audio_buffer.len() {
            assert!((audio_buffer[i] - new_buffer[i]).abs() < f32::EPSILON);
        }
        // Check the saved data
        let mut result = true;

        let file_path = match recorder.file_manager.make_paths() {
            Ok(p) => p.0,
            Err(err) => panic!("{err}"),
        };
        let metadata_path = recorder.file_manager.make_paths().unwrap().1;
        let metadata: Metadata = match read_file_metadata(metadata_path) {
            Ok(p) => p,
            Err(err) => panic!("{err}"),
        };
        let channels = metadata.channels;
        assert_eq!(channels, 1);
        assert_eq!(metadata.sample_rate, get_sample_rate());
        match read_f32_vec_from_file(&file_path, channels) {
            Ok(d) => {
                let imported_data = trim_audio(&d.get_buffer(0).unwrap());
                if imported_data.len() != new_buffer.len() {
                    eprintln!(
                        "* Error: recovered_tri.len()/{} != rec_tri.len()/{} audio_duration: {audio_duration}",
                        imported_data.len(),
                        new_buffer.len()
                    );
                    result = false;
                } else {
                    for i in 0..new_buffer.len() {
                        if (new_buffer[i] - imported_data[i]).abs() > f32::EPSILON {
                            eprintln!(
                                "* Error: Recorded tri differs at {i} audio_duration: {audio_duration}"
                            );
                            result = false;
                            break;
                        }
                    }
                }
            }
            Err(err) => panic!(
                "Failed to read data from {output_path:?}.  Error: {err} audio_duration: {audio_duration}"
            ),
        };
        assert!(result);
    }
}
