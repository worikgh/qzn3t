//! Definition of an LV2 simulator as defined in the Turtle files
use crate::mod_host_controller::ModHostController;
use crate::port::ContinuousType;
use crate::port::ControlPortProperties;
use crate::port::Port;
use crate::port::PortType;
use crate::run_executable::run_executable;
use core::fmt;
use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::io::Result;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;

/// The assembled simulator with all the data necessary to load it
/// into a host
#[derive(Debug)]
pub struct Lv2 {
   pub types: HashSet<Lv2Type>,
   pub ports: Vec<Port>,
   pub name: String,
   pub url: String,
}

#[derive(PartialEq, Eq, Hash, Debug, Ord, PartialOrd, Clone)]
pub enum Lv2Type {
   Plugin,
   ReverbPlugin,
   ChorusPlugin,
   FlangerPlugin,
   PhaserPlugin,
   WaveshaperPlugin,
   FunctionalProperty,
   SpectralPlugin,
   LimiterPlugin,
   AnalyserPlugnin,
   PitchPlugin,
   ObjectProperty,
   Property,
   SpatialPlugin,
   UtilityPlugin,
   AmplifierPlugin,
   ExpanderPlugin,
   CompressorPlugin,
   EQPlugin,
   ModulatorPlugin,
   InstrumentPlugin,
   SimulatorPlugin,
   FilterPlugin,
   DelayPlugin,
   DistortionPlugin,
   Class,
   Project,
   EnvelopePlugin,
   Other(String),
}

/// Stores all the data required to run LV2 simulators
#[derive(Debug, PartialEq, PartialOrd)]
struct Lv2Datum {
   subject: String,
   predicate: String,
   object: String,
}

// Filter Lv2Datum by predicate
fn predicate_filter<'a, T: Iterator<Item = &'a &'a Lv2Datum>>(
   i: T,
   filter: &'a str,
) -> Vec<&'a &'a Lv2Datum> {
   //Vec<&'a &'a Lv2Datum> {
   i.filter(|l| l.predicate == filter)
      .collect::<Vec<&'a &'a Lv2Datum>>()
}

/// For strings starting like `"1.0"^^<htt...` And the first quoted part
/// (1.0) is wanted.  Panic if invalid string passed
fn remove_quotes(inp: &str) -> &str {
   // The first character is a quote.  Thence the string intil the
   // next quote
   let i = inp[1..] // Exclude the first quote
	.find('"') // Find the next
	.expect("Removing quotes from {inp}.  i:{i}") // Unwrap it
	+ 1; // Why this?
   &inp[1..i]
}

/// Numbers for control ports are in the data often without a decimal
/// point.  This takes a LV2 object string and extracts the number.
/// Or panics.  It also returns the LV2 type
/// "0.5"^^<http://www.w3.org/2001/XMLSchema#decimal>
fn control_number(object: &str) -> (f64, String) {
   let b = &object[1..];
   let c = b
      .chars()
      .position(|c| c == '"')
      .expect("Fnding quote in object");
   let n = &b[..c];
   let n = n.parse::<f64>().expect("control number parse");

   let b = &b[c + 1..];
   let c = b
      .chars()
      .position(|c| c == '#')
      .expect("Fnding quote in object");
   let schema = b[c + 1..b.len() - 1].to_string();
   (n, schema)
}

#[allow(dead_code)]
fn make_control_port_values(
   min: f64,
   max: f64,
   logarithmic: bool,
) -> [f64; 128] {
   let mut result = [0_f64; 128];
   if logarithmic {
      let log_min = min.ln();
      let log_max = max.ln();

      for (i, item) in result.iter_mut().enumerate() {
         //      for i in 0..128 {
         let t = i as f64 / 127.0;
         *item = (log_min + (log_max - log_min) * t.exp()).exp();
      }
   } else {
      let increment = (max - min) / 127.0;

      for (i, item) in result.iter_mut().enumerate() {
         *item = min + increment * i as f64;
      }
   }
   // eprintln!("INFO: control_port_values: {result:?}");
   result
}

