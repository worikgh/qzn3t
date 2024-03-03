//! # Run the user interface
//! Addapted from  Ratatui List example
//!
//! [Ratatui]: https://github.com/ratatui-org/ratatui
//! [examples]: https://github.com/ratatui-org/ratatui/blob/main/examples
//! [examples readme]: https://github.com/ratatui-org/ratatui/blob/main/examples/README.md

use crate::lv2::{Lv2Type, ModHostController};
use std::{error::Error, io, io::stdout};

use color_eyre::config::HookBuilder;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{prelude::*, style::palette::tailwind, widgets::*};

const TODO_HEADER_BG: Color = tailwind::BLUE.c950;
const NORMAL_ROW_COLOR: Color = tailwind::SLATE.c950;
const ALT_ROW_COLOR: Color = tailwind::SLATE.c900;
const SELECTED_STYLE_FG: Color = tailwind::BLUE.c300;
const TEXT_COLOR: Color = tailwind::SLATE.c200;
const COMPLETED_TEXT_COLOR: Color = tailwind::GREEN.c500;

#[derive(Copy, Clone)]
enum Status {
    Active,
    Ready,
}

struct Lv2Simulator {
    name: String, // Cannot be a reference because it is bvuild from an enum
    status: Status,
}

struct StatefulList {
    state: ListState,
    items: Vec<Lv2Simulator>,
    last_selected: Option<usize>,
}

/// This struct holds the current state of the app. In particular, it has the `items` field which is
/// a wrapper around `ListState`. Keeping track of the items state let us render the associated
/// widget with its state and have access to features such as natural scrolling.
///
/// Check the event handling at the bottom to see how to change the state on incoming events.
/// Check the drawing logic for items on how to specify the highlighting style for selected items.
pub struct App<'a> {
    mod_host_controller: &'a ModHostController,
    items: StatefulList,
}

impl Drop for App<'_> {
    fn drop(&mut self) {
        restore_terminal().expect("Restore terminal");
    }
}
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
    pub fn new(mod_host_controller: &ModHostController) -> App {
        if let Err(err) = init_error_hooks() {
            eprintln!("{err}: Initialising error hooks");
        }
        let types: Vec<String> = mod_host_controller
            .simulators
            .iter()
            .map(|s| {
                s.types
                    .iter()
                    .filter(|&t| *t != Lv2Type::Plugin) // They are all 'Plugin"
                    .fold("".to_string(), |a, b| {
                        format!(
                            "{a}{}{:?}",
                            if a.as_str() == "" {
                                // Beginning of string
                                ""
                            } else {
                                "/"
                            },
                            b
                        )
                    })
            })
            .collect();

        App {
            mod_host_controller,
            items: StatefulList {
                state: ListState::default(),
                last_selected: None,
                items: types
                    .iter()
                    .map(|t| Lv2Simulator {
                        name: t.clone(),
                        status: Status::Ready,
                    })
                    .collect(),
            },
        }
    }

    /// Changes the status of the selected list item
    fn change_status(&mut self) {
        if let Some(i) = self.items.state.selected() {
            self.items.items[i].status = match self.items.items[i].status {
                Status::Ready => Status::Active,
                Status::Active => Status::Ready,
            }
        }
    }

    fn go_top(&mut self) {
        self.items.state.select(Some(0))
    }

    fn go_bottom(&mut self) {
        self.items.state.select(Some(self.items.items.len() - 1))
    }
}

impl App<'_> {
    pub fn run(mod_host_controller: &ModHostController) -> Result<(), Box<dyn Error>> {
        // init_error_hooks()?;
        let terminal = init_terminal()?;
        let mut app = App::new(mod_host_controller);
        app._run(terminal).expect("Calling _run");

        restore_terminal()?;

        Ok(())
    }

    pub fn _run(&mut self, mut terminal: Terminal<impl Backend>) -> io::Result<()> {
        // init_error_hooks().expect("App::run error hooks");

        loop {
            self.draw(&mut terminal)?;

            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    use KeyCode::*;
                    match key.code {
                        Char('q') | Esc => {
                            self.mod_host_controller
                                .input_tx
                                .send(b"quit\n".to_vec())
                                .expect("Send quit to mod-host");

                            return Ok(());
                        }
                        Char('h') | Left => self.items.unselect(),
                        Char('j') | Down => self.items.next(),
                        Char('k') | Up => self.items.previous(),
                        Char('l') | Right | Enter => self.change_status(),
                        Char('g') => self.go_top(),
                        Char('G') => self.go_bottom(),
                        _ => {}
                    }
                }
            }
        }
    }

    fn draw(&mut self, terminal: &mut Terminal<impl Backend>) -> io::Result<()> {
        terminal.draw(|f| f.render_widget(self, f.size()))?;
        Ok(())
    }
}

impl Widget for &mut App<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
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
        self.render_todo(upper_item_list_area, buf);
        self.render_info(lower_item_list_area, buf);
        self.render_footer(footer_area, buf);
    }
}

impl App<'_> {
    fn render_title(&self, area: Rect, buf: &mut Buffer) {
        Paragraph::new("Ratatui List Example")
            .bold()
            .centered()
            .render(area, buf);
    }

    fn render_todo(&mut self, area: Rect, buf: &mut Buffer) {
        // We create two blocks, one is for the header (outer) and the other is for list (inner).
        let outer_block = Block::default()
            .borders(Borders::NONE)
            .fg(TEXT_COLOR)
            .bg(TODO_HEADER_BG)
            .title("TODO List")
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
            .items
            .items
            .iter()
            .enumerate()
            .map(|(i, todo_item)| todo_item.to_list_item(i))
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
        StatefulWidget::render(items, inner_area, buf, &mut self.items.state);
    }

    fn render_info(&self, area: Rect, buf: &mut Buffer) {
        // We get the info depending on the item's state.
        let info = if let Some(i) = self.items.state.selected() {
            match self.items.items[i].status {
                Status::Ready => "✓ DONE: ".to_string(),
                Status::Active => "TODO: ".to_string(),
            }
        } else {
            "Nothing to see here...".to_string()
        };

        // We show the list item's info under the list in this paragraph
        let outer_info_block = Block::default()
            .borders(Borders::NONE)
            .fg(TEXT_COLOR)
            .bg(TODO_HEADER_BG)
            .title("TODO Info")
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

    fn render_footer(&self, area: Rect, buf: &mut Buffer) {
        Paragraph::new(
            "\nUse ↓↑ to move, ← to unselect, → to change status, g/G to go top/bottom.",
        )
        .centered()
        .render(area, buf);
    }
}

impl StatefulList {

    fn next(&mut self) {
        let i = match self.state.selected() {
            Some(i) => {
                if i >= self.items.len() - 1 {
                    0
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
                    self.items.len() - 1
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

impl Lv2Simulator {
    fn to_list_item(&self, index: usize) -> ListItem {
        let bg_color = match index % 2 {
            0 => NORMAL_ROW_COLOR,
            _ => ALT_ROW_COLOR,
        };
        let line = match self.status {
            Status::Active => Line::styled(format!(" ☐ {}", self.name), TEXT_COLOR),
            Status::Ready => Line::styled(
                format!(" ✓ {}", self.name),
                (COMPLETED_TEXT_COLOR, bg_color),
            ),
        };

        ListItem::new(line).bg(bg_color)
    }
}

impl From<&(String, Status)> for Lv2Simulator {
    fn from((name, status): &(String, Status)) -> Self {
        Self {
            name: name.clone(),
            status: *status,
        }
    }
}
