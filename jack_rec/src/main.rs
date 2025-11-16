// Copyright (c) 2025 Worik Turei Stanton
// License: GPL-3.0

//! Record all Jack audio channels playing audio output.  Output on
//! stdout the sample rate and list of output files in JSON format

use chrono::Utc;
use clap::{Arg, ArgAction, Command};
use jack_rec::Description;
use std::collections::HashMap;
use std::env;
use std::error::Error;
use std::ffi::OsString;
use std::fs::File;
use std::io::BufWriter;
use std::io::prelude::*;
use std::io::{self};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc;
use std::time::Duration;
struct MyArgs {
    pipes: Vec<String>,
    prefix: String,
}
fn process_args() -> MyArgs {
    process_args_inner(env::args_os())
}
fn process_args_inner<I, T>(args: I) -> MyArgs
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let matches = Command::new("jack_rec")
        .arg(
            Arg::new("prefix")
                .short('p')
                .long("prefix")
                .value_name("PREFIX")
                .help("The output file prefix. Specified at most once.")
                .action(ArgAction::Set),
        )
        .arg(
            Arg::new("jackd_pipe")
                .short('i')
                .long("jackd-pipe")
                .value_name("PIPE_NAME")
                .help("Name of Jackd pipes to monitor. Can occur zero, one or many times.")
                .action(ArgAction::Append) // This allows multiple occurrences
                .num_args(1), // This specifies that each occurrence takes one value
        )
        .get_matches_from(args);

    // Get the prefix value (uses default if not provided)
    let prefix = match matches.get_one::<String>("prefix") {
        Some(p) => p.to_string(),
        None => format!("{:?}", Utc::now().format("%Y-%m-%d_%H-%M-%S").to_string()),
    };
    println!("Prefix: {}", prefix);

    // Get all occurrences of the jackd_pipe argument
    let pipes: Vec<String> = matches
        .get_many::<String>("jackd_pipe")
        .map(|values| values.cloned().collect())
        .unwrap_or_default();
    if !pipes.is_empty() {
        let dbg_str = pipes.iter().fold("".to_string(), |a, b| format!("{a} {b}"));
        eprintln!("DBG jack_rec: pipes: {dbg_str}",);
    }
    MyArgs {
        pipes,
        prefix: prefix.to_string(),
    }
}

