//! Make a table widget of control Port information that allows port
//! values to be edited
//use crate::lv2::ModHostController;
use crate::lv2::Port;
use ratatui::layout::Constraint;
use ratatui::style::palette::tailwind;
use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::style::Stylize;
use ratatui::text::Text;
use ratatui::widgets::Cell;
use ratatui::widgets::HighlightSpacing;
use ratatui::widgets::Row;
use ratatui::widgets::Table;

pub fn port_table<'a>(ports: &[Port]) -> Table<'a> {
    let normal_row_color: Color = tailwind::SLATE.c950;
    let alt_row_color: Color = tailwind::SLATE.c900;
    let row_fg: Color = tailwind::SLATE.c200;
    let buffer_bg: Color = tailwind::SLATE.c950;
    let selected_style_fg: Color = tailwind::BLUE.c400;
    let rows = ports.iter().enumerate().map(|(i, port)| {
        let color = match i % 2 {
            0 => normal_row_color,
            _ => alt_row_color,
        };
        let min: String;
        let max: String;
        let logarithmic: String;
        if let Some((n, _, x, l)) = port.get_min_def_max() {
            min = format!("{n:4}");
            max = format!("{x:4}");
            logarithmic = format!("{l}");
        } else {
            min = "".to_string();
            max = "".to_string();
            logarithmic = "".to_string();
        }

        let item = [port.name.clone(), min, port.value.clone(), max, logarithmic];
        item.into_iter()
            .map(|content| Cell::from(Text::from(format!("\n{content}\n"))))
            .collect::<Row>()
            .style(Style::new().fg(row_fg).bg(color))
            .height(2)
    });
    let (ln_name, _ln_symb) = ports.iter().fold((0, 0), |a, b| {
        (
            if a.0 < b.name.len() {
                b.name.len()
            } else {
                a.0
            },
            if a.1 < b.symbol.len() {
                b.symbol.len()
            } else {
                a.1
            },
        )
    });
    let ln_name = ln_name as u16;
    let _ln_symb = _ln_symb as u16;

    let selected_style = Style::default()
        .add_modifier(Modifier::REVERSED)
        .fg(selected_style_fg);

    Table::new(
        rows,
        [
            // + 1 is for padding.
            Constraint::Min(ln_name + 1),
            Constraint::Min(6),
            Constraint::Min(6),
            Constraint::Min(6),
            Constraint::Min(6),
        ],
    )
    .highlight_style(selected_style)
    .bg(buffer_bg)
    .highlight_spacing(HighlightSpacing::Always)
}
