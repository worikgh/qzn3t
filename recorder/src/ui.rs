// Copyright (c) 2025 Worik Turei Stanton
// License: GPL-3.0

//! The user interface for Qzn3t Recorder
use crate::errors::RecorderError;
use crate::structs::Command;

use crossterm::{
    ExecutableCommand, cursor,
    event::{self, Event, KeyCode},
    style::{self, Color, Stylize},
    terminal::{disable_raw_mode, enable_raw_mode},
};
use std::io::{self, Write};
use std::{collections::HashMap, sync::mpsc::Sender};
use std::{
    error::Error,
    sync::{Arc, atomic::AtomicBool},
};
use std::{fmt, sync::atomic::Ordering};

#[derive(Debug)]
enum State {
    Recording,
    Reviewing,
    AtRest,
}
/// Match commands to text do display
pub struct UI {
    // Match numbers to commands.  This is created by the `display` method and read by....
    index: HashMap<usize, Command>,
    // The status to display
    status: Option<String>,
    // The last command returned by user interaction
    last_command: Option<Command>,

    state: State,
}
impl UI {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            index: HashMap::new(),
            status: None,
            last_command: None,
            state: State::AtRest,
        }
    }
    /// Display the menu.  Depends on the crate `crossterm`
    pub fn display(&mut self, selected: Option<Command>) {
        // The commands that are offered to the user
        let available_commands = [
            Command::Record,
            Command::ReviewRecord,
            Command::Stop,
            Command::Quit,
        ];

        // If there has been a command selected before
        // (`last_command.is_some()`) and `selected` paramter is
        // `None` use the last command
        let selected = if selected.is_some() {
            selected
        } else if self.last_command.is_some() {
            self.last_command.clone()
        } else {
            None
        };

        // Clear screen and move cursor to top (crossterm)
        print!("\x1B[2J\x1B[1;1H");

        println!("=== Qzn3t Menu ===\n");

        for (i, item) in available_commands.iter().enumerate() {
            self.index.insert(i, item.clone());

            if Some(item) == selected.as_ref() {
                println!("\r> {i} {} <", item);
            } else {
                println!("\r  {i} {}", item);
            }
        }

        println!(
            "\n\r{}",
            style::Print(format!("{:?}", self.state))
                .0
                .clone()
                .with(Color::Red)
        );
        if self.status.is_some() {
            println!(
                "\n{}",
                style::Print(self.status.as_ref().unwrap())
                    .0
                    .clone()
                    .with(Color::Red)
            );
        } else {
            println!(); // Empty line if no status
        }
        io::stdout().flush().unwrap();
    }

    /// Maintain the UI's state.  This assumes that all commands sent
    /// to the backend succeed.
    fn state_transition(cmd: &Command, state: &State) -> Result<State, RecorderError> {
        // Quit is always a valid command
        if cmd == &Command::Quit {
            return Ok(State::AtRest);
        }

        match state {
            State::Recording | State::Reviewing => {
                if *cmd == Command::Stop {
                    Ok(State::AtRest)
                } else {
                    Err(RecorderError::BadCommand(cmd.clone()))
                }
            }
            State::AtRest => match cmd {
                Command::Record => Ok(State::Recording),
                Command::ReviewRecord => Ok(State::Reviewing),
                _ => Ok(State::AtRest),
            },
        }
    }

    /// Get a command from the UI.  Blocks until the user makes an action.
    pub fn get_command(&mut self) -> Result<Command, UIError> {
        let e = event::read().map_err(|err| UIError::Fatal(format!("{err}")))?;
        if let Event::Key(event) = e {
            let c = if let KeyCode::Char(c) = event.code {
                c
            } else {
                return Err(UIError::Fatal(format!(
                    "Error UI.get_command({event:?}) unknown"
                )));
            };
            if let Some(i) = c.to_digit(10) {
                let i = i as usize;
                if let Some(command) = self.index.get(&i) {
                    match Self::state_transition(command, &self.state) {
                        Ok(state) => {
                            eprintln!("DBG recorder: State change {:?} -> {:?}", self.state, state);
                            self.state = state;
                            self.last_command = Some(command.clone());
                            Ok(command.clone())
                        }
                        Err(err) => {
                            if err == RecorderError::BadCommand(command.clone()) {
                                Err(UIError::BadChoice(c))
                            } else {
                                panic!("Impossible error {err:?}")
                            }
                        }
                    }
                } else {
                    Err(UIError::BadChoice(c))
                }
            } else {
                Err(UIError::BadChoice(c))
            }
        } else {
            eprintln!("Error recorder: UI.get_command: {e:?}: This should not be reachable");
            Ok(Command::Continue)
        }
    }

    pub fn set_up_screen() -> Result<(), Box<dyn Error>> {
        // Enable raw mode for direct key reading
        enable_raw_mode()?;

        // Setup terminal
        let mut stdout = io::stdout();
        stdout.execute(cursor::Hide)?;
        Ok(())
    }

    pub fn cleanup_screen() -> Result<(), Box<dyn Error>> {
        let mut stdout = io::stdout();
        stdout.execute(cursor::Show)?;
        disable_raw_mode()?;
        Ok(())
    }
}

// impl Default for UI {
//     fn default() -> Self {
//         Self::new()
//     }
// }

/// Errors fo rthe user interface.  There are two sorts of error:
/// UIFatal and UIBadChoice
#[derive(Debug)]
pub enum UIError {
    Fatal(String),
    BadChoice(char),
}

impl Error for UIError {}
impl fmt::Display for UIError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let msg = match self {
            UIError::Fatal(err) => format!("UIError::Fatal: {err}"),
            UIError::BadChoice(msg) => format!("UIError::BadChoice: {} is not a valid choice", msg),
        };
        write!(f, "{msg}")
    }
}

/// The UI loop
pub fn ui_loop(
    command_tx: &Sender<Command>,
    ui_run: Arc<AtomicBool>,
) -> Result<(), Box<dyn Error>> {
    // The user interface...
    let mut ui = UI::new();
    let _ = UI::set_up_screen();
    loop {
        ui.display(None);
        if !ui_run.load(Ordering::SeqCst) {
            break;
        }

        let command = match ui.get_command() {
            Ok(c) => c,
            Err(uierr) => match uierr {
                UIError::BadChoice(_) => {
                    eprintln!("{uierr}");
                    continue;
                }
                UIError::Fatal(err) => return Err(err.into()),
            },
        };
        command_tx.send(command.clone())?;
        if command == Command::Quit {
            break;
        }
    }
    let _ = UI::cleanup_screen();
    Ok(())
}
