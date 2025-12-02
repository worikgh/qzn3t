// Copyright (c) 2025 Worik Turei Stanton
// License: GPL-3.0

use anyhow::Result; // TODO: Get rid of this
use clap::Parser;
use qzn3t_recorder::app::App;
use qzn3t_recorder::app::AppData;
use qzn3t_recorder::errors::RecorderError;
use qzn3t_recorder::io::Inputs;
use qzn3t_recorder::send_audio_to_jack::send_audo_to_jack;
use qzn3t_recorder::structs::Args;
use qzn3t_recorder::structs::Command;
use qzn3t_recorder::ui::ui_loop;
use std::sync::mpsc;

fn inner_main(args: Args) -> Result<(), RecorderError> {
    let mut inputs = Inputs::new();
    if args.input.is_empty() {
        return Err(RecorderError::NoInputs);
    }
    for i in args.input.iter() {
        let parts: Vec<&str> = i.split(':').collect();
        if parts.len() < 2 || parts.len() > 3 {
            return Err(RecorderError::InvalidPipeName(i.to_string()));
        } else if parts.len() == 2 {
            inputs.add(i)?;
        } else {
            let client = parts[0];
            let port = parts[1];
            let name = parts[2];
            let client_port = format!("{client}:{port}");
            inputs.add_name(&client_port, &name)?;
        }
    }
    // Channel to send audio data to Jackd
    let (audio_tx, audio_rx) = mpsc::channel::<f32>();

    // The app is controlled through a channel with the front end UI
    let (command_tx, command_rx) = mpsc::channel::<Command>();

    // The main programme runs in `App`
    let mut app = App;

    // Start the application.  Runs in its own thread, the handle is in `app_handle`
    match args.kommand {
        None => {
            let app_data: AppData = app.initialise(
                audio_tx,
                command_rx,
                inputs,
                if args.directory.is_some() {
                    Some(args.directory.as_ref().unwrap().into())
                } else {
                    None
                },
                args.raw,
            )?;
            let _out_port = send_audo_to_jack("output", audio_rx, app_data.recorder_run.clone())?;
            let ui_run = app_data.ui_run.clone();
            let t = app.run(app_data)?;

            // The audio output.  Stays valid so long as `_out_port` exists.
            ui_loop(&command_tx, ui_run)?;

            _ = t.join();
            Ok(())
        }
        Some(k) => {
            let mut cfg: AppData = app.initialise(
                audio_tx,
                command_rx,
                inputs,
                if args.directory.is_some() {
                    Some(args.directory.as_ref().unwrap().into())
                } else {
                    None
                },
                args.raw,
            )?;
            cfg.handle_kommand(k)?;
            Ok(())
        }
    }
}

fn main() {
    let args = Args::parse();
    if let Err(err) = inner_main(args) {
        panic!("Error in recorder: {err}");
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_no_input() {
        let args = Args::default();
        let test = inner_main(args);
        eprintln!("{test:?}");
        assert!(matches!(test, Err(RecorderError::NoInputs)));
    }

    #[test]
    fn invalid_input_pipe() {
        let mut args = Args::default();
        args.input.push("abcdefg".to_string());
        let test = inner_main(args);
        eprintln!("{test:?}");
        assert!(matches!(test, Err(RecorderError::InvalidPipeName(_))));
    }

    #[test]
    fn invalid_pipe_type() {
        let mut args = Args::default();
        args.input.push("system:playback_1".to_string());
        let test = inner_main(args);
        eprintln!("{test:?}");
        assert!(matches!(test, Err(RecorderError::NotOutputPipe(_))));
    }

    #[test]
    fn non_exist_pipe() {
        let mut args = Args::default();
        args.input.push("asdfdg:1234".to_string());
        let test = inner_main(args);
        eprintln!("{test:?}");
        assert!(matches!(test, Err(RecorderError::PipeNotFound(_))));
    }

    #[test]
    fn duplicate_pipe() {
        let mut args = Args::default();
        args.input.push("system:capture_1".to_string());
        args.input.push("system:capture_1".to_string());
        let test = inner_main(args);
        eprintln!("{test:?}");
        assert!(matches!(test, Err(RecorderError::DuplicateInput(_))));
    }
}
