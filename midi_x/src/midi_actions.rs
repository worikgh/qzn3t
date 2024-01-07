//! Functions to handle MIDI connections.
//! Abstraction for the MIDI
//! This should be a trait
//! * Connect two devices
//! * disconnect two devices
//! * List all connections  from a device
//! List all devices
//! Each device is expressed as a string "<Client name>:<Port Name>"

use std::collections::HashMap;
#[derive(Debug)]
pub enum MidiError {
    // The call to the system's MIDI failed.
    OsFailure(String),
    BadMidiDevice(String),
    BadName(String),
}

/// Convert a named device: "<Client name>:<Port Name>" to a port:
/// "\d+:\d+"
fn name_to_numeric(_name: &str) -> String {
    unimplemented!();
}

/// Depending on `aconnect` from ALSA, get the raw output
fn raw_aconnect_l() -> Result<String, MidiError> {
    raw_aconnect_cmds(vec!["-l"])
}

/// Make a connections
fn raw_connect(lhs: &str, rhs: &str) -> Result<(), MidiError> {
    _ = raw_aconnect_cmds(vec![lhs, rhs])?;
    Ok(())
}

fn raw_disconnect(lhs: &str, rhs: &str) -> Result<(), MidiError> {
    _ = raw_aconnect_cmds(vec!["-d", lhs, rhs])?;
    Ok(())
}

fn raw_aconnect_cmds(cmds: Vec<&str>) -> Result<String, MidiError> {
    use std::process::Command;
    let mut command: Command = Command::new("aconnect");
    command.args(cmds);

    let output = match command.output() {
        // Executes the command as a child process, waiting for it to
        // finish and collecting all of its output.
        Ok(o) => o,
        Err(err) => return Err(MidiError::OsFailure(format!("{err}"))),
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(MidiError::OsFailure(format!("{}", stderr)));
    }
    let stdout: String = String::from_utf8_lossy(&output.stdout).to_string();
    return Ok(stdout);
}

/// Convert the raw output from `aconnect -l` passed as `input` into a
/// hashmap to convert devices as clint/port number into strings
fn map_midi_ports(input: &str) -> HashMap<(u8, u8), String> {
    let mut result = HashMap::new();

    // Keep track of the current client
    let mut current_client: Option<(u8, String)> = None;

    for line in input.lines() {
        let line = line.trim();
        if line.starts_with("client") {
            match line.split_whitespace().nth(1) {
                Some(client_number) => {
                    let name_start = line.find('\'').unwrap() + 1;
                    let name_end = line.rfind('\'').unwrap();
                    let client_name = &line[name_start..name_end];
                    let client_number = &client_number[..client_number.len() - 1];

                    let client_number: u8 = match client_number.parse() {
                        Ok(cn) => cn,
                        Err(err) => panic!("{err} Cannot convert client_number: {client_number}"),
                    };
                    current_client = Some((client_number, client_name.to_string()));
                }
                None => current_client = None,
            }
        } else if let Some((client_number, client_name)) = &current_client {
            if line.len() > 0
                && line
                    .trim()
                    .split_ascii_whitespace()
                    .nth(0)
                    .unwrap() // Ok because line not empty
                    .parse::<u8>()
                    .is_ok()
            {
                // Check on a line that starts with a number
                // 0 'Launchpad X LPX DAW In'

                if let Some(port_number) = line.split_whitespace().nth(0) {
                    let name_start = line.find('\'').unwrap() + 1;
                    let name_end = line.rfind('\'').unwrap();
                    let port_name = format!("{}:{}", client_name, &line[name_start..name_end]);
                    result.insert((*client_number, port_number.parse().unwrap()), port_name);
                }
            }
        }
    }

    result
}

/// Connect two devices.
pub fn connect(lhs: &str, rhs: &str) -> Result<(), MidiError> {
    // Check both devices exist
    let devices: Vec<String> = list_devices()?;
    if lhs.starts_with("LpxCtl:colour_port") {
	eprintln!("in disconnect for '{lhs}' / '{rhs}'");
	eprintln!("Devices: {devices:?}");
    }
    if let None = devices.iter().find(|&x| x == lhs) {
        return Err(MidiError::BadName(lhs.to_string()));
    }
    if let None = devices.iter().find(|&x| x == rhs) {
        return Err(MidiError::BadName(rhs.to_string()));
    }
    Ok(raw_connect(lhs, rhs)?)
}

pub fn disconnect(lhs: &str, rhs: &str) -> Result<(), MidiError> {
    // Check both devices exist
    let devices: Vec<String> = list_devices()?;
    if let None = devices.iter().find(|&x| x == lhs) {
        return Err(MidiError::BadName(lhs.to_string()));
    }
    if let None = devices.iter().find(|&x| x == rhs) {
        return Err(MidiError::BadName(rhs.to_string()));
    }
    Ok(raw_disconnect(lhs, rhs)?)
}

