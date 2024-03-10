use crate::lv2_simulator::Lv2Simulator;
use crate::lv2_simulator::Status;
/// Display LV2 simulators and select/deselect them
use ratatui::widgets::ListState;

pub struct Lv2StatefulList {
    // Ratatui object.  Size of List and selected item
    pub state: ListState,

    pub items: Vec<Lv2Simulator>,

    pub last_selected: Option<usize>, // The line that is selected
}

impl Lv2StatefulList {
    /// Create a Lv2Statefullist from a vector of name, url pairs.
    pub fn new(types: &[(String, String)]) -> Lv2StatefulList {
        Lv2StatefulList {
            state: ListState::default(),
            last_selected: None,
            items: types
                .iter()
                .map(|t| Lv2Simulator {
                    name: t.0.clone(),
                    status: Status::Unloaded,
                    url: t.1.clone(),
                })
                .collect(),
        }
    }
    /// An empty list
    pub fn empty() -> Lv2StatefulList {
        Lv2StatefulList {
            state: ListState::default(),
            items: vec![],
            last_selected: None,
        }
    }
}
