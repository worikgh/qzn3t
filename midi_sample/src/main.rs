extern crate jack;
extern crate midir;
extern crate serde;
extern crate symphonia;
use jack::contrib::ClosureProcessHandler;
use jack::ClientStatus;
use jack::{Client, Control};
use midir::{MidiInput, MidiInputConnection};
use rubato::{
    Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType,
};
use serde::de::{self, Visitor};
use serde::Deserialize;
use std::env;
use std::fmt;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::sync::mpsc::channel;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::Sender;
use std::thread;
use std::time::Duration;
use symphonia::core::audio::{SampleBuffer, SignalSpec};
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::errors::Error;
use symphonia::core::formats::{FormatOptions, Track};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

/// There need to be enough of these that there is always one channel
/// available.  If long samples (that tie up a channel) are being
/// played in quick succession each new (long) sample ties up another
/// channel.  The symptom is sample playing continues after triggering
/// stops as the backlog is processed.  Nothing gets dropped.
const NUM_RECEIVERS: usize = 300;

struct U8Visitor;

impl<'de> Visitor<'de> for U8Visitor {
    type Value = u8;

    fn expecting(
        &self,
        f: &mut fmt::Formatter,
    ) -> fmt::Result {
        write!(
            f,
            "a u8 as a number or string (hex with optional '0x' prefix)"
        )
    }

    fn visit_u64<E: de::Error>(
        self,
        v: u64,
    ) -> Result<u8, E> {
        v.try_into()
            .map_err(|_| E::custom(format!("Error midi_sample: Failure processing configuration: value {v} too large for u8",)))
    }

    fn visit_str<E: de::Error>(
        self,
        s: &str,
    ) -> Result<u8, E> {
        let s = s.trim(); // Handle whitespace
        if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X"))
        {
            u8::from_str_radix(hex, 16)
        } else {
            s.parse::<u8>()
        }
        .map_err(|e| E::custom(e.to_string()))
    }
}

fn deserialize_u8<'de, D>(deserializer: D) -> Result<u8, D::Error>
where
    D: de::Deserializer<'de>,
{
    deserializer.deserialize_any(U8Visitor)
}
/// Each sample is described by a path to an audio file and a MIDI
/// note
#[derive(Debug, Deserialize)]
struct SampleDescr {
    path: String,
    #[serde(deserialize_with = "deserialize_u8")]
    note: u8,
}

/// The programme is initialised with a JSON representation of this
#[derive(Debug, Deserialize)]
struct Config {
    samples_descr: Vec<SampleDescr>,
}

/// Each sample is converted to a `Vec<32>` buffer and a MIDI note on
/// start up.  When the MIDI note is received the buffer is played on
/// the output
struct SampleData {
    data: Vec<f32>,
    note: u8,
}

/// The configuration file  processing
fn process_samples_json(
    file_path: &str
) -> Result<Vec<SampleDescr>, Box<dyn std::error::Error>> {
    // Read the JSON file
    let mut contents = String::new();
    let mut file = File::open(file_path)?;
    file.read_to_string(&mut contents)
        .expect("Error midi_sample: Failed to read configuration file");

    // Convert JSON
    let mut config: Config = match serde_json::from_str(&contents) {
        Ok(s) => s,
        Err(err) => {
            panic!("Error midi_sample: Processing {file_path} JSON: {err}")
        },
    };
    for p in config.samples_descr.iter_mut() {
        let sample_path: &Path = Path::new(&p.path);
        if sample_path.is_relative() {
            // Relative to `file_path` make it absolute
            let path = Path::new(file_path).parent().unwrap().join(sample_path);
            p.path = path.to_str().unwrap().to_string();
        }
    }

    Ok(config.samples_descr)
}

