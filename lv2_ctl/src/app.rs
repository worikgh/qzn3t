//! # Run the user interface
//! Addapted from  Ratatui List example
//!
//! [Ratatui]: https://github.com/ratatui-org/ratatui
//! [examples]: https://github.com/ratatui-org/ratatui/blob/main/examples
//! [examples readme]: https://github.com/ratatui-org/ratatui/blob/main/examples/README.md
use crate::colours::HEADER_BG;
use crate::colours::NORMAL_ROW_COLOR;
use crate::colours::SELECTED_STYLE_FG;
use crate::colours::TEXT_COLOR;
use crate::lv2::Port;
use crate::lv2::PortType;
use crate::lv2::{Lv2, ModHostController};
use crate::lv2_simulator::Lv2Simulator;
use crate::lv2_simulator::Status;
use crate::lv2_stateful_list::Lv2StatefulList;
use crate::port_table::port_table;
use color_eyre::config::HookBuilder;
use crossterm::{
    event::{self, Event, KeyCode},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{prelude::*, widgets::*};
use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::thread;
use std::time::{Duration, Instant};
use std::{error::Error, io, io::stdout};

/// Used by the scroll bar for LV2 Controls (F2)
const ITEM_HEIGHT: usize = 4;

#[derive(Debug, PartialEq, Eq)]
enum AppState {
    List,    // Listing all simulators
    Command, // Interacting with mod-host
}

/// This struct holds the current state of the app.
pub struct App<'a> {
    // Data from mod-host
    buffer: String,

    // JACK audi Connections
    jack_connections: HashMap<String, String>,

    mod_host_controller: &'a ModHostController,

    // Maintain the view for the first screen.  Which simulators are
    // "loaded"
    lv2_stateful_list: Lv2StatefulList,

    // Maintain the view for the second screen of loaded simulators
    lv2_loaded_list: Lv2StatefulList,

    app_state: AppState,

    // Internal state to prevent complaining too much about
    // unrecogised responses from mod-host
    unrecognised_resp: HashSet<String>,

    /// The Ports in the current view and the scrolling table they are
    /// displayed in
    ports: Vec<&'a Port>,
    table_state: TableState,
    scroll_bar_state: ScrollbarState,

    /// The last command sent to mod-host.  It is command orientated
    /// so a "resp..." from mod-host refers to the last command sent.
    /// This programme is asynchronous, so a command is sent, and
    /// later a response is received.  This allows the two to be
    /// connected.  When a response is received set this back to None.
    last_mh_command: Option<String>,

    /// Commands are queued when they arrive.  They are sent in the
    /// order they are received.
    mh_command_queue: VecDeque<String>,

    /// Store the last status output so do not thrash status reporting
    /// mechanism (eprintln! as I write) with repeated status messages
    status: Option<String>,
}

impl Drop for App<'_> {
    /// Ensure the terminal is returned whole to the user on exit
    fn drop(&mut self) {
        restore_terminal().expect("Restore terminal");
    }
}

/// Ensure the terminal is returned whole to the user on panic
fn init_error_hooks() -> color_eyre::Result<()> {
    let (panic, error) = HookBuilder::default().into_hooks();
    let panic = panic.into_panic_hook();
    let error = error.into_eyre_hook();
    color_eyre::eyre::set_hook(Box::new(move |e| {
        let _ = restore_terminal();
        error(e)
    }))?;
    std::panic::set_hook(Box::new(move |info| {
        let _ = restore_terminal();
        panic(info)
    }));
    Ok(())
}

/// Set up the terminal for being a TUI
fn init_terminal() -> color_eyre::Result<Terminal<impl Backend>> {
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout());
    let terminal = Terminal::new(backend)?;
    Ok(terminal)
}

fn restore_terminal() -> color_eyre::Result<()> {
    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;
    Ok(())
}

