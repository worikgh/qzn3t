/// Process LV2 descriptions and simulators
use std::io::StdinLock;
use std::io::Lines;
use core::fmt;
use std::io::Result;
use std::io;
use std::collections::HashSet;
use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver, Sender, };
use std::thread;
use crate::run_executable::{run_executable, trunc_vec_0};

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

#[derive(PartialEq, Debug, PartialOrd)]
struct ControlPortProperties {
    min: f64,
    max: f64,
    default: f64,
    logarithmic: bool,
}

//#[derive(, Eq, Hash, Ord,)]
#[derive(PartialEq, Debug, PartialOrd)]
enum PortType {
    Input,
    Output,
    Control(ControlPortProperties),
    Audio,
    AtomPort,
    Other(String),
}

#[derive(Debug)]
struct Port {
    name: String,
    types: Vec<PortType>,
    index: usize,
}

#[derive(Debug)]
pub struct Lv2 {
    pub types: HashSet<Lv2Type>,
    ports: Vec<Port>,
    name: String,
    url: String,
}

#[derive(Debug)]
/// Interface to mod-host
pub struct ModHostController {
    pub simulators:Vec<Lv2>,
    pub mod_host_th:thread::JoinHandle<()>,
    input_tx:Sender<Vec<u8>>, // Send data to mod-host
    output_rx:Receiver<Vec<u8>>, // Get data from mod-host
    
}

impl ModHostController {

    /// Get a response from mod-host if one is available.  Will not
    /// block.  Will return what is available.  May not be a complete
    /// response
    pub fn get_data_nb(&self) -> Result<String> {
	let resp = match self.output_rx.recv() {
            Ok(t) => t,
            Err(err) =>  return Err(io::Error::new(io::ErrorKind::Other, err.to_string())),
	};

	let resp = trunc_vec_0(resp);
	match String::from_utf8(resp) {
            Ok(s) => Ok(s),
            Err(err) => Err(io::Error::new(io::ErrorKind::InvalidData, err.to_string()))
	}
    }
}

/// Stores all the data required to run LV2 simulators
#[derive(PartialEq, PartialOrd)]
struct Lv2Datum {
    subject: String,
    predicate: String,
    object: String,
}

/// Unicode constants for display
const LESSEQ: &str = "\u{2a7d}"; // <=
const LOG: &str = "\u{33d2}"; // log

impl fmt::Display for ControlPortProperties {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}{} {LESSEQ} {}  {LESSEQ} {}",
            if self.logarithmic {
                format!["{LOG} "]
            } else {
                "".to_string()
            },
            self.min,
            self.default,
            self.max,
        )
    }
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
        let port_types: Vec<String> = self.types.iter().map(|t| format!("{}", t)).collect();
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
            self.ports
                .iter()
                .fold("".to_string(), |a, b| format!("{}\n\t{}", a, b)),
            self.types
                .iter()
                .fold("".to_string(), |a, b| format!("{}\n\t{:?}", a, b))
        )
    }
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

/// For strings start `"1.0"^^<htt...` And the first quoted part
/// (1.0) is wanted.  Panic if invalid string passed
fn remove_quotes<'a>(inp: &'a str) -> &'a str {
    let i = inp[1..].find('"').unwrap() + 1;
    &inp[1..i]
}

