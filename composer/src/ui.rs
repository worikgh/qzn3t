// Copyright (c) 2025 Worik Turei Stanton
// License: GPL-3.0

//! The user interface for Qzn3t Composer
use crate::structs::Command;
#[allow(dead_code, unused_imports)]
use crossterm::{
    ExecutableCommand, cursor,
    event::{self, Event, KeyCode},
    style::{self, Color, Stylize},
    terminal::{disable_raw_mode, enable_raw_mode},
};
use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::io::{self, Write};
/// Match commands to text do display
pub struct UI {
    // Match numbers to commands.  This is created by the `display` method and read by....
    index: HashMap<usize, Command>,
    // The status to display
    status: Option<String>,
    // The last command returned by user interaction
    last_command: Option<Command>,
}
impl UI {
    pub fn new() -> Self {
        Self {
            index: HashMap::new(),
            status: None,
            last_command: None,
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
            eprintln!("DBG composer: UI.display Using `last_command`");
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
                    self.last_command = Some(command.clone());
                    Ok(command.clone())
                } else {
                    Err(UIError::BadChoice(c))
                }
            } else {
                Err(UIError::BadChoice(c))
            }
        } else {
            eprintln!("DBG composer: UI.get_command: {e:?}: This should not be reachable");
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
