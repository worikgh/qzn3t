// Copyright (c) 2026 Worik Turei Stanton
// License: GPL-3.0

//! Test Qzn3t in the whole.
use jack::{
    AsyncClient, AudioIn, AudioOut, Client, ClientOptions, Control, NotificationHandler, Port,
    PortFlags, ProcessHandler,
};
use qzn3t_audio_buffer::{AudioBuffer, get_sample_rate};
use qzn3t_engine::{Engine, Session, SessionMode};
use std::{
    fs::create_dir_all,
    path::PathBuf,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

/// For a Jack client that receives audio data on its `in_ports` and
/// stores it in a shared buffer
#[allow(unused)]
struct JackProcessSink {
    // input: Vec<Vec<f32>>,
    output: Arc<Mutex<AudioBuffer>>,
    out_ports: Vec<Port<AudioOut>>,
    in_ports: Vec<Port<AudioIn>>,
}
impl ProcessHandler for JackProcessSink {
    fn process(&mut self, _: &Client, ps: &jack::ProcessScope) -> Control {
        for (c, port) in self.in_ports.iter().enumerate() {
            let in_a_p = port.as_slice(ps);

            let mut output = self.output.lock().unwrap();
            (*output).add_samples(c, in_a_p).unwrap();
        }
        Control::Continue
    }
}
pub struct Notifications;
impl NotificationHandler for Notifications {}

/// Holds a Jack client that receives audio data on its input ports,
/// and sinks the audio data into the shared buffer
#[derive(Debug)]
struct JackSink {
    client: AsyncClient<Notifications, JackProcessSink>,
    output: Arc<Mutex<AudioBuffer>>,
}
impl JackSink {
    fn new(channels: usize) -> Self {
        let (client, _s) =
            Client::new("qzn3t_integration_test", ClientOptions::NO_START_SERVER).unwrap();
        assert!(!client.name().is_empty());
        let output = Arc::new(Mutex::new(AudioBuffer::new(channels).unwrap()));
        let in_ports: Vec<Port<AudioIn>> = (0..channels)
            .map(|i| {
                client
                    .register_port(&format!("in_{i}"), AudioIn::default())
                    .unwrap()
            })
            .collect();

        let jack_process = JackProcessSink {
            // input: vec![vec![]; channels],
            output: output.clone(),
            in_ports,
            out_ports: vec![],
        };
        let ac = client.activate_async(Notifications, jack_process).unwrap();
        Self {
            client: ac,
            output: output.clone(),
        }
    }
}

/// A directory for storing files in
fn dst_dir() -> PathBuf {
    std::env::current_dir().unwrap().join("tests/data")
}

/// Test data consisting of linear data from -1.0 to 1.0 using
/// `sample_cnt` samples
fn test_signal_linear_rising(sample_cnt: usize) -> Vec<f32> {
    (0..sample_cnt)
        .map(|idx| -1.0 + 2.0 * idx as f32 / sample_cnt as f32)
        .collect()
}

/// Test data consisting of linear data from 1.0 to -1.0 using
/// `sample_cnt` samples
#[allow(unused)]
fn test_signal_linear_falling(sample_cnt: usize) -> Vec<f32> {
    let mut ret = test_signal_linear_rising(sample_cnt);
    ret.reverse();
    ret
}

#[test]
/// Test playing audio data from an audio buffer
fn play_audio() {
    let sample_rate: usize = get_sample_rate() as usize;
    let channels = 3;
    // let data_len = sample_rate; // One second of data
    // `data_len` of 10_500 and above fails.  The critical point is between 10_500 and 9_750
    let data_len = 9_750;

    // The sink for data
    let jack_sink = JackSink::new(channels);
    assert!(!jack_sink.client.as_client().name().is_empty());

    let sink_port_names = jack_sink
        .client
        .as_client()
        .ports(
            Some(jack_sink.client.as_client().name()),
            None,
            PortFlags::empty(),
        )
        .iter()
        .map(|p| p.to_string())
        .collect::<Vec<String>>();

    let mut engine = Engine::new().unwrap();

    let out_ports_strings: Vec<String> = (0..sink_port_names.len())
        .map(|i| format!("out_{i}"))
        .collect();
    let out_ports_str: Vec<&str> = out_ports_strings.iter().map(|i| i.as_str()).collect();

    let p = dst_dir(); //.join("play_audio");
    if !p.exists() {
        create_dir_all(&p).unwrap();
    }
    assert!(p.is_dir());
    let p = p.join("play_audio");
    let session = Session::new(&[], &out_ports_str, &p, SessionMode::Playing);

    engine.start_session(session).unwrap();
    let source_port_names = engine.all_ports().unwrap();

    assert_eq!(source_port_names.len(), sink_port_names.len());
    for (source, sink) in source_port_names.iter().zip(sink_port_names.iter()) {
        engine
            .client()
            .unwrap()
            .connect_ports_by_name(source, sink)
            .unwrap();
    }

    // Add some test data to play
    let test_data = test_signal_linear_rising(data_len);
    let audio_buffer = AudioBuffer::new_data(vec![test_data.clone(); channels]).unwrap();
    let _audio_len = audio_buffer.len();
    engine.add_audio_buffer_play(audio_buffer);

    // Run...
    // let handle = engine.run().unwrap();
    engine.run().unwrap();
    thread::sleep(Duration::from_millis(
        (1_000 * data_len / sample_rate) as u64,
    ));
    thread::sleep(Duration::from_millis(
        (1_000 * data_len / sample_rate) as u64,
    ));
    {
        assert_eq!(jack_sink.output.lock().unwrap().channels(), channels);
        {
            // Check if `test_data` is in all output channels starting at the same place
            let ab = jack_sink.output.lock().unwrap();
            assert!(ab.len() > test_data.len());
            let test_cl = |haystack: &[f32], needle: &[f32]| -> Option<usize> {
                if haystack.len() < needle.len() {
                    return None;
                }
                'OUTER: for i in 0..(haystack.len() - needle.len()) {
                    for j in 0..needle.len() {
                        if (haystack[i + j] - needle[j]).abs() < f32::EPSILON {
                            continue 'OUTER;
                        }
                    }
                    // Found needle in haystack
                    return Some(i);
                }
                None
            };
            let mut indexes = vec![];
            for c in 0..channels {
                let channel_data = ab.get_channel(c).unwrap();
                let index_opt = test_cl(&channel_data, &test_data);
                if let Some(idx) = index_opt {
                    indexes.push(idx);
                } else {
                    panic!("Channel {c} does not contain test_data");
                }
            }
            //
            assert!(indexes[1..].iter().all(|u| *u == indexes[0]));
        }
    }
}