/// Resample the samples from disc.  This is done during
/// initialisation
fn resample(
    data: &[f32],
    input_rate: usize,
    output_rate: usize,
) -> Vec<f32> {
    let channels = 1;
    let chunk_size = 1024;
    let ratio = output_rate as f64 / input_rate as f64;

    let sinc_params = SincInterpolationParameters {
        sinc_len: 256, // Length of the sinc kernel (higher = better quality, slower)
        f_cutoff: 0.95, // Cutoff frequency (0.0 to 1.0, typically 0.95 for anti-aliasing)
        oversampling_factor: 256, // Oversampling for better interpolation
        window: rubato::WindowFunction::BlackmanHarris2, // Window function for smoothing
        interpolation: SincInterpolationType::Linear,    // Interpolation method
    };

    let mut resampler = SincFixedIn::new(
        ratio,
        2.0, // quality
        sinc_params,
        chunk_size,
        channels,
    )
    .unwrap();

    let mut output = Vec::new();
    let mut remaining_samples = data;

    // Process complete chunks
    while remaining_samples.len() >= chunk_size {
        let (chunk, rest) = remaining_samples.split_at(chunk_size);
        let out_chunk = resampler
            .process(&[chunk], None)
            .unwrap_or_else(|e| panic!("Resampling failed: {}", e));
        output.extend(&out_chunk[0]);
        remaining_samples = rest;
    }

    // Process remaining partial chunk (if any)
    if !remaining_samples.is_empty() {
        // Pad with zeros to complete the chunk
        let mut padded_chunk = remaining_samples.to_vec();
        padded_chunk.resize(chunk_size, 0.0);
        let out_chunk = resampler
            .process(&[&padded_chunk], None)
            .unwrap_or_else(|e| panic!("Resampling failed: {}", e));
        output.extend(&out_chunk[0]);
    }

    output
}

