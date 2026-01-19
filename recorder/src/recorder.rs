// Copyright (c) 2025 Worik Turei Stanton
// License: GPL-3.0

use clap::Parser;
use qzn3t_recorder::app::App;
use qzn3t_recorder::app::AppData;
use qzn3t_recorder::errors::RecorderError;
use qzn3t_recorder::io::JackPipes;
use qzn3t_recorder::send_audio_to_jack::send_audo_to_jack;
use qzn3t_recorder::structs::Args;
use qzn3t_recorder::structs::Command;
use qzn3t_recorder::ui::ui_loop;
use std::env::temp_dir;
use std::sync::mpsc;

fn inner_main(args: Args) -> Result<(), RecorderError> {
    let inputs = JackPipes::from_command_line(&args.inputs, true)?;
    let outputs = JackPipes::from_command_line(&args.outputs, false)?;
    // Channel to send audio data to Jackd
    let (audio_tx, audio_rx) = mpsc::channel::<f32>();

    // The app is controlled through a channel with the front end UI
    let (command_tx, command_rx) = mpsc::channel::<Command>();

    // The main programme runs in `App`
    let mut app = App;

    // Directory recordings go to
    let dir = if args.directory.is_some() {
        args.directory.as_ref().unwrap().into()
    } else {
        temp_dir()
    };

    // Start the application.  Runs in its own thread, the handle is in `app_handle`
    match args.kommand {
        None => {
            let file_path = dir.join(args.file_name.as_str());

            let app_data: AppData =
                app.initialise(audio_tx, command_rx, inputs, outputs, &file_path)?;
            let _out_port = send_audo_to_jack(audio_rx, app_data.run_f.clone())?;
            let ui_run = app_data.ui_run_f.clone();
            let t = app.run(app_data)?;

            // The audio output.  Stays valid so long as `_out_port` exists.
            ui_loop(&command_tx, ui_run)?;

            _ = t.join();
            Ok(())
        }
        Some(k) => {
            let file_path = dir.join(args.file_name.as_str());
            let mut cfg: AppData =
                app.initialise(audio_tx, command_rx, inputs, outputs, &file_path)?;
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
    fn invalid_input_pipe() {
        // No client:port
        let mut args = Args::default();
        let pname = "abcdefg";
        args.inputs.push(pname.to_string());
        let test = inner_main(args);
        assert!(matches!(test, Err(RecorderError::InvalidPipeName(ref s)) if s == pname));

        // Too many fields
        let mut args = Args::default();
        let pname = "abc:defg:1234:trwge";
        args.inputs.push(pname.to_string());
        let test = inner_main(args);
        assert!(matches!(test, Err(RecorderError::InvalidPipeName(ref s)) if s == pname));
    }

    #[test]
    fn invalid_pipe_type() {
        let mut args = Args::default();
        let pname = "system:playback_1";
        args.inputs.push(pname.to_string());
        let test = inner_main(args);
        assert!(matches!(test, Err(RecorderError::NotOutputPipe(ref s)) if s == pname));
    }

    #[test]
    fn non_exist_pipe() {
        let mut args = Args::default();
        args.inputs.push("asdfdg:1234".to_string());
        let test = inner_main(args);
        assert!(matches!(test, Err(RecorderError::PipeNotFound(_))));
    }

    #[test]
    fn duplicate_pipe() {
        let mut args = Args::default();
        args.inputs.push("system:capture_1".to_string());
        args.inputs.push("system:capture_1".to_string());
        let test = inner_main(args);
        assert!(matches!(test, Err(RecorderError::DuplicateInput(_))));
    }
}