impl App<'_> {
    /// Initialise the App
    pub fn new(mod_host_controller: &ModHostController) -> App {
        if let Err(err) = init_error_hooks() {
            eprintln!("1 INFO: {err}: Initialising error hooks");
        }

        // Vec<(name, url)>
        let types: Vec<(String, String)> = mod_host_controller
            .simulators
            .iter()
            .map(|s| (s.name.clone(), s.url.clone()))
            .collect();

        App {
            jack_connections: HashMap::new(),
            buffer: "".to_string(),
            app_state: AppState::List,
            mod_host_controller,
            lv2_loaded_list: Lv2StatefulList::empty(),
            lv2_stateful_list: Lv2StatefulList::new(&types),
            unrecognised_resp: HashSet::new(),

            table_state: TableState::default().with_selected(0),
            scroll_bar_state: ScrollbarState::default(),
            ports: vec![],

            last_mh_command: None,
            mh_command_queue: VecDeque::new(),

            status: None,
        }
    }

    fn status_string(&self) -> String {
        format!(
            "{:?} queued cmd# {} Last Command: {:?}",
            self.app_state,
            self.mh_command_queue.len(),
            self.last_mh_command
        )
    }

    /// Queue a command to send to mod-host
    fn send_mh_cmd(&mut self, cmd: &str) {
        self.mh_command_queue.push_back(cmd.to_string());
    }

    /// Called from the event loop to send a message to mod-host
    fn pump_mh_queue(&mut self) {
        if self.last_mh_command.is_none() && !self.mh_command_queue.is_empty() {
            // Safe because queue is not empty
            let cmd = self.mh_command_queue.pop_front().unwrap();

            eprintln!("CMD: {cmd}");
            self.last_mh_command = Some(cmd.trim().to_string());
            self.mod_host_controller
                .input_tx
                .send(cmd.as_bytes().to_vec())
                .expect("Send to mod-host");
        }
    }
    /// Changes the status of the selected list item.  
    fn change_status(&mut self) {
        eprintln!("2 INFO change_status: {:?}", self.app_state);
        match self.app_state {
            AppState::List => {
                if let Some(i) = self.get_stateful_list_mut().state.selected() {
                    // There is a selected item at index `i`

                    self.get_stateful_list_mut().items[i].status = match self
                        .get_stateful_list_mut()
                        .items[i]
                        .status
                    {
                        Status::Unloaded => {
                            // Set status of Lv2 to Loaded.  Put the
                            // simulator into the list of loaded
                            // simulators
                            let lv2: Lv2Simulator = self.lv2_stateful_list.items[i].clone();
                            eprintln!(
                                    "STATEREP change_status Unloaded -> Pending i({i}) url({}) mh_id({})",
                                    lv2.url, lv2.mh_id
                                );
                            let cmd = format!("add {} {}\n", lv2.url, lv2.mh_id);
                            self.send_mh_cmd(cmd.as_str());

                            // TODO:  Move this into the handler for received messages
                            self.lv2_loaded_list.items.insert(
                                if i > self.lv2_loaded_list.items.len() {
                                    self.lv2_loaded_list.items.len()
                                } else {
                                    i
                                },
                                lv2,
                            );
                            Status::Pending
                        }

                        // Set status of Lv2 to Unloaded.  Remove from the
                        // list of loaded simulators
                        Status::Loaded => {
                            if let Some(j) = self
                                .lv2_loaded_list
                                .items
                                .iter()
                                .position(|x| x.url == self.lv2_stateful_list.items[i].url)
                            {
                                let lv2: Lv2Simulator = self.lv2_stateful_list.items[i].clone();
                                let cmd = format!("remove {}\n", lv2.mh_id);
                                self.send_mh_cmd(cmd.as_str());

                                self.send_mh_cmd(cmd.as_str());
                                eprintln!(
                                    "STATEREP Remove Loaded -> Unloaded {:?}",
                                    self.lv2_loaded_list.items.remove(j)
                                );
                                // TODO: Move this into the handler
                                // for received messages.  This should
                                // be `Status::Pending
                                Status::Unloaded
                            } else {
                                panic!("A loaded LV2 was not om loaded_list.  {i}");
                            }
                        }
                        Status::Pending => Status::Pending,
                    }
                }
            }
            AppState::Command => {
                // Showing and editing the Port values for a LV2.  Change the LV2 simulator
                // TODO: Optimisation - check the same LV2 is not selected twice
                if let Some(idx) = self.get_stateful_list_mut().state.selected() {
                    // Connect the selected effect to system in/out
                    //eprintln!("Effect: effect_{idx}");

                    let mh_id = self.get_stateful_list().items[idx].mh_id;
                    //  Disconnect any existing connections.  This
                    //  connects one, and only one, LV2
                    let cmds = self
                        .jack_connections
                        .iter()
                        .map(|(l, r)| format!("disconnect {l} {r}"))
                        .collect::<Vec<String>>();
                    for cmd in cmds.iter() {
                        self.send_mh_cmd(cmd.as_str());
                    }

                    let mhc = &self.mod_host_controller;
                    let lv_url = &self.get_stateful_list().items[idx].url;
                    let lv2 = match mhc.get_lv2_url(lv_url) {
                        Some(l) => l,
                        None => panic!("Getting Lv2 by url: {}", lv_url),
                    };
                    // For each input audio port make a connection
                    let mut i = 1; // To name input ports system:capture_1....
                    for p in lv2
                        .ports
                        .iter()
                        .filter(|p| {
                            p.types.iter().any(|t| t == &PortType::Input)
                                && p.types.iter().any(|t| t == &PortType::Audio)
                        })
                        .collect::<Vec<&Port>>()
                        .iter()
                    {
                        let lhs = format!("system:capture_{i}");
                        let rhs = format!("effect_{mh_id}:{}", p.symbol.as_str());
                        let cmd = format!("connect {lhs} {rhs}");
                        self.send_mh_cmd(cmd.as_str());
                        i += 1;
                    }
                    // For each output audio port make a connection
                    let mut i = 1; // To name input ports system:capture_1....
                    for p in lv2
                        .ports
                        .iter()
                        .filter(|p| {
                            p.types.iter().any(|t| t == &PortType::Output)
                                && p.types.iter().any(|t| t == &PortType::Audio)
                        })
                        .collect::<Vec<&Port>>()
                        .iter()
                    {
                        let lhs = format!("effect_{mh_id}:{}", p.symbol.as_str());
                        let rhs = format!("system:playback_{i}");
                        let cmd = format!("connect {lhs} {rhs}");
                        self.send_mh_cmd(cmd.as_str());
                        i += 1;
                    }
                } else {
                    eprintln!("ERR Nothing selected!");
                }
            }
        }
    }

    fn go_top(&mut self) {
        self.get_stateful_list_mut().state.select(Some(0))
    }

    fn go_bottom(&mut self) {
        let len = self.get_stateful_list().items.len();
        self.get_stateful_list_mut().state.select(Some(len - 1))
    }

    /// Run the app
    pub fn run(mod_host_controller: &ModHostController) -> Result<(), Box<dyn Error>> {
        let terminal = init_terminal()?;
        let mut app = App::new(mod_host_controller);

        app._run(terminal).expect("Calling _run");

        restore_terminal()?;

        Ok(())
    }

    /// Set a status line
    fn set_status(&self, status: &str) {
        // No actual status yet
        eprintln!("3 INFO Status: {status}");
    }

    /// Handle a response from mod-host that starts with "resp ".
    /// There are at least these cases:
    ///
    /// 1. A LV2 simulator has just been added and 'resp N' with N
    /// being the "instance number" of the added simulator
    ///
    /// 2. A `param_get` command has been sent and get: 'resp N M'
    /// where N is the instance number, and M is the value of the
    /// parameter
    fn procss_resp(&mut self, response: &str) {
        // Can only get a "resp " from mod-host after a command has been sent
        let last_mh_command: String;
        {
            last_mh_command = self
                .last_mh_command
                .as_ref()
                .expect("The last command sent to mod-host is empty")
                .trim()
                .to_string();
        }
        // Get the first word as a slice
        let sp: usize = last_mh_command
            .chars()
            .position(|x| x.is_whitespace())
            .expect("No space in resp string");
        let fw_cmd = &last_mh_command[0..sp];
        match fw_cmd {
            "add" => {
                // Adding an LV2.  Get the instance number from the command
                let sp = last_mh_command
                    .rfind(' ')
                    .unwrap_or_else(|| panic!("Malformed command: '{last_mh_command}'"));
                let instance_number = last_mh_command[sp..].trim().parse::<usize>().unwrap_or_else(|_| {
                    panic!(
                        "No instance number at end of add command: '{last_mh_command}'  sp: {sp} => '{}'",
			&last_mh_command[sp..]
                    )
                });

                // Get a reference to the item in use.  Its state will
                // be modified: to `'oaded` if all is well, to
                // `unloaded` if mod-host returned an error
                let item: &mut Lv2Simulator = self
                    .lv2_stateful_list
                    .items
                    .iter_mut()
                    .find(|x| x.mh_id == instance_number)
                    .expect("Cannot find LV2 instance: {instance_number}");

                if let Ok(n) = response[5..].parse::<isize>() {
                    eprintln!("Got a idx: {n}");

                    if n >= 0 {
                        // If `n` > 0 it is the instance_number of the simulator
                        let n = n as usize; // Ok.  n >= 0
                        if n != instance_number {
                            panic!("Bad instance_number in `add` response: {n} is not {instance_number}");
                        }
                        item.status = Status::Loaded;
                    } else {
                        // Error code
                        let errno = n;
                        eprintln!("ERR: {errno}.  Command: {:?}", self.last_mh_command);
                        item.status = Status::Unloaded;
                    }
                    self.last_mh_command = None;
                }
            }
            "param_get" => {
                // Got the current value of a Port.  Get the symbol for the port from the command
                let sp = last_mh_command[10..] // Consider string after "param_get " prefix
                    .find(' ')
                    .expect("Malformed command: {last_mh_command}") + 10;
                let instance_number_str = last_mh_command[10..sp].trim();
                let instance_number = instance_number_str
                    .parse::<usize>()
                    .unwrap_or_else(|_| {
                        panic!(
                            "No instance number at end of param_get command: \
			     '{last_mh_command}' => '{instance_number_str}"
                        )
                    });

                let sp = last_mh_command
                    .rfind(' ')
                    .expect("Malformed command: {last_mh_command}");
                let symbol = last_mh_command[sp..].trim();

                // Get the instance number and the value from the response
                let r = &response[5..];
                let sp: usize = r
                    .chars()
                    .position(|x| x.is_whitespace())
                    .unwrap_or_else(|| panic!("Malformed response to param_get: {response}"));
                let n = r[..sp]
                    .trim()
                    .parse::<usize>()
                    .expect("Malformed response to param_get: Instance number invalid. {response}");
                if n != instance_number {
                    panic!(
                        "Bad instance_number in `param_get` response: {n} is not {instance_number}"
                    );
                }
                let value = r[sp..].trim();
                self.set_port_value(n, symbol, value);
                self.last_mh_command = None;
            }
            _ => panic!("Unknown command: {last_mh_command}"),
        };
    }

    /// This needs to set the value of the port where ever it is
    /// displayed.  Probably by maipulating self.ports.
    fn set_port_value(&mut self, instance_number: usize, symbol: &str, value: &str) {
        eprintln!("UNIMPLEMENTED: set_port_value(&mut self, {instance_number}, {symbol}, {value})");
    }

    /// If there are any results ready in buffer....
    fn process_buffer(&mut self) {
        // If there is no '\mn' in buffer, do not process it, leave it
        // till next time.
        while let Some(s) = self.buffer.as_str().find('\n') {

            // There is a line available
            let r = self.buffer.as_str()[0..s].trim().to_string();
            if !r.is_empty() {
                // Skip blank lines.
                //eprintln!("INFO m-h: {r}");
                if r.trim() == "mod-host>" || r.trim() == "using block size: 1024" {
                    // Unsure why we see these....
                    eprintln!("INFO m-h: {r}");
                } else if r.len() > 5 && &r.as_str()[0..5] == "resp " {
                    self.procss_resp(r.as_str());
                } else {
                    match &self.last_mh_command {
                        Some(s) => {
                            if format!("mod-host> {}",s.trim()).as_str() == r.trim(){
                                // All good mod-host repeats back commands
                                // Command is not complete yet
                            } else {
                                eprintln!("'{s}': Fuck you: '{r}'");
                            }
                        }
                        None => (),
                    };
                    if self.unrecognised_resp.insert(r.clone()) {
                        eprintln!("INFO Unrecognised: {r}");
                    }
                }
            }
            self.buffer = if s < self.buffer.len() {
                self.buffer.as_str()[(s + 1)..].to_string()
            } else {
                "".to_string()
            };
        }
    }

    /// The main body of the App
    fn _run(&mut self, mut terminal: Terminal<impl Backend>) -> io::Result<()> {
        // init_error_hooks().expect("App::run error hooks");

        let target_fps = 60; // 400 is about the limit on Raspberry Pi 5
        let frame_time = Duration::from_secs(1) / target_fps as u32;

	// Set this to false to make process exit on next loop
	let mut run = true;
        loop {
	    if !run {
		break;
	    }
            let start_time = Instant::now();
            let status = self.status_string();

            if let Some(st) = &self.status {
                if st != &status {
                    self.set_status(status.as_str());
                    self.status = Some(status);
                }
            } else {
                self.set_status(status.as_str());
                self.status = Some(status);
            }

            self.draw(&mut terminal)?;
            if event::poll(Duration::from_secs(0)).expect("Polling for event") {
                match event::read() {
                    Ok(Event::Key(key)) => {
                        use KeyCode::*;
                        match key.code {
                            Char('q') | Esc => {
                                self.send_mh_cmd("quit");
                                // Move this to handler of data from mod-host?
                                //return Ok(());
				run = false;
                            }
                            Char('u') | Left => self.get_stateful_list_mut().unselect(),
                            Down => self.get_stateful_list_mut().next(),
                            Up => self.get_stateful_list_mut().previous(),
                            Right | Enter => {
                                eprintln!("STATEREP _run call change_status");
                                self.change_status()
                            }
                            Char('g') => self.go_top(),
                            Char('G') => self.go_bottom(),

                            Char('n') => {
                                // In LV2 Control view (F2) move down
                                // on Port in Port display
                                if self.app_state == AppState::Command {
                                    // In LV2 Control view (F2) move down a port
                                    let i = match self.table_state.selected() {
                                        Some(i) => {
                                            if i >= self.ports.len() - 1 {
                                                0
                                            } else {
                                                i + 1
                                            }
                                        }
                                        None => 0,
                                    };
                                    self.table_state.select(Some(i));
                                    self.update_port_data(
                                        self.get_stateful_list().items
                                            [self.get_stateful_list().state.selected().unwrap()]
                                        .mh_id,
                                        self.ports[i].name.as_str(),
                                    );
                                    self.scroll_bar_state =
                                        self.scroll_bar_state.position(i * ITEM_HEIGHT);
                                }
                            }
                            Char('p') => {
                                // In LV2 Control view (F2) move down
                                // on Port in Port display
                                if self.app_state == AppState::Command {
                                    // In LV2 Control view (F2) move down
                                    let i = match self.table_state.selected() {
                                        Some(i) => {
                                            if i == 0 {
                                                self.ports.len() - 1
                                            } else {
                                                i - 1
                                            }
                                        }
                                        None => 0,
                                    };
                                    self.table_state.select(Some(i));
                                    self.scroll_bar_state =
                                        self.scroll_bar_state.position(i * ITEM_HEIGHT);
                                }
                            }
                            // Function keys for setting modes
                            F(1) => self.app_state = AppState::List,
                            F(2) => self.app_state = AppState::Command,
                            _ => {
                                eprintln!(
                                    "INFO Unrecognised key code: {:?} Modifier: {:?} Control: {}",
                                    key.code,
                                    key.modifiers,
                                    key.modifiers & crossterm::event::KeyModifiers::CONTROL
                                        == crossterm::event::KeyModifiers::CONTROL
                                );
                            }
                        }
                    }
                    Ok(Event::Resize(_, _)) => (),
                    Err(err) => panic!("{err}: Reading event"),
                    x => panic!("Error reading event: {x:?}"),
                };
            }

            // Send data to mod-host if it is enqueued
            self.pump_mh_queue();

            // Is there any data from mod-host
            if let Ok(Some(data)) = self.mod_host_controller.try_get_data() {
                self.buffer += data.as_str();
                self.process_buffer();
            }

            let elapsed_time = Instant::now() - start_time;
            if elapsed_time < frame_time {
                thread::sleep(frame_time - elapsed_time);
            } else {
                eprintln!("Timing error: {elapsed_time:?}/{frame_time:?}");
            }
        }
	Ok(())
    }

    fn draw(&mut self, terminal: &mut Terminal<impl Backend>) -> io::Result<()> {
        terminal.draw(|f| f.render_widget(self, f.size()))?;
        Ok(())
    }

    /// Get the StateFulList that is currently in view
    fn get_stateful_list(&self) -> &Lv2StatefulList {
        match self.app_state {
            AppState::List => &self.lv2_stateful_list,
            AppState::Command => &self.lv2_loaded_list,
        }
    }

    /// Get a mutable reference to the StateFulList that is currently
    /// in view
    fn get_stateful_list_mut(&mut self) -> &mut Lv2StatefulList {
        match self.app_state {
            AppState::List => &mut self.lv2_stateful_list,
            AppState::Command => &mut self.lv2_loaded_list,
        }
    }

    fn render_list(&mut self, area: Rect, buf: &mut Buffer) {
        // Create a space for header,  list and the footer.
        let vertical = Layout::vertical([
            Constraint::Length(2),
            Constraint::Min(0),
            Constraint::Length(2),
        ]);
        let [header_area, rest_area, footer_area] = vertical.areas(area);

        // Create two chunks with equal vertical screen space. One for the list and the other for
        // the info block.
        let vertical = Layout::vertical([Constraint::Percentage(50), Constraint::Percentage(50)]);
        let [upper_item_list_area, lower_item_list_area] = vertical.areas(rest_area);

        self.render_title(header_area, buf);
        self.render_lv2_list(upper_item_list_area, buf);
        self.render_details(lower_item_list_area, buf);
        self.render_footer(footer_area, buf);
    }

    fn render_selected_lv2(&mut self, area: Rect, buf: &mut Buffer) {
        // Create a space for header, todo list and the footer.
        let vertical = Layout::vertical([
            Constraint::Length(2),
            Constraint::Min(0),
            Constraint::Length(2),
        ]);
        let [header_area, rest_area, footer_area] = vertical.areas(area);

        // Create two chunks: Top chunk is (25%) is the list of
        // selected devices (that are loaded into mod-host with
        // `add...`) and the bottom 75% is for configuring the
        // simulator
        let vertical = Layout::vertical([Constraint::Percentage(25), Constraint::Percentage(75)]);
        let [upper_item_list_area, lower_item_list_area] = vertical.areas(rest_area);

        self.render_title(header_area, buf);
        self.render_lv2_list_selected(upper_item_list_area, buf);
        self.render_control_area(lower_item_list_area, buf);
        self.render_footer(footer_area, buf);
    }

    /// Ask mod-host what the value is for this port
    fn get_port_value(&mut self, index: usize, symbol: &str) {
        let cmd = format!("param_get {index} {symbol}");
        self.send_mh_cmd(cmd.as_str());
    }
}