/// List all connections to the client named in `client`. (`client`
/// does not include port.  utput does)
pub fn list_connections(client: &str) -> Result<Vec<(String, String)>, MidiError> {
    let binding = raw_aconnect_l()?;
    let lines: Vec<&str> = binding.lines().collect();

    // When we find the client named in `client` set found.  Quit loop
    // on next client or when processed "To" line
    let mut found = false;

    // The connections are expressed as numeric pairs.  These numbers
    // are unstable which is why string definitions of devices are
    // used. So `device_map` matches `(<u8>, <ub>)` that is obtained
    // from the raw output from `aconnect -l` "Connected To" lines
    let device_map = map_midi_ports(binding.as_str());

    // The current port.  Not always set
    let mut current_port: Option<String> = None;
    let mut result: Vec<(String, String)> = Vec::new();
    for line in lines {
        if line.starts_with("client ") {
            if found {
                break;
            }
            if line.contains(client) {
                found = true;
                continue;
            }
        }
        if found
            && line.len() > 0
            && line
                .trim()
                .split_ascii_whitespace()
                .nth(0)
                .unwrap() // Ok because line not empty
                .parse::<u8>()
                .is_ok()
        {
            // Check on a line that starts with a number
            // 0 'Launchpad X LPX DAW In'
            // This is the port that goes with the client
            let start = line.find('\'').unwrap();
            let end = line.rfind('\'').unwrap();
            let port = line[(start + 1)..end].to_string();
            current_port = Some(port);
            continue;
        }
        if found && line.trim().starts_with("Connecting To: ") {
            // Connecting To: 142:0, 128:0

            // Extract the connection strings only
            let connections = line.split(": ").nth(1).unwrap();

            // Split and convert to tuples
            let connected_to_num: Vec<(u8, u8)> = connections
                .split(", ")
                .map(|conn| {
                    let parts: Vec<&str> = conn.split(":").collect();
                    (parts[0].parse().unwrap(), parts[1].parse().unwrap())
                })
                .collect();

            // First collect all devices and
            for pair in connected_to_num.iter() {
                let device: &str = device_map.get(pair).unwrap();
                let lhs = format!("{}:{}", client, current_port.as_ref().unwrap(),);
                let rhs = device.to_string();
                result.push((lhs, rhs));
            }
        }
    }
    Ok(result)
}

/// Return a list of "<device>:<port>" for every device on the system
pub fn list_devices() -> Result<Vec<String>, MidiError> {
    let mut result: Vec<String> = vec![];
    let binding = raw_aconnect_l()?;
    let lines: Vec<&str> = binding.lines().collect();
    let mut client: String = "".to_string();
    for line in lines {
        if line.starts_with("client ") {
            // Client definition line.  E.g:
            // client 143: 'PipeWire-RT-Event' [type=user,pid=1018]
            // Assume only two \' characters
            if let Some(start) = line.find('\'') {
                if let Some(end) = line.rfind('\'') {
                    if start < end {
                        client = line[(start + 1)..end].trim().to_string();
                        continue;
                    }
                }
            }
            // Get to here and line starts with "client" but is invalid
            return Err(MidiError::BadMidiDevice(line.to_string()));
        }
        // Might be port defition.  E.g:
        //     0 'Launchpad X LPX DAW In'

        if let Some(start) = line.find('\'') {
            let port_num = line[0..start].trim();
            if let Err(_) = port_num.parse::<usize>() {
                // Something else.
                continue;
            }
            if let Some(end) = line.rfind('\'') {
                if start < end {
                    let port = line[(start + 1)..end].trim();
                    result.push(format!("{client}:{port}"));
                    continue;
                }
            }
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {

    use super::*;
    #[test]
    fn test_list_devices() {
        let devices = list_devices().unwrap();
        for dev in devices.iter() {
            println!("Device: {dev}");
        }
        assert!(devices.len() > 0);
    }
    #[test]
    fn test_number_name() {
        let binding = raw_aconnect_l().unwrap();
        let numbers_name = map_midi_ports(binding.as_str());
        for (k, v) in numbers_name {
            println!("{}:{}\t{}", k.0, k.1, v);
        }
        assert!(true);
    }
    #[test]
    fn test_raw_aconnect() {
        let connect = raw_aconnect_l().unwrap();
        println!("{connect}");
        assert!(connect.len() != 0);
    }
    #[test]
    fn test_connections() {
        let devices = list_devices().unwrap();
        let mut hm: HashMap<String, bool> = HashMap::new();
        for device in devices {
            let dev = device.split(":").next().unwrap();
            hm.insert(dev.to_string(), true);
        }
        for (k, _) in hm {
            let connections = list_connections(k.as_str()).unwrap();
            println!("{k}");
            for connection in connections {
                println!("\t{connection}");
            }
        }
    }
}