fn get_number_from_object(object: &str) -> &str {
   // "29"^^<http://www.w3.org/2001/XMLSchema#integer> not usize
   let c = object[1..]
      .chars()
      .position(|c| c == '^')
      .expect("Two quotes in number string");
   &object[1..c]
}

fn _port_name(data: &[&Lv2Datum]) -> String {
   data
      .iter()
      .filter(|&l| l.predicate == "<http://lv2plug.in/ns/lv2core#name>")
      .collect::<Vec<&&Lv2Datum>>()
      .iter()
      .fold(String::new(), |a, b| {
         // println!("Name: '{a}' + '{}'", b.object);
         a + remove_quotes(b.object.as_str())
      })
}

#[derive(Clone)]
pub struct ScaleDescription {
   pub labels: Vec<String>,
   pub values: Vec<String>,
}

/// For control ports get the important data
fn get_mmdls(
   l: &[&Lv2Datum],
   lv2_data: &[Lv2Datum],
) -> (
   f64,
   f64,
   f64,
   bool,
   Option<ScaleDescription>,
   ContinuousType,
) {
   let min_set: Vec<&Lv2Datum> =
      predicate_filter(l.iter(), "<http://lv2plug.in/ns/lv2core#minimum>")
         .into_iter()
         .copied()
         .collect();

   if min_set.is_empty() {
      eprintln!("Oh dear");
      assert!(min_set.len() == 1);
   }
   let min_set = control_number(&min_set[0].object);
   let min_type = min_set.1;
   let min = min_set.0;
   let max_set: Vec<&Lv2Datum> =
      predicate_filter(l.iter(), "<http://lv2plug.in/ns/lv2core#maximum>")
         .into_iter()
         .copied()
         .collect();
   assert!(max_set.len() == 1);
   let max_set = control_number(&max_set[0].object);
   let max_type = max_set.1;
   let max = max_set.0;
   // let max: f64 = l
   // 	 .iter()
   //   .filter(|&l| l.predicate == "<http://lv2plug.in/ns/lv2core#maximum>")
   // .collect::<Vec<&&Lv2Datum>>()
   // .iter()
   //   .fold(0.0, |a, b| a
   // 		  + control_number(b.object.as_str()));

   let default_set: Vec<&Lv2Datum> =
      predicate_filter(l.iter(), "<http://lv2plug.in/ns/lv2core#default>")
         .into_iter()
         .copied()
         .collect();
   let default_set = if default_set.len() == 1 {
      control_number(&default_set[0].object)
   } else if default_set.is_empty() {
      // What to do if not default?
      (min, min_type.clone())
   } else {
      panic!("Too many defaults");
   };
   let def_type = default_set.1;
   let default = default_set.0;
   // let default: f64 =
   //      predicate_filter(l.iter(),
   // 							 "<http://lv2plug.in/ns/lv2core#default>")
   //       .iter()
   //       .fold(0.0, |a, b| a + control_number(b.object.as_str()));

   let logarithmic: bool = !predicate_filter(
      l.iter(),
      "<http://lv2plug.in/ns/lv2core#portProperty>",
   )
   .iter()
   .filter(|lv| {
      lv.object == "<http://lv2plug.in/ns/ext/port-props#logarithmic>"
   })
   .collect::<Vec<&&&Lv2Datum>>()
   .is_empty();

   // Get `scalePoints' to construct a `scale` value
   // for the Port, if it is of that type .  It will
   // have a scale if it is a Control Port with
   // discrete values.  E.g. "on"/"off", or
   // "sin"/"triangle"/"square"
   let scale_points = l
      .iter()
      .filter(|l| l.predicate == "<http://lv2plug.in/ns/lv2core#scalePoint>")
      .collect::<Vec<&&Lv2Datum>>()
      .iter()
      .map(|&l| l.object.as_str())
      .collect::<Vec<&str>>();
   let scale: Option<ScaleDescription> = if scale_points.is_empty() {
      None
   } else {
      let mut scale_description = ScaleDescription {
         labels: vec![],
         values: vec![],
      };
      for sp in scale_points.iter() {
         // For each scale point need to rescan all the
         // tripples.  Frustratingly inefficient
         let both_points: Vec<&Lv2Datum> =
            lv2_data.iter().filter(|&x| &x.subject == sp).collect();
         if both_points.len() != 2 {
            eprintln!("scale is wrong.   {both_points:?}");
            continue;
         }
         for sp in both_points {
            if sp.predicate.as_str()
               == "<http://www.w3.org/2000/01/rdf-schema#label>"
            {
               let label = remove_quotes(sp.object.as_str());

               scale_description.labels.push(label.to_string());
            }
            if sp.predicate.as_str()
               == "<http://www.w3.org/1999/02/22-rdf-syntax-ns#value>"
            {
               scale_description
                  .values
                  .push(get_number_from_object(sp.object.as_str()).to_string());
            }
         }
      }
      Some(scale_description)
   };

   // if max_type != min_type || max_type != def_type {
   //     eprint!("Control port types differ:  {l:?} {default} - {max}");
   // }
   // For some reason it is sometimes true that not all the minimum,
   // maximum, and default values are the same type.  Use a "majority
   // rules" algorithm.
   let con_type = if max_type == min_type || max_type == def_type {
      max_type
   } else if min_type == def_type {
      min_type
   } else {
      eprintln!("All three types are diferent");
      "double".to_string()
   };

   (
      min,
      max,
      default,
      logarithmic,
      scale,
      match con_type.as_str() {
         "integer" => ContinuousType::Integer,
         "decimal" => ContinuousType::Decimal,
         "double" => ContinuousType::Float,
         _ => panic!("{con_type}: Unknown conntrol port type"),
      },
   )
}

