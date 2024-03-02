/// Run the user interface
use ratatui::widgets::ListItem;
use ratatui::widgets::HighlightSpacing;
use ratatui::widgets::StatefulWidget;
use ratatui::style::palette::tailwind;
use ratatui::layout::Constraint;
use ratatui::layout::Layout;
use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::widgets::ListDirection;
use ratatui::style::Style;
use ratatui::widgets::List;
use crate::tui;
use ratatui::buffer::Buffer;
use ratatui::layout::Alignment;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::symbols::border;
use ratatui::text::Line;
use ratatui::text::Text;
use ratatui::widgets::block::title::Position;
use ratatui::widgets::block::title::Title;
use ratatui::widgets::block::Block;
use crossterm::event::Event;
use ratatui::widgets::Borders;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;
use crossterm::event;
use crossterm::event::KeyEventKind;
use ratatui::Frame;
use ratatui::widgets::ListState;
use crossterm::event::KeyEvent;
use std::io;
use crossterm::event::KeyCode;
#[derive(Copy, Clone, Debug, Default)]
enum Status {
    #[default]
    Todo,
    Completed,
}

#[derive(Debug,)]
struct TodoItem<'a> {
    todo: &'a str,
    info: &'a str,
    status: Status,
}

impl TodoItem<'_> {
    fn to_list_item(&self, index: usize) -> ListItem {
        let bg_color = match index % 2 {
            0 => NORMAL_ROW_COLOR,
            _ => ALT_ROW_COLOR,
        };
        let line = match self.status {
            Status::Todo => Line::styled(format!(" ☐ {}", self.todo), TEXT_COLOR),
            Status::Completed => Line::styled(
                format!(" ✓ {}", self.todo),
                (COMPLETED_TEXT_COLOR, bg_color),
            ),
        };

        ListItem::new(line).bg(bg_color)
    }
}

#[derive(Debug, Default)]
struct StatefulList<'a> {
    state: ListState,
    items: Vec<TodoItem<'a>>,
    last_selected: Option<usize>,
}
#[derive(Debug, Default)]
pub struct App<'a> {
    items: StatefulList<'a>,
    counter: u8,
    exit: bool,
}

impl App<'_> {
    pub fn new() -> Self {
	Self {
	    
	    items:StatefulList::default(),
	    counter:0,
	    exit:false,
	}
    }
	    
	    
    /// runs the application's main loop until the user quits
    pub fn run(&mut self, terminal: &mut tui::Tui) -> io::Result<()> {
        while !self.exit {
            terminal.draw(|frame| self.render_frame(frame))?;
            self.handle_events()?;
        }
        Ok(())
    }

    fn render_frame(&self, frame: &mut Frame) {
	frame.render_widget(self, frame.size());
    }

    fn handle_events(&mut self) -> io::Result<()> {
        match event::read()? {
            // it's important to check that the event is a key press event as
            // crossterm also emits key release and repeat events on Windows.
            Event::Key(key_event) if key_event.kind == KeyEventKind::Press => {
                self.handle_key_event(key_event)
            }
            _ => {}
        };
        Ok(())        
    }
    fn handle_key_event(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Char('q') => self.exit(),
            KeyCode::Left => self.increment_counter(),
            KeyCode::Right => self.decrement_counter(),
            _ => {}
        }
    }
    fn exit(&mut self) {
        self.exit = true;
    }

    fn decrement_counter(&mut self) {
        self.counter += 1;
    }

    fn increment_counter(&mut self) {
        self.counter -= 1;
    }
}
const TODO_HEADER_BG: Color = tailwind::BLUE.c950;
const NORMAL_ROW_COLOR: Color = tailwind::SLATE.c950;
const ALT_ROW_COLOR: Color = tailwind::SLATE.c900;
const SELECTED_STYLE_FG: Color = tailwind::BLUE.c300;
const TEXT_COLOR: Color = tailwind::SLATE.c200;
const COMPLETED_TEXT_COLOR: Color = tailwind::GREEN.c500;
impl App<'_> {
    fn render_title(&self, area: Rect, buf: &mut Buffer) {
        Paragraph::new("Qzn3T LV2 Control")
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

    fn render_info(&self, _area: Rect, _buf: &mut Buffer) {
	todo!()
    }

    fn render_footer(&self, _area: Rect, _buf: &mut Buffer) {
	todo!()
    }
}
    
impl Widget for &App<'_> {
    fn render(mut self, area: Rect, buf: &mut Buffer) {
        // Create a space for header, list and the footer.
        let vertical = Layout::vertical([
            Constraint::Length(2),
            Constraint::Min(0),
            Constraint::Length(2),
        ]);
	let [header_area, rest_area, footer_area] = vertical.areas(area);
        // Create two chunks with equal vertical screen space. One for the list and the other for
        // the info block.
        let vertical = Layout::vertical([Constraint::Percentage(50), Constraint::Percentage(50)]);
        let [_upper_item_list_area, lower_item_list_area] = vertical.areas(rest_area);
        self.render_title(header_area, buf);
        // self.render_todo(upper_item_list_area, buf);
        self.render_info(lower_item_list_area, buf);
        self.render_footer(footer_area, buf);
	
        let title = Title::from(" Qzn3T LV2 Control ".bold());
        let instructions = Title::from(Line::from(vec![
            " One ".red().into(),
            " Two".blue().bold().bold(),
            " Three".green().italic().into(),
            " Four".blue().bold(),
            " Five".slow_blink().magenta().into(),
            "<Q> ".blue().bold(),
        ]));
        let _block = Block::default()
            .title(title.alignment(Alignment::Center))
            .title(
                instructions
                    .alignment(Alignment::Right)
                    .position(Position::Bottom),
            )
            .borders(Borders::ALL)
            .border_set(border::PLAIN);

        let _counter_text = Text::from(vec![Line::from(vec![
            "Value: ".into(),
            self.counter.to_string().yellow(),
        ])]);
	let items = ["Item 1", "Item 2", "Item 3"];
	 List::new(items)
	    .block(Block::default().title("List").borders(Borders::ALL))
	    .style(Style::default().fg(Color::White))
	    .highlight_style(Style::default().add_modifier(Modifier::ITALIC))
	    .highlight_symbol(">>")
	    .repeat_highlight_symbol(true)
	    .direction(ListDirection::BottomToTop);

	//     Paragraph::new(counter_text)
	//         .centered()
	//         .block(block)
	//         .render(area, buf);
	// }
	
    }
}
