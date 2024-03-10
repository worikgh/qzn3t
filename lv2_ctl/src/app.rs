//! # Run the user interface
//! Addapted from  Ratatui List example
//!
//! [Ratatui]: https://github.com/ratatui-org/ratatui
//! [examples]: https://github.com/ratatui-org/ratatui/blob/main/examples
//! [examples readme]: https://github.com/ratatui-org/ratatui/blob/main/examples/README.md

use crate::colours::NORMAL_ROW_COLOR;
use crate::colours::SELECTED_STYLE_FG;
use crate::colours::TEXT_COLOR;
use crate::colours::TODO_HEADER_BG;
use crate::lv2::{Lv2, ModHostController};
use crate::lv2_simulator::Lv2Simulator;
use crate::lv2_simulator::Status;
use crate::lv2_stateful_list::Lv2StatefulList;
use color_eyre::config::HookBuilder;
use crossterm::{
    event::{self, Event, KeyCode},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{prelude::*, widgets::*};
use std::thread;
use std::time::{Duration, Instant};
use std::{error::Error, io, io::stdout};

enum AppState {
    List,    // Listing all simulators
    Command, // Interacting with mod-host
}

/// This struct holds the current state of the app. 
pub struct App<'a> {
    mod_host_controller: &'a ModHostController,

    // Maintain the view for the first screen.  Which simulators are
    // "ticked"
    lv2_stateful_list: Lv2StatefulList,

    // Maintain the view for the second screen of ticked simulators
    lv2_loaded_list: Lv2StatefulList,

    app_state: AppState,
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
            eprintln!("{err}: Initialising error hooks");
        }

        // Vec<(name, url)>
        let types: Vec<(String, String)> = mod_host_controller
            .simulators
            .iter()
            .map(|s| (s.name.clone(), s.url.clone()))
            .collect();

        App {
            app_state: AppState::List,
            mod_host_controller,
            lv2_loaded_list: Lv2StatefulList::empty(),
            lv2_stateful_list: Lv2StatefulList::new(&types),
        }
    }

    /// Changes the status of the selected list item.  
    fn change_status(&mut self) {
        if let Some(i) = self.get_stateful_list_mut().state.selected() {
            self.get_stateful_list_mut().items[i].status =
                match self.get_stateful_list_mut().items[i].status {
                    Status::Unloaded => {
                        // Set status of Lv2 to Ticked.  Put the
                        // simulator into the list of ticked
                        // simulators
                        let lv2: Lv2Simulator = self.lv2_stateful_list.items[i].clone();
                        self.lv2_loaded_list.items.insert(
                            if i > self.lv2_loaded_list.items.len() {
                                self.lv2_loaded_list.items.len()
                            } else {
                                i
                            },
                            lv2,
                        );
                        Status::Loaded
                    }

                    // Set status of Lv2 to Unticked.  Remove from the
                    // list of ticked simulators
                    Status::Loaded => {
                        if let Some(i) = self
                            .lv2_loaded_list
                            .items
                            .iter()
                            .position(|x| x.url == self.lv2_stateful_list.items[i].url)
                        {
                            self.lv2_loaded_list.items.remove(i);
                        }
                        Status::Unloaded
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

    /// The main body of the App
    fn _run(&mut self, mut terminal: Terminal<impl Backend>) -> io::Result<()> {
        // init_error_hooks().expect("App::run error hooks");

        let target_fps = 60; // 400 is about the limit on Raspberry Pi 5
        let frame_time = Duration::from_secs(1) / target_fps as u32;
        loop {
            let start_time = Instant::now();

            self.draw(&mut terminal)?;
            if event::poll(Duration::from_secs(0)).expect("Polling for event") {
                match event::read() {
                    Ok(Event::Key(key)) => {
                        use KeyCode::*;
                        match key.code {
                            Char('q') | Esc => {
                                self.mod_host_controller
                                    .input_tx
                                    .send(b"quit\n".to_vec())
                                    .expect("Send quit to mod-host");

                                return Ok(());
                            }
                            Char('h') => (),
                            Left => self.get_stateful_list_mut().unselect(),
                            Down => {
                                self.get_stateful_list_mut().next();
                            }
                            Up => self.get_stateful_list_mut().previous(),
                            Right | Enter => self.change_status(),
                            Char('g') => self.go_top(),
                            Char('G') => self.go_bottom(),

                            // Function keys for setting modes
                            F(1) => self.app_state = AppState::List,
                            F(2) => self.app_state = AppState::Command,
                            _ => {}
                        }
                    }
                    Ok(Event::Resize(_, _)) => (),
                    Err(err) => panic!("{err}: Reading event"),
                    x => panic!("Error reading event: {x:?}"),
                };
            }
            let elapsed_time = Instant::now() - start_time;
            if elapsed_time < frame_time {
                thread::sleep(frame_time - elapsed_time);
            } else {
                eprintln!("Timing error: {elapsed_time:?}/{frame_time:?}");
            }
        }
    }

    fn draw(&mut self, terminal: &mut Terminal<impl Backend>) -> io::Result<()> {
        terminal.draw(|f| f.render_widget(self, f.size()))?;
        Ok(())
    }

    fn render_list(&mut self, area: Rect, buf: &mut Buffer) {
        // Create a space for header, todo list and the footer.
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
            Constraint::Length(3),
        ]);
        let [header_area, rest_area, footer_area] = vertical.areas(area);

        // Create two chunks with equal vertical screen space. One for the list and the other for
        // the info block.
        let vertical = Layout::vertical([Constraint::Percentage(25), Constraint::Percentage(75)]);
        let [upper_item_list_area, lower_item_list_area] = vertical.areas(rest_area);

        self.render_title(header_area, buf);
        self.render_lv2_list_selected(upper_item_list_area, buf);
        self.render_control_area(lower_item_list_area, buf);
        self.render_footer(footer_area, buf);
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
            .bg(TODO_HEADER_BG)
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
            .bg(TODO_HEADER_BG)
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
            .bg(TODO_HEADER_BG)
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

    fn render_control_area(&self, area: Rect, buf: &mut Buffer) {
        // We show the list item's info under the list in this paragraph
        let outer_info_block = Block::default()
            .borders(Borders::NONE)
            .fg(TEXT_COLOR)
            .bg(TODO_HEADER_BG)
            .title("LVC Control")
            .bold()
            .title_alignment(Alignment::Center);
        // let inner_info_block = Block::default()
        //     .borders(Borders::NONE)
        //     .bg(NORMAL_ROW_COLOR)
        //     .padding(Padding::horizontal(1));

        let outer_info_area = area;
        // let inner_info_area = outer_info_block.inner(outer_info_area);

        // We can render the header. Inner info will be rendered later
        outer_info_block.render(outer_info_area, buf);
    }

    fn render_footer(&self, area: Rect, buf: &mut Buffer) {
        match self.app_state {
            AppState::List => Paragraph::new(
                "Use ↓↑ to move, ← to unselect, → to change status, g/G to go top/bottom.\nAny other character to send instructions",
            ),
	    AppState::Command => Paragraph::new(
                "Use ↓↑ to move, ← to unselect, → to change status.\nAny other characters fol to send instructions <Enter> to send",
            )
        }
	.centered()
	    .render(area, buf)
	;
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
