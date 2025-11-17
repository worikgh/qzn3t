// Copyright (c) 2025 Worik Turei Stanton
// License: GPL-3.0

use anyhow::Result; // TODO: Get rid of this
use clap::Parser;
use qzn3t_recorder::app::App;
use qzn3t_recorder::app::AppData;
use qzn3t_recorder::io::Inputs;
use qzn3t_recorder::send_audio_to_jack::send_audo_to_jack;
use qzn3t_recorder::structs::Args;
use qzn3t_recorder::structs::Command;
use qzn3t_recorder::ui::ui_loop;
use std::error::Error;
use std::sync::mpsc;

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    let mut inputs = Inputs::new();
    let input = &args.input;
    let parts: Vec<&str> = input.split(':').collect();
    if parts.len() != 2 {
        return Err(Box::from("Input must be in the format 'client:port'"));
    }
    inputs.add_input(&args.input)?;

    // Channel to send audio data to Jackd
    let (audio_tx, audio_rx) = mpsc::channel::<f32>();

    // The app is controlled through a channel with the front end UI
    let (command_tx, command_rx) = mpsc::channel::<Command>();

    // The main programme runs in `App`
    let mut app = App::new();
    let file_name = format!("{}/{}", args.directory, args.file_name);

    // Start the application.  Runs in its own thread, the handle is in `app_handle`
    match args.kommand {
        None => {
            let app_data: AppData = app.initialise(
                audio_tx,
                command_rx,
                inputs.names().first().unwrap(),
                file_name,
                args.raw,
            )?;
            let _out_port = send_audo_to_jack("output", audio_rx, app_data.audio_run.clone())?;
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
                args.input.as_str(),
                file_name,
                args.raw,
            )?;
            cfg.handle_kommand(k)?;
            Ok(())
        }
    }
}