use mio::{Events, Interest, Poll, Token};
use std::os::unix::io::AsRawFd;
const STDIN_TOKEN: Token = Token(0);
fn main() -> Result<(), Box<dyn Error>> {
    let args = process_args();
    let prefix = args.prefix;

    // Create client
    let (client, _status) =
        jack::Client::new("qzn3t_jack_rec", jack::ClientOptions::NO_START_SERVER).unwrap();

    // Get all ports to collect data from matching "system:playback"
    let ports = if args.pipes.is_empty() {
        let system_playback =
            client.ports(Some("system:playback"), None, jack::PortFlags::IS_INPUT);

        // Filter the output ports.  Keep any that are connected to a
        // "system:playback" port.
        let ports: Vec<String> = client
            .ports(None, None, jack::PortFlags::IS_OUTPUT)
            .iter()
            .filter(|p| {
                let outport = client.port_by_name(p.as_str()).unwrap();
                system_playback
                    .to_vec()
                    .iter()
                    .any(|name| outport.is_connected_to(name.as_str()).unwrap())
            })
            .cloned()
            .collect::<Vec<String>>();
        ports
    } else {
        args.pipes
    };

    // `description` contains the paths to the generated files and the
    // sample rate.  It is converted to JSON and output on the stdout
    // when the recording is finished.  It is all that is needed to
    // convert the files from raw audio to a more usable format.
    let mut description = Description {
        sample_rate: client.sample_rate(),
        output_files: vec![],
    };

    // Create a client that writes all data to a file, for each port
    // that is being monitored
    let mut clients = vec![];

    // The mape the name of the stream to the channel receiving audio
    let mut channels: HashMap<String, mpsc::Receiver<f32>> = HashMap::new();

    // The flag that shuts down `jack_rec`
    let kill_flag = Arc::new(AtomicBool::new(false));

    for name in ports.iter() {
        let name = name.replace('/', "_").to_string();

        let (sender, receiver) = mpsc::channel::<f32>();
        channels.insert(name.clone(), receiver);
        let async_client =
            jack_rec::run_port("qzn3t".to_string(), name, sender, kill_flag.clone())?;
        clients.push(async_client);
    }

    // Make the BufWriters to write the audio data to files
    let mut writers: HashMap<String, BufWriter<File>> = HashMap::new();
    for (name, _) in channels.iter() {
        // The path for the audio data to be written to
        let fname = format!("{prefix}_{name}.raw");
        let fpath = Path::new(&fname);
        let file = match File::create(fpath) {
            Ok(f) => f,
            Err(e) => panic!("Error jack_rec: Cannot create: {fname}  Err: {e}"),
        };
        description.output_files.push(fname.clone());
        // This writer writes the data from the port
        let writer = BufWriter::new(file);
        writers.insert(name.to_string(), writer);
    }

    let mut poll = Poll::new()?;
    let stdin = io::stdin();
    let stdin_fd = stdin.as_raw_fd();

    // Register stdin for read events
    poll.registry().register(
        &mut mio::unix::SourceFd(&stdin_fd),
        STDIN_TOKEN,
        Interest::READABLE,
    )?;

    let mut events = Events::with_capacity(128);

    loop {
        // Write all the data from all the channels
        for (name, channel) in channels.iter() {
            let writer = writers.get_mut(name).unwrap();
            while let Ok(b) = channel.try_recv() {
                writer.write_all(&b.to_ne_bytes())?;
            }
            writer.flush()?;
        }

        // Check if a key pressed
        poll.poll(&mut events, Some(Duration::from_millis(0)))?;
        if !events.is_empty() {
            break;
        }
    }

    for client in clients {
        client.deactivate().unwrap();
    }
    let json_str = serde_json::to_string_pretty(&description).unwrap();
    print!("{json_str}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;

    // Helper function to create test arguments
    fn create_args(args: Vec<&str>) -> Vec<OsString> {
        args.into_iter().map(OsString::from).collect()
    }

    #[test]
    fn test_no_arguments() {
        let args = create_args(vec!["jack_rec"]);
        let result = process_args_inner(args);

        // Should have default prefix (timestamp format) and default pipe
        assert!(result.prefix.contains("202") || result.prefix.contains('-'));
        assert_eq!(result.pipes.len(), 0);
    }

    #[test]
    fn test_custom_prefix() {
        let args = create_args(vec!["jack_rec", "-p", "my_custom_prefix"]);
        let result = process_args_inner(args);

        assert_eq!(result.prefix, "my_custom_prefix");
        assert_eq!(result.pipes.len(), 0);
    }

    #[test]
    fn test_custom_prefix_long_option() {
        let args = create_args(vec!["jack_rec", "--prefix", "another_prefix"]);
        let result = process_args_inner(args);

        assert_eq!(result.prefix, "another_prefix");
        assert_eq!(result.pipes.len(), 0);
    }

    #[test]
    fn test_single_pipe() {
        let args = create_args(vec!["jack_rec", "-i", "pipe1"]);
        let result = process_args_inner(args);

        assert!(result.prefix.contains("202") || result.prefix.contains('-'));
        assert_eq!(result.pipes, vec!["pipe1".to_string()]);
    }

    #[test]
    fn test_single_pipe_long_option() {
        let args = create_args(vec!["jack_rec", "--jackd-pipe", "pipe2"]);
        let result = process_args_inner(args);

        assert!(result.prefix.contains("202") || result.prefix.contains('-'));
        assert_eq!(result.pipes, vec!["pipe2".to_string()]);
    }

    #[test]
    fn test_multiple_pipes() {
        let args = create_args(vec![
            "jack_rec", "-i", "pipe1", "-i", "pipe2", "-i", "pipe3",
        ]);
        let result = process_args_inner(args);

        assert!(result.prefix.contains("202") || result.prefix.contains('-'));
        assert_eq!(
            result.pipes,
            vec![
                "pipe1".to_string(),
                "pipe2".to_string(),
                "pipe3".to_string()
            ]
        );
    }

    #[test]
    fn test_multiple_pipes_long_option() {
        let args = create_args(vec![
            "jack_rec",
            "--jackd-pipe",
            "pipeA",
            "--jackd-pipe",
            "pipeB",
        ]);
        let result = process_args_inner(args);

        assert!(result.prefix.contains("202") || result.prefix.contains('-'));
        assert_eq!(result.pipes, vec!["pipeA".to_string(), "pipeB".to_string()]);
    }

    #[test]
    fn test_both_prefix_and_pipes() {
        let args = create_args(vec![
            "jack_rec",
            "-p",
            "test_prefix",
            "-i",
            "pipe1",
            "-i",
            "pipe2",
        ]);
        let result = process_args_inner(args);

        assert_eq!(result.prefix, "test_prefix");
        assert_eq!(result.pipes, vec!["pipe1".to_string(), "pipe2".to_string()]);
    }

    #[test]
    fn test_mixed_long_and_short_options() {
        let args = create_args(vec![
            "jack_rec",
            "--prefix",
            "mixed_prefix",
            "-i",
            "short_pipe",
            "--jackd-pipe",
            "long_pipe",
        ]);
        let result = process_args_inner(args);

        assert_eq!(result.prefix, "mixed_prefix");
        assert_eq!(
            result.pipes,
            vec!["short_pipe".to_string(), "long_pipe".to_string()]
        );
    }

    #[test]
    fn test_empty_pipe_list() {
        // This should use the default pipe since no pipe arguments are provided
        let args = create_args(vec!["jack_rec", "-p", "only_prefix"]);
        let result = process_args_inner(args);

        assert_eq!(result.prefix, "only_prefix");
        assert_eq!(result.pipes.len(), 0);
    }

    #[test]
    fn test_program_name_only() {
        let args = create_args(vec!["jack_rec"]);
        let result = process_args_inner(args);

        // Should have defaults for both
        assert!(result.prefix.contains("202") || result.prefix.contains('-'));
        assert_eq!(result.pipes.len(), 0);
    }

    #[test]
    fn test_multiple_identical_pipes() {
        let args = create_args(vec!["jack_rec", "-i", "same", "-i", "same", "-i", "same"]);
        let result = process_args_inner(args);

        assert_eq!(
            result.pipes,
            vec!["same".to_string(), "same".to_string(), "same".to_string()]
        );
    }
}
