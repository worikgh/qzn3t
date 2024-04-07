use crate::colours::ALT_ROW_COLOR;
use crate::colours::COMPLETED_TEXT_COLOR;
use crate::colours::NORMAL_ROW_COLOR;
use crate::colours::PENDING_TEXT_COLOR;
use crate::colours::SELECTED_TEXT_FG;
use crate::colours::STATIC_TEXT_FG;
/// The representation of a LV2 simulator for the purposes of
/// displaying it and keeping state about it
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::ListItem;

/// Whether the simuator is loaded into mod-host
#[derive(Copy, Debug, Clone, PartialEq, PartialOrd)]
pub enum Status {
   Loaded,
   Pending,
   Unloaded,
}

#[derive(Clone, Debug)]
pub struct Lv2Simulator {
   // Display name
   pub name: String,

   // Unique identifier
   pub url: String,

   // Status (un)loaded
   pub status: Status,

   /// The number assigned to this simulator for mod-host
   pub mh_id: usize,
}

impl Lv2Simulator {
   /// Make a list item for App::lv2_stateful_list
   pub fn to_stateful_list_item(&self, index: usize) -> ListItem {
      let bg_color = match index % 2 {
         0 => NORMAL_ROW_COLOR,
         _ => ALT_ROW_COLOR,
      };
      let line = match self.status {
         Status::Loaded => Line::styled(
            format!(" ☐ {:>3} {}", self.mh_id, self.name),
            SELECTED_TEXT_FG,
         ),
         Status::Unloaded => Line::styled(
            format!(" ✓ {:>3} {}", self.mh_id, self.name),
            (COMPLETED_TEXT_COLOR, bg_color),
         ),
         Status::Pending => Line::styled(
            format!(" {:>3} {} ", self.mh_id, self.name),
            (PENDING_TEXT_COLOR, bg_color),
         ),
      };
      ListItem::new(line).bg(bg_color)
   }

   /// Make a ListItem for App::lv2_loaded_list
   pub fn to_static_list_item(&self, index: usize) -> ListItem {
      let bg_color = match index % 2 {
         0 => NORMAL_ROW_COLOR,
         _ => ALT_ROW_COLOR,
      };
      let line = Line::styled(
         format!("{} effect_{} ", self.name, self.mh_id,),
         STATIC_TEXT_FG,
      );

      ListItem::new(line).bg(bg_color)
   }
}

/// Create a `Lv2Simulator` two Strings: name, url, and a `Status`
impl From<&(String, String, Status)> for Lv2Simulator {
   fn from((name, url, status): &(String, String, Status)) -> Self {
      Self {
         name: name.clone(),
         status: *status,
         url: url.clone(),
         mh_id: 0,
      }
   }
}
