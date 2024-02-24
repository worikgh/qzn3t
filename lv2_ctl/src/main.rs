use core::fmt;
use std::collections::HashMap;
use std::collections::HashSet;
use std::io;

#[derive(Debug)]
enum Lv2Type {
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
    Other(String),
}
#[derive(Debug)]
enum PortType {
    Input,
    Output,
    Control,
    Audio,
    AtomPort,
    Other(String),
}

#[derive(Debug)]
struct Port {
    name: String,
    index: usize,
    style: Vec<PortType>,
}

#[derive(Debug)]
struct Lv2 {
    style: Vec<Lv2Type>,
    ports: Vec<Port>,
    name: String,
}
/// Stores all the data required to run LV2 simulators
#[derive(PartialEq, PartialOrd)]
struct Lv2Datum {
    subject: String,
    predicate: String,
    object: String,
}
impl fmt::Display for Lv2Datum {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {} {}", self.subject, self.predicate, self.object)
    }
}
impl fmt::Display for Port {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {}: {}",
            self.index,
            self.name,
            self.style
                .iter()
                .map(|s| format!("{s:?}"))
                .collect::<Vec<String>>()
                .join(", ")
        )
    }
}

impl fmt::Display for Lv2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}{}{}",
            self.name,
            self.ports
                .iter()
                .fold("".to_string(), |a, b| format!("{}\n\t{}", a, b)),
            self.style
                .iter()
                .fold("".to_string(), |a, b| format!("{}\n\t{:?}", a, b))
        )
    }
}
fn main() -> std::io::Result<()> {
    let mut simulators: Vec<Lv2> = Vec::new();

    // Store processed plugins so only get procerssed once
    let mut processed: HashSet<&String> = HashSet::new();
    let lines = io::stdin().lines();
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

    for l in lv2_data.iter() {
        if &l.object == "<http://lv2plug.in/ns/lv2core#Plugin>" {
            if !processed.insert(&l.subject) {
                // println!("Processed: {}", l.subject);
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
            let style: Vec<Lv2Type> = plugin_data
                .iter()
                .filter(|lv| lv.predicate == "<http://www.w3.org/1999/02/22-rdf-syntax-ns#type>")
                .collect::<Vec<&&Lv2Datum>>()
                .iter()
                .map(|l| {
                    let i = l.object.find('#').unwrap();
                    let j = l.object.rfind('>').unwrap();
                    match &l.object.as_str()[(i + 1)..j] {
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
			println!("Port: {p}");
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
				println!("Name: '{a}' + '{}'", b.object);
				a + b.object.as_str()
			    });
                        let index: usize = l
                            .iter()
                            .filter(|&l| l.predicate == "<http://lv2plug.in/ns/lv2core#index>")
                            .collect::<Vec<&&Lv2Datum>>()
                            .iter()
                            .fold(0_usize, |a, &b| {
                                let b2 = b.object.as_str()[1..].to_string();
                                let i = b2.find('"').expect("{b2}");
                                let b2 = b2.as_str()[..i].to_string();
                                let b2 = b2.as_str().parse::<usize>().unwrap();
                                a + b2
                            });
                        let style: Vec<PortType> = l
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
                                    "ControlPort" => PortType::Control,
                                    "OutputPort" => PortType::Output,
                                    "AudioPort" => PortType::Audio,
                                    "AtomPort" => PortType::AtomPort,
                                    x => PortType::Other(x.to_string()),
                                }
                            })
                            .collect();
                        Port { name, index, style }
                    })
                    .collect::<Vec<Port>>();
            };
            // eprintln!("Simulator: {name}");
            simulators.push(Lv2 { style, ports, name });
        }
    }
    for s in simulators.iter() {
        println!("{s}");
    }
    println!("Found {} simulators", simulators.len());
    Ok(())
}
