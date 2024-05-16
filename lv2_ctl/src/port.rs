/// A Port can be:
/// * MIDI in/out.  Unimplemented as yet, here
/// * Audio in/out.
/// * Control
/// * Control can be:
///   * discrete, with a list of values and labels
///   * Continuous.  A continuous port can be:
///     * Integer
///     * Decimal
///     * Float
use crate::lv2::ScaleDescription;

#[derive(Clone, PartialEq, Debug, PartialOrd)]
pub enum ContinuousType {
   Integer,
   Decimal,
   Float,
}

#[derive(Clone, PartialEq, Debug, PartialOrd)]
pub struct ContinuousControlPort {
   pub kind: ContinuousType,
   pub max: f64,
   pub min: f64,
   pub default: f64,
   pub logarithmic: bool,
}

impl ContinuousControlPort {
   fn _values(&self) -> Vec<String> {
      vec![]
   }
}

#[derive(Clone, PartialEq, Debug, PartialOrd)]
pub struct ScaleControlPort {
   labels_values: Vec<(String, String)>,
}

impl ScaleControlPort {
   fn _values(&self) -> Vec<&String> {
      self
         .labels_values
         .iter()
         .map(|(_, a)| a)
         .collect::<Vec<&String>>()
   }

   #[allow(dead_code)]
   fn labels(&self) -> Vec<&String> {
      self
         .labels_values
         .iter()
         .map(|(a, _a)| a)
         .collect::<Vec<&String>>()
   }
}

#[derive(Clone, PartialEq, Debug, PartialOrd)]
pub enum ControlPortProperties {
   Continuous(ContinuousControlPort),
   Scale(ScaleControlPort),
}

impl ControlPortProperties {
   pub fn new(
      min: f64,
      max: f64,
      default: f64,
      logarithmic: bool,
      _scale: Option<ScaleDescription>,
      kind: ContinuousType,
   ) -> Self {
      ControlPortProperties::Continuous(ContinuousControlPort {
         kind,
         min,
         max,
         default,
         logarithmic,
      })
   }
}

//#[derive(, Eq, Hash, Ord,)]
#[derive(Clone, PartialEq, Debug, PartialOrd)]
pub enum PortType {
   Input,
   Output,
   Control(ControlPortProperties),
   Audio,
   AtomPort,
   Other(String),
}
impl PortType {
   // Return the strings  to asign to the control values.  These are sent to mod-host
   fn _values(pt: PortType) -> Vec<String> {
      match pt {
         PortType::Control(properties) => {
            match properties {
               ControlPortProperties::Continuous(_cp) => {
                  // struct ContinuousControlPort {
                  // 	 kind: ContinuousType,
                  // 	 max: f64,
                  // 	 min: f64,
                  // 	 default: f64,
                  // 	 logarithmic: bool,
                  // }
               }
               ControlPortProperties::Scale(_sp) => (),
            }
         }
         _ => panic!("Only implemented for Control ports"),
      };
      vec![]
   }
}
#[derive(Debug, Clone)]
pub struct Port {
   pub name: String,   // For display
   pub symbol: String, // For sending to mod-host
   pub types: Vec<PortType>,
   pub index: usize, // index from LV2 description
}

impl Port {
   pub fn new() -> Self {
      Self {
         name: "".to_string(),
         symbol: "".to_string(),
         types: vec![],
         index: 0,
      }
   }
}
impl Default for Port {
   fn default() -> Self {
      Self::new()
   }
}