impl Widget for &mut App<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        match self.app_state {
            AppState::List => self.render_list(area, buf),
            AppState::Command => self.render_selected_lv2(area, buf),
        };
    }
}

impl App<'_> {
    fn render_lv2(&self, lv2: &Lv2) -> String {
        format!("{lv2}")
    }

    fn render_title(&self, area: Rect, buf: &mut Buffer) {
        Paragraph::new("Qzn3t Lv2 Control")
            .centered()
            .render(area, buf);
    }

    fn render_lv2_list_selected(&mut self, area: Rect, buf: &mut Buffer) {
        // We create two blocks, one is for the header (outer) and the other is for list (inner).
        let outer_block = Block::default()
            .borders(Borders::NONE)
            .fg(TEXT_COLOR)
            .bg(HEADER_BG)
            .title("LV2 Simulators")
            .title_alignment(Alignment::Center);
        let inner_block = Block::default()
            .borders(Borders::NONE)
            .fg(TEXT_COLOR)
            .bg(NORMAL_ROW_COLOR);

        // We get the inner area from outer_block. We'll use this area later to render the table.
        let outer_area = area;
        let inner_area = outer_block.inner(outer_area);

        // We can render the header in outer_area.
        outer_block.render(outer_area, buf);

        // Iterate through all elements in the `items` and stylize them.
        let lv2_simulators: Vec<&Lv2Simulator> = self.lv2_stateful_list.items.iter().collect();
        let items: Vec<ListItem> = lv2_simulators
            .iter()
            .enumerate()
            .filter(|&l| l.1.status == Status::Loaded)
            .map(|(i, lv2_item)| lv2_item.to_static_list_item(i))
            .collect();

        // Create a List from all list items and highlight the currently selected one
        let items2 = List::new(items)
            .block(inner_block)
            .highlight_style(
                Style::default()
                    .add_modifier(Modifier::BOLD)
                    .add_modifier(Modifier::REVERSED)
                    .fg(SELECTED_STYLE_FG),
            )
            .highlight_symbol(">")
            .highlight_spacing(HighlightSpacing::Always);

        // We can now render the item list
        // (look careful we are using StatefulWidget's render.)
        // ratatui::widgets::StatefulWidget::render as stateful_render
        StatefulWidget::render(items2, inner_area, buf, &mut self.lv2_loaded_list.state);
    }

    fn render_lv2_list(&mut self, area: Rect, buf: &mut Buffer) {
        // We create two blocks, one is for the header (outer) and the other is for list (inner).
        let outer_block = Block::default()
            .borders(Borders::NONE)
            .fg(TEXT_COLOR)
            .bg(HEADER_BG)
            .title("LV2 Simulators")
            .title_alignment(Alignment::Center);
        let inner_block = Block::default()
            .borders(Borders::NONE)
            .fg(TEXT_COLOR)
            .bg(NORMAL_ROW_COLOR);

        // We get the inner area from outer_block. We'll use this area later to render the table.
        let outer_area = area;
        let inner_area = outer_block.inner(outer_area);

        // We can render the header in outer_area.
        outer_block.render(outer_area, buf);

        // Iterate through all elements in the `items` and stylize them.
        let items: Vec<ListItem> = self
            .lv2_stateful_list
            .items
            .iter()
            .enumerate()
            .map(|(i, todo_item)| todo_item.to_stateful_list_item(i))
            .collect();

        // Create a List from all list items and highlight the currently selected one
        let items = List::new(items)
            .block(inner_block)
            .highlight_style(
                Style::default()
                    .add_modifier(Modifier::BOLD)
                    .add_modifier(Modifier::REVERSED)
                    .fg(SELECTED_STYLE_FG),
            )
            .highlight_symbol(">")
            .highlight_spacing(HighlightSpacing::Always);

        // We can now render the item list
        // (look careful we are using StatefulWidget's render.)
        // ratatui::widgets::StatefulWidget::render as stateful_render
        StatefulWidget::render(items, inner_area, buf, &mut self.lv2_stateful_list.state);
    }

    fn render_details(&self, area: Rect, buf: &mut Buffer) {
        // We get the info depending on the item's state.
        let info = if let Some(i) = self.get_stateful_list().state.selected() {
            self.render_lv2(&self.mod_host_controller.simulators.as_slice()[i])
        } else {
            "Nothing to see here...".to_string()
        };

        // We show the list item's info under the list in this paragraph
        let outer_info_block = Block::default()
            .borders(Borders::NONE)
            .fg(TEXT_COLOR)
            .bg(HEADER_BG)
            .title("Details")
            .title_alignment(Alignment::Center);
        let inner_info_block = Block::default()
            .borders(Borders::NONE)
            .bg(NORMAL_ROW_COLOR)
            .padding(Padding::horizontal(1));

        // This is a similar process to what we did for list. outer_info_area will be used for
        // header inner_info_area will be used for the list info.
        let outer_info_area = area;
        let inner_info_area = outer_info_block.inner(outer_info_area);

        // We can render the header. Inner info will be rendered later
        outer_info_block.render(outer_info_area, buf);

        let info_paragraph = Paragraph::new(info)
            .block(inner_info_block)
            .fg(TEXT_COLOR)
            .wrap(Wrap { trim: false });

        // We can now render the item info
        info_paragraph.render(inner_info_area, buf);
    }

    fn update_port_data(&mut self, index: usize, symbol: &str) {
        self.get_port_value(index, symbol);
        eprintln!("Update {index} {symbol}");
    }

    // In screen 2 (F2) render the control details for the selected
    // LV2 simulator `area` is the screen real-estate that can be used
    fn render_control_area(&mut self, area: Rect, buf: &mut Buffer) {
        // We show the list item's info under the list in this paragraph
        let outer_info_block = Block::default()
            .borders(Borders::NONE)
            .fg(TEXT_COLOR)
            .bg(HEADER_BG)
            .title("LV2 Control")
            .bold()
            .title_alignment(Alignment::Center);

        let inner_info_area: Rect = outer_info_block.inner(area);

        // Render the controls into `inner_info_area`

        // Get the list of ports to render.  Want Control/Input ports
        self.ports = if let Some(i) = self.get_stateful_list().state.selected() {
            // The controls for this LV2

            let lv2 = &self.mod_host_controller.simulators.as_slice()[i];

            lv2.ports
                .iter()
                .filter(|p| {
                    p.types.iter().any(|t| matches!(t, PortType::Control(_)))
                        && p.types.contains(&PortType::Input)
                })
                .collect::<Vec<&Port>>()
        } else {
            vec![]
        };
        self.scroll_bar_state = ScrollbarState::new((self.ports.len()) * ITEM_HEIGHT);
        // Get the table widget
        let table = port_table(&self.ports);
        ratatui::widgets::StatefulWidget::render(
            table,
            inner_info_area,
            buf,
            &mut self.table_state,
        );
        ratatui::widgets::StatefulWidget::render(
            Scrollbar::default()
                .orientation(ScrollbarOrientation::VerticalRight)
                .begin_symbol(None)
                .end_symbol(None),
            inner_info_area,
            buf,
            &mut self.scroll_bar_state,
        );
        // table.render(inner_info_area, buf);

        // We can render the header. Inner info will be rendered later
        outer_info_block.render(area, buf);
    }

    fn render_footer(&self, area: Rect, buf: &mut Buffer) {
        match self.app_state {
            AppState::List => Paragraph::new(
                "Use ↓↑ to move, ← to unselect, → to change status, \
		 g/G to go top/bottom.\nAny other character to send instructions",
            ),
            AppState::Command => Paragraph::new(
                "Use ↓↑ to move, ← to unselect, → to change status.\n\
		 Any other characters fol to send instructions <Enter> to send",
            ),
        }
        .centered()
        .render(area, buf);
    }
}

impl Lv2StatefulList {
    fn next(&mut self) {
        let i = match self.state.selected() {
            Some(i) => {
                if (i + 1) >= self.items.len() {
                    i // `next` at bottom of list has no effect
                } else {
                    i + 1
                }
            }
            None => self.last_selected.unwrap_or(0),
        };
        self.state.select(Some(i));
    }

    fn previous(&mut self) {
        let i = match self.state.selected() {
            Some(i) => {
                if i == 0 {
                    0 // `previous` at top of list has no effect
                } else {
                    i - 1
                }
            }
            None => self.last_selected.unwrap_or(0),
        };
        self.state.select(Some(i));
    }

    fn unselect(&mut self) {
        let offset = self.state.offset();
        self.last_selected = self.state.selected();
        self.state.select(None);
        *self.state.offset_mut() = offset;
    }
}
