// Copyright (c) 2026 Worik Turei Stanton
// License: GPL-3.0

//! Test Qzn3t in the whole.
use jack::{
    AsyncClient, AudioIn, AudioOut, Client, ClientOptions, Control,
    NotificationHandler, Port, PortFlags, ProcessHandler,
};
use qzn3t_audio_buffer::AudioBuffer;
use qzn3t_engine::{Engine, Session};
use std::{
    f32,
    fs::{OpenOptions, create_dir_all},
    io::Write,
    path::PathBuf,
    sync::{Arc, Mutex},
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
        let (client, _s) = Client::new(
            "qzn3t_integration_test",
            ClientOptions::NO_START_SERVER,
        )
        .unwrap();
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

    fn port_names(&self) -> Vec<String> {
        self.client
            .as_client()
            .ports(
                Some(self.client.as_client().name()),
                None,
                PortFlags::empty(),
            )
            .iter()
            .map(|p| p.to_string())
            .collect::<Vec<String>>()
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

#[allow(unused)]
fn test_signal_sine(sample_cnt: usize) -> Vec<f32> {
    (0..(2 * sample_cnt))
        .map(|n| {
            let n = 2.0 * f32::consts::PI * n as f32 / sample_cnt as f32 - 1.0;
            n.sin()
        })
        .collect()
}

#[allow(unused)]
fn test_signal_triangle(sample_cnt: usize) -> Vec<f32> {
    let modlp = sample_cnt / 4;
    let ret: Vec<f32> = (0..sample_cnt)
        .map(|n| 2.0 * (n % modlp) as f32 / modlp as f32 - 1.0)
        .collect();

    let s = ret.iter().fold("".to_string(), |a, b| format!("{a}{b}\n"));
    let mut f = OpenOptions::new()
        .truncate(true)
        .create(true)
        .write(true)
        .open("/tmp/foobar")
        .unwrap();
    f.write_all(s.as_bytes()).unwrap();
    ret
}

/// Test data consisting of linear data from 1.0 to -1.0 using
/// `sample_cnt` samples
#[allow(unused)]
fn test_signal_linear_falling(sample_cnt: usize) -> Vec<f32> {
    let mut ret = test_signal_linear_rising(sample_cnt);
    ret.reverse();
    ret
}

fn make_test_file(name: &str) -> PathBuf {
    let p = dst_dir();
    if !p.exists() {
        create_dir_all(&p).unwrap();
    }
    assert!(p.is_dir());
    p.join(name)
}

#[test]
fn play_by_step() {
    // let data_len = 10;
    let data_len = 48_025;

    let test_data_0 = test_signal_triangle(data_len);
    let test_data_1 = test_signal_triangle(data_len);
    let audio_data =
        AudioBuffer::new_data(vec![test_data_0.clone(), test_data_1.clone()])
            .unwrap();
    let channels = audio_data.channels();

    let jack_sink = JackSink::new(audio_data.channels());
    assert!(!jack_sink.client.as_client().name().is_empty());

    let sink_port_names = jack_sink.port_names();
    let out_ports_strings: Vec<String> = (0..sink_port_names.len())
        .map(|i| format!("out_{i}"))
        .collect();
    let out_ports_str: Vec<&str> =
        out_ports_strings.iter().map(|i| i.as_str()).collect();

    let p = make_test_file("play_by_step");
    let session = Session::new(&[], &out_ports_str, &p);
    let mut engine = Engine::new().unwrap();
    engine.start_session(session).unwrap();

    let player = engine.get_player(audio_data);
    engine.add_stepper(Box::new(player));
    engine.connect_outputs(&sink_port_names).unwrap();
    engine.run().unwrap();

    {
        // Check if `test_data` is in all output channels. TODO:
        // Test with different data in the channels
        let ab = jack_sink.output.lock().unwrap();
        let test_cl = |one: &[f32], another: &[f32]| -> bool {
            // Remove all zeros.  This destroys the integrity of
            // audio, but strips leading and trailing zeros, and
            // the results should then be identical.  Of course if
            // the two differ by zeros inserted into one f the
            // tracks this will not detect that.
            let one =
                one.iter().filter(|&f| f.abs() > 0.0).collect::<Vec<&f32>>();
            let another = another
                .iter()
                .filter(|&f| f.abs() > 0.0)
                .collect::<Vec<&f32>>();
            !one.iter()
                .zip(another.iter())
                .any(|(&a, &b)| (a - b).abs() > f32::EPSILON)
        };

        // Examine what the jack sink got
        for c in 0..channels {
            let channel_data = ab.get_channel(c).unwrap();
            if !test_cl(&channel_data, &test_data_0) {
                panic!("Channel {c} not same as test_data");
            }
        }
    }
}
