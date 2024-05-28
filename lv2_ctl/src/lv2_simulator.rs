//! The information needed to display a loaded LV2 simulator in a list.

/// Whether the simuator is loaded into mod-host
#[derive(Copy, Debug, Clone, PartialEq, PartialOrd)]
pub enum Status {
   Loaded,
   Pending,
   Unloaded,
}

/// The control ports need to be displayed.  They need a value a value
/// to display and adjust
#[derive(Clone, Debug)]
pub struct ControlPort {
   /// The name that describes this port to mod-host
   pub param_symbol: String,

   // The values that can be sent to the backend.  Port values can
   // be integer or decimal, but they are allways sent as strings.
   pub value: Option<String>,

   /// If it is a ScalePort it has a label as well as a value
   pub label: Option<Vec<String>>,
}

/// All the information the front end needs to control a LV2
/// simulator.  Most of the data is stored as String, that is how it
/// is sent to and received from mod-host
#[derive(Clone, Debug)]
pub struct Lv2Simulator {
   /// Display name
   pub name: String,

   /// Unique identifier.  There can be more than one Lv2Simulator in
   /// a list, each with a different `mh_id`.
   pub url: String,

   /// Status loaded, unloaded, or pending
   pub status: Status,

   /// The number assigned to this simulator for mod-host
   pub mh_id: usize,

   /// The control ports that are displayed for the user to interact with
   pub control_ports: Vec<ControlPort>,

   /// The input ports.
   pub input_ports: Vec<String>,

   /// The output ports.
   pub output_ports: Vec<String>,
}

impl Lv2Simulator {}

/// The values the port can take.
/// The maximum is 128 discreet values for MIDI control
// fn make_values(ctrl_prop: ControlPortProperties) -> Vec<String> {
//    match ctrl_prop.scale {
//       Some(sd) => {
//          // Discreet values
//          sd.values.iter().map(|v| format!("{v}")).collect()
//       }
//       None => {
//          // 128 values

//          vec![]
//       }
//    }
// }

// Implement the `From<&Port>` trait for `ControlPort` for Ports of type `Control`
// impl From<&Port> for ControlPort {
//    fn from(port: &Port) -> Self {
//       let p = port
//          .types
//          .iter()
//          .find(|p| match p {
//             PortType::Control(_) => true,
//             _ => false,
//          })
//          .expect("A control port");
//       if let PortType::Control(control_properties) = &port.types[0] {
//          ControlPort {
//             display_name: port.name.clone(),
//             param_symbol: port.symbol.clone(),
//             min: format!("{}", control_properties.min),
//             max: format!("{}", control_properties.max),
//             values: vec![],
//             labels: None,
//          }
//       } else {
//          panic!("Attempting to convert a non-Control type Port to ControlPort");
//       }
//    }
// }
/// Create a `Lv2Simulator` two Strings: name, url, and a `Status`
impl From<&(String, String, Status)> for Lv2Simulator {
   fn from((name, url, status): &(String, String, Status)) -> Self {
      Self {
         name: name.clone(),
         status: *status,
         url: url.clone(),
         mh_id: 0,
         control_ports: vec![],
         input_ports: vec![],
         output_ports: vec![],
         // value: None,
      }
   }
}