/// `lines` defines the LV2 simulators on the host computer.  Uses
/// the output of [serd](https://gitlab.com/drobilla/serd)
pub fn get_lv2_controller(
   lines: impl Iterator<Item = Result<String>>,
   // lines: Lines<StdinLock>,
) -> Result<ModHostController> {
   let mut lv2_data: Vec<Lv2Datum> = vec![];
   let mut subject_store: HashMap<String, usize> = HashMap::new();
   let mut predicate_store: HashMap<String, usize> = HashMap::new();
   let mut object_store: HashMap<String, usize> = HashMap::new();

   let mut index_sbj = 0;
   for line in lines {
      let line = line?;
      //.map(|x| x.unwrap()) {
      // `line` is n three useful parts: Subject, Predicate, and Object
      // 1. The line upto the first space ' ' is the Subject
      // 2. The remainder of the line upto the next space is the Predicate
      // 3. The remainder of the line, except for the last two characters, is the Object
      // The Object can contain spaces.
      // The last two characters are " ."

      let mut split: Vec<&str> = line.as_str().split(' ').collect();
      let subject = split.remove(0).to_string();
      let predicate = split.remove(0).to_string();

      let _object = split.join(" ");
      if &_object[_object.len() - 2..] != " ." {
         panic!("Bad line: {line}");
      }
      let object = _object.as_str()[..(_object.len() - 2)].to_string();

      if subject_store.get(&subject).is_none() {
         // First time a subject is seen.

         subject_store.insert(subject.clone(), index_sbj);
         index_sbj += 1;
      }
      if predicate_store.get(&predicate).is_none() {
         predicate_store.insert(predicate.clone(), index_sbj);
         index_sbj += 1;
      }
      if object_store.get(&object).is_none() {
         object_store.insert(object.clone(), index_sbj);
         index_sbj += 1;
      }
      // eprintln!("SPO {subject} {predicate} {object}");
      lv2_data.push(Lv2Datum {
         subject,
         predicate,
         object,
      });
   }

   // Keep track of which subjects have been processed
   let mut processed: HashSet<&String> = HashSet::new();

   // Keep track of the simulators to put into the result
   let mut simulators: Vec<Lv2> = vec![];

   for l in lv2_data.iter() {
      if &l.object == "<http://lv2plug.in/ns/lv2core#Plugin>" {
         // Examine this because it is a plugin.

         if !processed.insert(&l.subject) {
            // This subject has been processed
            continue;
         }
         // eprintln!("TRIPPLE: {} {} {}", l.subject, l.predicate, l.object);
         // It is a plugin that has not been processed yet

         // Collect all data for this plugin identified by `subject`
         let plugin_data: Vec<&Lv2Datum> = lv2_data
            .iter()
            .filter(|lv| lv.subject == l.subject)
            .collect();

         // Get name
         let plugin_name = plugin_data
            .iter()
            .filter(|lv| lv.predicate == "<http://usefulinc.com/ns/doap#name>")
            .collect::<Vec<&&Lv2Datum>>()
            .iter()
            .fold("".to_string(), |a, &b| {
               a + &b.object.as_str()[1..(b.object.len() - 1)]
            });

         // Collect all types for this plugin.  Will probably be two or three
         let plugin_type: HashSet<Lv2Type> = plugin_data
            .iter()
            .filter(|lv| {
               lv.predicate
                  == "<http://www.w3.org/1999/02/22-rdf-syntax-ns#type>"
            })
            .collect::<Vec<&&Lv2Datum>>()
            .iter()
            .map(|lv| {
               let i = lv.object.find('#').unwrap();
               let j = lv.object.rfind('>').unwrap();
               match &lv.object.as_str()[(i + 1)..j] {
                  "Plugin" => Lv2Type::Plugin,
                  "ReverbPlugin" => Lv2Type::ReverbPlugin,
                  "ChorusPlugin" => Lv2Type::ChorusPlugin,
                  "FlangerPlugin" => Lv2Type::FlangerPlugin,
                  "PhaserPlugin" => Lv2Type::PhaserPlugin,
                  "WaveshaperPlugin" => Lv2Type::WaveshaperPlugin,
                  "FunctionalProperty" => Lv2Type::FunctionalProperty,
                  "SpectralPlugin" => Lv2Type::SpectralPlugin,
                  "LimiterPlugin" => Lv2Type::LimiterPlugin,
                  "AnalyserPlugin" => Lv2Type::AnalyserPlugnin,
                  "PitchPlugin" => Lv2Type::PitchPlugin,
                  "ObjectProperty" => Lv2Type::ObjectProperty,
                  "Property" => Lv2Type::Property,
                  "SpatialPlugin" => Lv2Type::SpatialPlugin,
                  "UtilityPlugin" => Lv2Type::UtilityPlugin,
                  "AmplifierPlugin" => Lv2Type::AmplifierPlugin,
                  "ExpanderPlugin" => Lv2Type::ExpanderPlugin,
                  "CompressorPlugin" => Lv2Type::CompressorPlugin,
                  "EQPlugin" => Lv2Type::EQPlugin,
                  "ModulatorPlugin" => Lv2Type::ModulatorPlugin,
                  "InstrumentPlugin" => Lv2Type::InstrumentPlugin,
                  "SimulatorPlugin" => Lv2Type::SimulatorPlugin,
                  "FilterPlugin" => Lv2Type::FilterPlugin,
                  "DelayPlugin" => Lv2Type::DelayPlugin,
                  "DistortionPlugin" => Lv2Type::DistortionPlugin,
                  "Class" => Lv2Type::Class,
                  "EnvelopePlugin" => Lv2Type::EnvelopePlugin,
                  "Project" => Lv2Type::Project,
                  x => Lv2Type::Other(x.to_string()),
               }
            })
            .collect();

         // The ports.  These will be control ports and audio I/O ports
         let plugin_ports: Vec<Port>;
         {
            // Collect all the subject names of ports for this simulator
            let port_names: Vec<String> = plugin_data
               .iter()
               .filter(|lv| {
                  &lv.predicate == "<http://lv2plug.in/ns/lv2core#port>"
                     && lv.subject == l.subject
               })
               .collect::<Vec<&&Lv2Datum>>()
               .iter()
               .map(|&&lv| lv)
               .collect::<Vec<&Lv2Datum>>()
               .iter()
               .map(|p| p.object.clone())
               .collect::<Vec<String>>();

            // Process the ports Each port has a subject like
            // `_:gx_zita_rev1b9`.  Usually about two dozen lines
            // that describe a port

            plugin_ports = port_names
               .iter()
                 .map(|p| {
                  // :Vec<Vec<Port>> `p`

                  // `l` is the set of tripples that define this port
                   let plugin_data: Vec<&Lv2Datum> = lv2_data.iter().
							  filter(|&x| &x.subject == p).collect();

                  let name: String = plugin_data
                     .iter()
                     .filter(|&l| l.predicate == "<http://lv2plug.in/ns/lv2core#name>")
                     .collect::<Vec<&&Lv2Datum>>()
                     .iter()
                     .fold(String::new(), |a, b| {
                        // println!("Name: '{a}' + '{}'", b.object);
                        a + remove_quotes(b.object.as_str())
                     });

                  let symbol: String = plugin_data
                     .iter()
                     .filter(|&l| l.predicate == "<http://lv2plug.in/ns/lv2core#symbol>")
                     .collect::<Vec<&&Lv2Datum>>()
                     .iter()
                     .fold(String::new(), |a, b| {
                        // println!("Symbol: '{a}' + '{}'", b.object);
                        a + remove_quotes(b.object.as_str())
                     });

                  let index: usize = plugin_data
                     .iter()
                     .filter(|&l| l.predicate == "<http://lv2plug.in/ns/lv2core#index>")
                     .collect::<Vec<&&Lv2Datum>>()
                     .iter()
                     .fold(0_usize, |a, &b| {
                        let b2 = b.object.as_str()[1..].to_string();
                        let i = b2.find('"').expect("{b2}");
                        let b2 = b2.as_str()[..i].to_string();
                        let b2 = b2.as_str().parse::<usize>().expect("{b2} not a usize");
                        a + b2
                     });

                   // Usually more than one type for a port
                   let types: Vec<PortType> = plugin_data
                     .iter()
                     .filter(|l| l.predicate == "<http://www.w3.org/1999/02/22-rdf-syntax-ns#type>")
                     .collect::<Vec<&&Lv2Datum>>()
                     .iter()
                     .map(|&l| {
                        let i = l.object.find('#').unwrap();
                        let j = l.object.rfind('>').unwrap();
                        match &l.object.as_str()[(i + 1)..j] {
                           // Input,
                           // Output,
                           // Control,
                           // Audio,
                           // Other(String),
                           "InputPort" => PortType::Input,
                            "ControlPort" => {
										  // eprintln!("Control port: {name} {symbol}");
										  let (min, max, default, logarthmic, scale, tp) = get_mmdls(&plugin_data, &lv2_data);
										  PortType::Control(ControlPortProperties::new(min, max, default, logarthmic,  scale.clone(), tp,))
									 },
                           "OutputPort" => PortType::Output,
                           "AudioPort" => PortType::Audio,
                           "AtomPort" => PortType::AtomPort,
                           x => PortType::Other(x.to_string()),
                        }
                     })
                     .collect();

						 Port {
                     symbol,
                     name,
                     index,
                     types,
                  }
               })
               .collect::<Vec<Port>>();
         };

         let url = l.subject.as_str()[1..(l.subject.len() - 1)].to_string();
         if !plugin_name.is_empty() {
            let lv2 = Lv2 {
               url,
               types: plugin_type,
               ports: plugin_ports,
               name: plugin_name,
            };
            simulators.push(lv2);
         }
      }
   }

   eprintln!("Found {} simulators", simulators.len());

   // Run the mod-host sub-process
   // pub fn run_executable(path: &str, input_rx: Receiver<Vec<u8>>, output_tx: Sender<Vec<u8>>) {
   let (input_tx, input_rx): (Sender<Vec<u8>>, Receiver<Vec<u8>>) = channel();
   let (output_tx, output_rx): (Sender<Vec<u8>>, Receiver<Vec<u8>>) = channel();

   // Spawn the run_executable function in a separate thread
   let mod_host_th: thread::JoinHandle<()> = thread::spawn(move || {
      run_executable(
         "/home/puppy/mod-host/mod-host",
         &vec!["-i", "-n"],
         input_rx,
         output_tx,
      );
   });

   let result = ModHostController {
      mod_host_th,
      simulators,
      input_tx,
      output_rx,
      last_mh_command: None,
      mh_command_queue: VecDeque::new(),
   };
   {
      // Ensure mod-host is going.  This is taking a gamble.  The
      // gamble is that we will get the whole response all at once.
      let resp = result.get_data()?;
      // const MOD_HOST: &str = "mod-host> ";
      const MOD_HOST: &str = "mod-host>";
      let resp = resp.as_str().trim();
      if resp != MOD_HOST {
         panic!("Unknown response: '{resp}'.  Not: '{MOD_HOST}'");
      }
      eprintln!("Channel working: {resp}");
   }
   Ok(result)
   // // Send a command
   // input_tx
   //     .send(b"add http://guitarix.sourceforge.net/plugins/gx_redeye#chump 1\n".to_vec())
   //     .unwrap();
   // // Start interacting with the user
   // let resp = match output_rx.recv() {
   //     Ok(t) => t,
   //     Err(err) => panic!("{err}: Waiting for mod-host"),
   // };
   // let resp = trunc_vec_0(resp);

   // let resp = match String::from_utf8(resp) {
   //     Ok(s) => s,
   //     Err(err) => panic!("{err} Cannot translate resppone"),
   // };
   // println!("Got {resp}");
}

