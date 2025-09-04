// Copyright (c) 2025 Worik Turei Stanton
// License: GPL-3.0

//! Record all Jack audio channels playing audio output.  Output on
//! stdout the sample rate and list of output files in JSON format

use chrono::Utc;
use clap::{Arg, ArgAction, Command};
use serde::Serialize;
use std::env;
use std::ffi::OsString;
use std::fs::File;
use std::io::BufWriter;
use std::io::prelude::*;
use std::io::{self};
use std::path::Path;
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
    eprintln!("DBG jack_rec matches: {matches:?}");

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
        .unwrap_or_else(|| vec![]); // Default if no values provided
    MyArgs {
        pipes,
        prefix: prefix.to_string(),
    }
}

fn main() {
    #[derive(Serialize)]
    struct Description {
        sample_rate: usize,
        output_files: Vec<String>,
    }

    let args = process_args();
    let prefix = args.prefix;
    // Create client
    let (client, _status) =
        jack::Client::new("qzn3t_jack_rec", jack::ClientOptions::NO_START_SERVER).unwrap();
    // The `in_ports` that match "system:playback" are the audio output

    // `description` contains the paths to the generated files and the
    // sample rate.  It is converted to JSON and output on the stdout
    // when the recording is finished.  It is all that is needed to
    // convert the files from raw audio to a more usable format.
    let mut description = Description {
        sample_rate: client.sample_rate(),
        output_files: vec![],
    };

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

    // Create a client that writes all data to a file, for each port
    // that is being monitored
    let mut clients = vec![];
    for name in ports.iter() {
        let name = name.replace('/', "_");
        let (client, _status) =
            jack::Client::new("qzt", jack::ClientOptions::NO_START_SERVER).expect("Client qzt");
        let spec = jack::AudioIn;
        let inport = client.register_port(&name, spec).unwrap();
        let to_port = inport.name().as_ref().unwrap().to_string();
        let fname = format!("{prefix}_{name}.raw");
        eprintln!("DBG jack_re: Create file '{fname}' from port: '{name}'");
        let fpath = Path::new(&fname);
        let file = match File::create(fpath) {
            Ok(f) => f,
            Err(e) => panic!("Error jack_rec: Cannot create: {fname}  Err: {e}"),
        };
        description.output_files.push(fname);

        // This writer gets moved into the closure
        let mut writer = BufWriter::new(file);
        let process_callback =
            move |_jc: &jack::Client, ps: &jack::ProcessScope| -> jack::Control {
                // Called every time there is data available
                let in_a_p: &[f32] = inport.as_slice(ps);
                for v in in_a_p {
                    let bytes = v.to_ne_bytes();
                    writer.write_all(&bytes).unwrap();
                }

                // Is this needed?  No.  `writer` goes out ouf scope
                // when the Jack client is shut down with `deactivate`
                //writer.flush().unwrap();
                jack::Control::Continue
            };

        let process = jack::ClosureProcessHandler::new(process_callback);
        // Activate the client, which starts the processing.
        let active_client = client.activate_async(Notifications, process).unwrap();
        let from_port = name;

        let (client, _status) =
            jack::Client::new("qzn3t", jack::ClientOptions::NO_START_SERVER).expect("Client qzn3t");
        match client.connect_ports_by_name(from_port.as_str(), to_port.as_str()) {
            Ok(()) => (),
            Err(err) => {
                eprintln!("Failed  {from_port} -> {} '{err}'", to_port);
            }
        };
        clients.push(active_client);
    }
    let mut input = String::new();

    // Block on stdin, effectively a keypress
    io::stdin().read_line(&mut input).unwrap();
    for client in clients {
        client.deactivate().unwrap();
    }
    let json_str = serde_json::to_string_pretty(&description).unwrap();
    print!("{json_str}");
}

struct Notifications;

impl jack::NotificationHandler for Notifications {
    fn sample_rate(&mut self, _: &jack::Client, srate: jack::Frames) -> jack::Control {
        println!("JACK: sample rate changed to {srate}");
        jack::Control::Continue
    }
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
