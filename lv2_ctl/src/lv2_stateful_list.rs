use crate::lv2_simulator::Lv2Simulator;
/// Display LV2 simulators and select/deselect them
use ratatui::widgets::ListState;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Lv2StatefulList {
   // Ratatui object.  Size of List and selected item
   pub state: ListState,

   pub items: Vec<Lv2Simulator>,

   pub last_selected: Option<usize>, // The line that is selected
}
impl Lv2StatefulList {
   /// Get the selected Lv2Simulator
   pub fn _clone_selected(&self) -> Option<Lv2Simulator> {
      self.state.selected().map(|t| self.items[t].clone())
   }

   /// Update a port in the collection.  Return true if it existed,
   /// false never because it must exist
   #[allow(dead_code)]
   pub fn update_port(&mut self, url: &str, symbol: &str, value: &str) -> bool {
      match self.items.iter_mut().find(|i| i.url == url) {
         Some(i) => match i
            .control_ports
            .iter_mut()
            .find(|i| i.param_symbol == symbol)
         {
            Some(i) => {
               i.value = Some(value.to_string());
               true
            }
            None => panic!("No port {url}/{symbol}"),
         },
         None => panic!("No symulator {url}"),
      }
   }

   #[allow(dead_code)]
   pub fn get_selected_url(&self) -> Option<String> {
      self.state.selected().map(|t| self.items[t].url.clone())
   }

   #[allow(dead_code)]
   pub fn get_selected_mh_id(&self) -> Option<usize> {
      self.state.selected().map(|t| self.items[t].mh_id)
   }
   // pub fn get_selected_value(&self) -> Option<String> {
   //    self.state.selected().map(|t| {
   //       self.items[t]
   //          .value
   //          .as_ref()
   //          .unwrap_or(&"No Value".to_string())
   //          // .expect("Must be a value to get")
   //          .clone()
   //    })
   // }
   /// Create a Lv2Statefullist from a vector of name, url pairs.
   pub fn new(types: Vec<Lv2Simulator>) -> Lv2StatefulList {
      Lv2StatefulList {
         state: ListState::default(),
         last_selected: None,
         items: types,
         // .iter()
         // .enumerate()
         // .map(|t| Lv2Simulator {
         //    name: t.1 .0.clone(),
         //    status: Status::Unloaded,
         //    url: t.1 .1.clone(),
         //    mh_id: t.0, // This is used as mod-host to communicate with loaded simulator
         // })
         // .collect(),
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
