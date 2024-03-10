use crate::colours::ALT_ROW_COLOR;
use crate::colours::COMPLETED_TEXT_COLOR;
use crate::colours::NORMAL_ROW_COLOR;
use crate::colours::SELECTED_TEXT_FG;
use crate::colours::STATIC_TEXT_FG;
/// The representation of a LV2 simulator for the purposes of
/// displaying it and keeping state about it
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::ListItem;

/// Whether the simuator is loaded into mod-host
#[derive(Copy, Clone, PartialEq, PartialOrd)]
pub enum Status {
    Loaded,
    Unloaded,
}

#[derive(Clone)]
pub struct Lv2Simulator {
    // Display name
    pub name: String,

    // Unique identifier
    pub url: String,

    // Status (un)loaded
    pub status: Status,
}

impl Lv2Simulator {
    /// Make a list item for App::lv2_stateful_list
    pub fn to_stateful_list_item(&self, index: usize) -> ListItem {
        let bg_color = match index % 2 {
            0 => NORMAL_ROW_COLOR,
            _ => ALT_ROW_COLOR,
        };
        let line = match self.status {
            Status::Loaded => Line::styled(format!(" ☐ {}", self.name), SELECTED_TEXT_FG),
            Status::Unloaded => Line::styled(
                format!(" ✓ {}", self.name),
                (COMPLETED_TEXT_COLOR, bg_color),
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
        let line = Line::styled(self.name.to_string(), STATIC_TEXT_FG);

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
        }
    }
}
