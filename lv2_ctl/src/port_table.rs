//! Make a table widget of control Port information

//use crate::lv2::ModHostController;
use crate::port::ControlPortProperties;
use crate::port::Port;
use crate::port::PortType;
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
   // Colours for the table
   let even_row_colour: Color = tailwind::SLATE.c950;
   let odd_row_colour: Color = tailwind::SLATE.c900;
   let row_fg: Color = tailwind::SLATE.c200;
   let buffer_bg: Color = tailwind::SLATE.c950;
   let selected_style_fg: Color = tailwind::BLUE.c400;

   let rows = ports.iter().enumerate().map(|(i, port)| {
      let colour = match i % 2 {
         0 => even_row_colour,
         _ => odd_row_colour,
      };

      // Set variables for the Port
      let (min, max, logarithmic) =
         if let Some(PortType::Control(control_port)) = port
            .types
            .iter()
            .find(|x| matches!(x, PortType::Control(_cp)))
         {
            match control_port {
               ControlPortProperties::Continuous(cp) => {
                  let min = format!("{:2}", cp.min);
                  let max = format!("{:2}", cp.max);
                  let log = format!("{}", cp.logarithmic);
                  (min, max, log)
               }
               ControlPortProperties::Scale(scale) => (
                  scale.labels_values[0].0.clone(),
                  scale
                     .labels_values
                     .last()
                     .expect("Expect some labels for port table")
                     .0
                     .clone(),
                  "false".to_string(),
               ),
            }
         } else {
            panic!("A port in port table that is not a Controlport")
         };

      // The row itself as a styled row
      let item = [
         port.name.clone(),
         min,
         "".to_string(), //port.value.clone().unwrap_or_else(|| "".to_string()),
         max,
         logarithmic,
      ];
      item
         .into_iter()
         .map(|content| Cell::from(Text::from(format!("\n{content}\n"))))
         .collect::<Row>()
         .style(Style::new().fg(row_fg).bg(colour))
         .height(2) // How does this react with the scroll bar?
   });

   // Find the longest name for assigning space in row
   let ln_name =
      ports
         .iter()
         .fold(0, |a, b| if a < b.name.len() { b.name.len() } else { a });
   let ln_name = ln_name as u16;

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