/// Numbers for control ports are in the data often without a decimal
/// point.  This takes a LV2 object string and extracts the number.
/// Or panics.
fn number(object: &str) -> f64 {
    let b = remove_quotes(object);
    match b.find(|c| c != '.' && c != '+') {
        Some(_) => b.parse::<f64>().expect(format!("Failed: {b}").as_str()),
        None => b.parse::<isize>().expect(format!("Failed: {b}").as_str()) as f64,
    }
}
pub fn get_lv2_controller(lines:Lines<StdinLock>) -> Result<ModHostController>{
    let mut lv2_data: Vec<Lv2Datum> = vec![];
    let mut subject_store: HashMap<String, usize> = HashMap::new();
    let mut predicate_store: HashMap<String, usize> = HashMap::new();
    let mut object_store: HashMap<String, usize> = HashMap::new();

    let mut index_sbj = 0;
    for line in lines.map(|x| x.unwrap()) {
        let mut split: Vec<&str> = line.as_str().split(' ').collect();
        let subject = split.remove(0).to_string();
        let predicate = split.remove(0).to_string();
        // print!("split {} left: ", split.len());
        let _object = split.join(" ");
        let object = _object.as_str()[..(_object.len() - 2)].to_string();
        // println!("'{subject}' / '{predicate}' / '{object}'");
        if subject_store.get(&subject).is_none() {
            // Firest time a subjecty seen.  Make a simulator for
            // it. This might not be the right thing to do....

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

        lv2_data.push(Lv2Datum {
            subject,
            predicate,
            object,
        });
    }

    // Keep track of which subjects have been processed
    let mut processed: HashSet<&String> = HashSet::new();

    // Keep track of the simulators to put into the result
    let mut simulators:Vec<Lv2> = vec!();
    for l in lv2_data.iter() {
        if &l.object == "<http://lv2plug.in/ns/lv2core#Plugin>" {
            if !processed.insert(&l.subject) {
                // Thi ssubject has been processed
                continue;
            }
            // Collect all data for this plugin
            let plugin_data: Vec<&Lv2Datum> = lv2_data
                .iter()
                .filter(|lv| lv.subject == l.subject)
                .collect();
            // Get name
            // http://usefulinc.com/ns/doap#name>
            let name = plugin_data
                .iter()
                .filter(|lv| lv.predicate == "<http://usefulinc.com/ns/doap#name>")
                .collect::<Vec<&&Lv2Datum>>()
                .iter()
                .fold("".to_string(), |a, &b| {
                    // println!("Fold {a} + {}/{}", b.object, &b.object.as_str()[1..(b.object.len() - 1)]);
                    a + &b.object.as_str()[1..(b.object.len() - 1)]
                });
            // println!("{name} {} {} {}", l.subject, l.predicate, l.object);
            // Collect all types
            let types: HashSet<Lv2Type> = plugin_data
                .iter()
                .filter(|lv| lv.predicate == "<http://www.w3.org/1999/02/22-rdf-syntax-ns#type>")
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

            let ports: Vec<Port>;
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

                // Process the ports
                // Each port  has a subject like `_:gx_zita_rev1b9`.  Usually about two dozen lines that describe a port

                ports = port_names
                    .iter()
                    .map(|p| {
                        // :Vec<Vec<Port>> `p`
                        // is a String and the subject of the lines that
                        // define this port.  Make a Vec<Port> here
                        let l = lv2_data
                            .iter()
                            .filter(|&x| &x.subject == p)
                            .collect::<Vec<&Lv2Datum>>();
                        // `l` is the set of tripples that define this port
                        let name: String = l
                            .iter()
                            .filter(|&l| l.predicate == "<http://lv2plug.in/ns/lv2core#name>")
                            .collect::<Vec<&&Lv2Datum>>()
                            .iter()
                            .fold(String::new(), |a, b| {
                                // println!("Name: '{a}' + '{}'", b.object);
                                a + remove_quotes(b.object.as_str())
                            });

                        let min: f64 =
                            predicate_filter(l.iter(), "<http://lv2plug.in/ns/lv2core#minimum>")
                                .iter()
                                .fold(0.0, |a, b| a + number(&b.object.as_str()));
                        let max: f64 = l
                            .iter()
                            .filter(|&l| l.predicate == "<http://lv2plug.in/ns/lv2core#maximum>")
                            .collect::<Vec<&&Lv2Datum>>()
                            .iter()
                            .fold(0.0, |a, b| a + number(b.object.as_str()));
                        let default: f64 =
                            predicate_filter(l.iter(), "<http://lv2plug.in/ns/lv2core#default>")
                                .iter()
                                .fold(0.0, |a, b| a + number(b.object.as_str()));
                        let logarithmic: bool = predicate_filter(
                            l.iter(),
                            "<http://lv2plug.in/ns/lv2core#portProperty>",
                        )
                        .iter()
                        .filter(|lv| {
                            lv.object == "<http://lv2plug.in/ns/ext/port-props#logarithmic>"
                        })
                        .collect::<Vec<&&&Lv2Datum>>()
                        .len()
                            > 0;
                        let index: usize = l
                            .iter()
                            .filter(|&l| l.predicate == "<http://lv2plug.in/ns/lv2core#index>")
                            .collect::<Vec<&&Lv2Datum>>()
                            .iter()
                            .fold(0_usize, |a, &b| {
                                let b2 = b.object.as_str()[1..].to_string();
                                let i = b2.find('"').expect("{b2}");
                                let b2 = b2.as_str()[..i].to_string();
                                let b2 = b2
                                    .as_str()
                                    .parse::<usize>()
                                    .expect(format!("{b2}").as_str());
                                a + b2
                            });
                        let types: Vec<PortType> = l
                            .iter()
                            .filter(|l| {
                                l.predicate == "<http://www.w3.org/1999/02/22-rdf-syntax-ns#type>"
                            })
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
                                    "ControlPort" => PortType::Control(ControlPortProperties {
                                        min,
                                        max,
                                        default,
                                        logarithmic,
                                    }),
                                    "OutputPort" => PortType::Output,
                                    "AudioPort" => PortType::Audio,
                                    "AtomPort" => PortType::AtomPort,
                                    x => PortType::Other(x.to_string()),
                                }
                            })
                            .collect();
                        Port { name, index, types }
                    })
                    .collect::<Vec<Port>>();
            };
            let url = l.subject.as_str()[1..(l.subject.len() - 1)].to_string();
            if name.len() > 0 {
                let lv2 = Lv2 {
                    url,
                    types,
                    ports,
                    name,
                };
                simulators.push(lv2);
            }
        }
    }
    // for s in simulators.iter() {
    //     println!("{s}");
    // }
    // println!("Found {} simulators", simulators.len());

    // Run the mod-host sub-process
    // pub fn run_executable(path: &str, input_rx: Receiver<Vec<u8>>, output_tx: Sender<Vec<u8>>) {
    let (input_tx, input_rx): (Sender<Vec<u8>>, Receiver<Vec<u8>>) = channel();
    let (output_tx, output_rx): (Sender<Vec<u8>>, Receiver<Vec<u8>>) = channel();

    // Spawn the run_executable function in a separate thread
    let mod_host_th:thread::JoinHandle<()> = thread::spawn(move || {
        run_executable(
            "/home/puppy/mod-host/mod-host",
            &vec!["-i", "-n"],
            input_rx,
            output_tx,
        );
    });
    let result = ModHostController{
	mod_host_th,
	simulators,
	input_tx,
	output_rx,
    };
    {
	// Ensure mod-host is going.  This is taking a gamble.  The
	// gamble is that we will getthe whole response all at once.
	let resp = result.get_data_nb()?;
	// const MOD_HOST: &str = "mod-host> ";
	const MOD_HOST: &str = "mod-host>";
	let resp = resp.as_str().trim();
	if resp != MOD_HOST {
            panic!("Unknown response: '{resp}'.  Not: '{MOD_HOST}'");
	}
	println!("Channel working: {resp}");
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