fn main() {
    // Get and process command line arguments.
    let args: Vec<String> = env::args().collect();
    let samples_descr: Vec<SampleDescr> =
        match process_samples_json(args[1].as_str()) {
            Ok(sd) => sd,
            Err(err) => {
                panic!("Error midi_sample: Failed to process input: {err}")
            },
        };

    // Prepare the sample buffers.  This code is from the Symphonia
    // example
    let mut sample_data: Vec<SampleData> = vec![];
    for SampleDescr { path, note } in samples_descr {
        // Create a media source. Note that the MediaSource trait is
        // automatically implemented for File, among other types.
        let file = Box::new(match File::open(path.as_str()) {
            Ok(f) => f,
            Err(err) => {
                panic!("Error midi_sample: Failed to open {path}: {err}")
            },
        });

        // Create the media source stream using the boxed media source from above.
        let mss = MediaSourceStream::new(file, Default::default());

        // Create a hint to help the format registry guess what format
        // reader is appropriate. In this example we'll leave it empty.
        let hint = Hint::new();

        // Use the default options when reading and decoding.
        let format_opts: FormatOptions = Default::default();
        let metadata_opts: MetadataOptions = Default::default();
        let decoder_opts: DecoderOptions = Default::default();

        // Probe the media source stream for a format.
        let probed = match symphonia::default::get_probe().format(
            &hint,
            mss,
            &format_opts,
            &metadata_opts,
        ) {
            Ok(p) => p,
            Err(e) => panic!("Error midi_sample: Failed probe {path}: {e}"),
        };

        // Get the format reader yielded by the probe operation.
        let mut format_reader = probed.format;

        // Get the default track.
        let track: &Track = format_reader.default_track().unwrap();

        // Create a decoder for the track.
        let mut decoder = symphonia::default::get_codecs()
            .make(&track.codec_params, &decoder_opts)
            .unwrap();

        // Store the track identifier, we'll use it to filter packets.
        let track_id = track.id;

        let mut sample_count = 0;
        let mut sample_buf: Option<SampleBuffer<f32>> = None;
        let mut data: Vec<f32> = vec![];

        loop {
            // Get the next packet from the format reader.
            if let Ok(packet) = format_reader.next_packet() {
                // If the packet does not belong to the selected track, skip it.
                if packet.track_id() != track_id {
                    continue;
                }

                // Decode the packet into audio samples, ignoring any decode errors.
                match decoder.decode(&packet) {
                    Ok(audio_buf) => {
                        // The decoded audio samples may now be
                        // accessed via the audio buffer if
                        // per-channel slices of samples in their
                        // native decoded format is desired. Use-cases
                        // where the samples need to be accessed in an
                        // interleaved order or converted into another
                        // sample format, or a byte buffer is
                        // required, are covered by copying the audio
                        // buffer into a sample buffer or raw sample
                        // buffer, respectively. In the example below,
                        // we will copy the audio buffer into a sample
                        // buffer in an interleaved order while also
                        // converting to a f32 sample format.

                        // If this is the *first* decoded packet,
                        // create a sample buffer matching the decoded
                        // audio buffer format.
                        if sample_buf.is_none() {
                            // Get the audio buffer specification.
                            let spec: SignalSpec = *audio_buf.spec();
                            eprintln!(
                                "DBG midi_sample: Path: {path} Sample rate: {}",
                                spec.rate
                            );
                            // Get the capacity of the decoded
                            // buffer. Note: This is capacity, not
                            // length!
                            let duration = audio_buf.capacity() as u64;

                            // Create the f32 sample buffer.
                            sample_buf =
                                Some(SampleBuffer::<f32>::new(duration, spec));
                        }

                        // Copy the decoded audio buffer into the sample
                        // buffer in an interleaved format.
                        if let Some(buf) = &mut sample_buf {
                            buf.copy_interleaved_ref(audio_buf);

                            // The samples may now be access via the
                            // `samples()` function.
                            sample_count += buf.samples().len();
                            data.append(&mut buf.samples().to_vec());
                        }
                    },
                    Err(Error::DecodeError(_)) => (),
                    Err(_) => break,
                }

                continue;
            }
            break;
        }

        // Extract the file name part of the sample to output some
        // stats.
        let disp_path = if let Some(idx) = path.rfind('/') {
            path.get((idx + 1)..).unwrap()
        } else {
            path.as_str()
        };
        eprintln!(
            "DBG midi_sample: Note -> Sample file: {note:X} -> {disp_path}  Size: {sample_count}"
        );

        // Store prepared sample
        sample_data.push(SampleData { data, note });
    }

    // Prepare the channels for sending data from the MIDI thread to
    // the Jack thread
    let mut senders: Vec<Sender<f32>> = Vec::new();
    let mut receivers: Vec<Receiver<f32>> = Vec::new();
    for _i in 0..NUM_RECEIVERS {
        let (sx, rx) = channel();
        senders.push(sx.clone());
        receivers.push(rx);
    }

    // Create the Jack client
    let (client, status) = match Client::new(
        "MidiSampleQzn3t",
        jack::ClientOptions::NO_START_SERVER,
    ) {
        Ok(a) => a,
        Err(err) => {
            panic!("Error midi_sample: Failed to create Jack client: {err}")
        },
    };

    if status != ClientStatus::empty() {
        panic!("Error midi_sample: Failed to create Jack client.  Invalid status: {status:?}");
    }
    let mut port = client.register_port("output", jack::AudioOut::default());

    // Activate the Jack client and start the audio processing thread
    let _as_client = client
        .activate_async(
            (),
            ClosureProcessHandler::new(
                move |_c: &Client, ps: &jack::ProcessScope| -> Control {
                    let output = port.as_mut().unwrap().as_mut_slice(ps);

                    for sample in output.iter_mut() {
                        let mut f: f32 = 0.0;
                        for r in receivers.iter() {
                            if let Ok(_f) = r.try_recv() {
                                // Mixing the channels together
                                f += _f;
                            }
                        }

                        // Unsure if this is the thing to do.  `tanh`
                        // is almost linear except in the extremes
                        // where it assymptotically approaches -1 and
                        // 1
                        // if f > 1.0 || f < -1.0 {
                        //     eprintln!(
                        //         "Sample is: {f}.  Adjusting too: {}",
                        //         f.tanh()
                        //     );
                        // }
                        *sample = f.tanh();
                    }
                    Control::Continue
                },
            ),
        )
        .unwrap();

    // Create a virtual midi port to read in data
    let lpx_midi = MidiInput::new("MidiSampleQzn3t").unwrap();
    let in_ports = lpx_midi.ports();
    let in_port = in_ports.first().ok_or("no input port available").unwrap();

    // Index the clousre below maintains for output clients
    let mut idx = 0;
    let _conn_in: MidiInputConnection<()> = lpx_midi
        .connect(
            in_port,
            "midi_input",
            move |_stamp, message: &[u8], _| {
                // let message = MidiMessage::from_bytes(message.to_vec());

                if message.len() == 3 && message[0] == 144 {
                    // All MIDI notes from LPX start with 144, for initial
                    // noteon and noteoff
                    let velocity = message[2];
                    if velocity != 0 {
                        // NoteOn
                        // eprintln!("DBG midi_sample: Message: {message:?}");
                        if let Some(sample) =
                            sample_data.iter().find(|s| s.note == message[1])
                        {
                            // Get the volume as a f32 fraction
                            let volume: f32 = message[2] as f32 / 127.0;
                            for f in sample.data.iter() {
                                senders
                                    .get(idx)
                                    .unwrap()
                                    .send(*f * volume)
                                    .unwrap();
                            }

                            idx += 1;
                            idx %= senders.len();
                        }
                    }
                }
            },
            (),
        )
        .unwrap();
    loop {
        thread::sleep(Duration::from_secs(1_000));
    }
}