/// Unicode constants for display
const LESSEQ: &str = "\u{2a7d}"; // <=
const LOG: &str = "\u{33d2}"; // log

impl fmt::Display for ControlPortProperties {
   fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
      write!(
         f,
         "{}",
         match self {
            ControlPortProperties::Continuous(c) => {
               format!(
                  "{} {} {LESSEQ} {}  {LESSEQ} {} {}",
                  match c.kind {
                     ContinuousType::Integer => "Int",
                     ContinuousType::Decimal => "Dec",
                     ContinuousType::Float => "F  ",
                  },
                  c.min,
                  c.default,
                  c.max,
                  if c.logarithmic { LOG } else { "" }
               )
            }
            ControlPortProperties::Scale(s) => format!(
               "Scale: {}",
               s.labels_values
                  .iter()
                  .fold(String::new(), |a, b| format!("{a} {}/{}", b.0, b.1))
            ),
         }
      )
   }
   //     write!(
   //       f,
   //       "{}{} {LESSEQ} {}  {LESSEQ} {}",
   //       if self.logarithmic {
   //          format!["{LOG} "]
   //       } else {
   //          "".to_string()
   //       },
   //       self.min,
   //       self.default,
   //       self.max,
   //    )
   // }
}
impl fmt::Display for PortType {
   fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
      match self {
         PortType::Input => write!(f, "Input"),
         PortType::Output => write!(f, "Output"),
         PortType::Control(properties) => write!(f, "Control({})", properties),
         PortType::Audio => write!(f, "Audio"),
         PortType::AtomPort => write!(f, "AtomPort"),
         PortType::Other(s) => write!(f, "Other({})", s),
      }
   }
}
impl fmt::Display for Port {
   fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
      let port_types: Vec<String> =
         self.types.iter().map(|t| format!("{}", t)).collect();
      write!(
         f,
         "Port {}: {} [{}]",
         self.index,
         self.name,
         port_types.join(", "),
      )
   }
}
impl fmt::Display for Lv2Datum {
   fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
      write!(f, "{} {} {}", self.subject, self.predicate, self.object)
   }
}
impl fmt::Display for Lv2 {
   fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
      write!(
         f,
         "{}: {}{}{}",
         self.name,
         self.url,
         self
            .ports
            .iter()
            .fold("".to_string(), |a, b| format!("{}\n\t{}", a, b)),
         self
            .types
            .iter()
            .fold("".to_string(), |a, b| format!("{}\n\t{:?}", a, b))
      )
   }
}
