mod configuration;
mod midi_actions;
use configuration::initialise;
use configuration::Configuration;
use midi_actions::connect;
use midi_actions::disconnect;
use midi_actions::list_connections;
use std::env;
use std::error::Error;
use std::fs::File;
use std::io::Read;
fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().collect();
    let path: &str = args.iter().nth(1).unwrap();
    let mut file = File::open(path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    let config = contents;
    let config: Configuration = initialise(config)?;

    // For all the devices in `config` remove any outgoing MIDI connections
    for device in config.devices.iter() {
        let connections: Vec<(String, String)> = match list_connections(device) {
            Ok(c) => c,
            Err(err) => {
                eprintln!("{err:?}: Getting connections for {device}");
                continue;
            }
        };
        for connection in connections.iter() {
            // println!("disconnect({}, {}", connection.0.as_str(), connection.1.as_str());
        let lhs = connection.0.as_str().trim();
        let rhs = connection.1.as_str().trim();
            match disconnect(lhs, rhs) {
                Ok(()) => (),
                Err(err) => {
                    eprintln!("{err:?}: Failed disconnect({lhs}, {rhs})");
                    continue;
                }
            };
        }
    }
    for c in config.connections {
        let lhs = c[0].as_str().trim();
        let rhs = c[1].as_str().trim();
        // println!("connect({lhs}, {rhs})");
        match connect(lhs, rhs) {
            Ok(()) => (),
            Err(err) => {
                eprintln!("{err:?}: Failed to connect {c:?}");
                continue;
            }
        };
    }
    Ok(())
}
