//! Functions to handle MIDI connections.
//! Abstraction for the MIDI
//! This should be a trait
//! * Connect two devices
//! * disconnect two devices
//! * List all connections  from a device
//! List all devices
//! Each device is expressed as a string "<Client name>:<Port Name>"
#[derive(Debug)]
pub enum MidiError {
    // The call to the system's MIDI failed.
    OsFailure(String),
    BadMidiDevice(String),
}

/// Convert a named device: "<Client name>:<Port Name>" to a port:
/// "\d+:\d+"
fn name_to_numeric(_name: &str) -> String {
    unimplemented!();
}

/// Depending on `aconnect` from ALSA, get the raw output
fn raw_aconnect() -> Result<String, MidiError> {
    use std::process::Command;
    let output = match Command::new("aconnect").arg("-l").output() {
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

pub fn connect(_lhs: &str, _rhs: &str) -> Result<(), MidiError> {
    unimplemented!();
}

pub fn disconnect(_lhs: &str, _rhs: &str) -> Result<(), MidiError> {
    unimplemented!();
}

pub fn list_connections(_device: &str) -> Result<Vec<String>, MidiError> {
    unimplemented!();
}
pub fn list_devices() -> Result<Vec<String>, MidiError> {
    // client 0: 'System' [type=kernel]
    //     0 'Timer           '
    //         Connecting To: 142:0
    //     1 'Announce        '
    //         Connecting To: 142:0, 128:0
    // client 14: 'Midi Through' [type=kernel]
    //     0 'Midi Through Port-0'
    // client 24: 'Launchpad X' [type=kernel,card=2]
    //     0 'Launchpad X LPX DAW In'
    //     1 'Launchpad X LPX MIDI In'
    // client 28: 'WORLDE' [type=kernel,card=3]
    //     0 'WORLDE MIDI 1   '
    // client 142: 'PipeWire-System' [type=user,pid=1018]
    //     0 'input           '
    //         Connected From: 0:1, 0:0
    // client 143: 'PipeWire-RT-Event' [type=user,pid=1018]
    //     0 'input           '
    let mut result:Vec<String> = vec!{};
    let binding = raw_aconnect()?;
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
                        client = line[(start+1)..end].trim().to_string();
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
	    if let Err(_) = port_num.parse::<usize>(){
		// Something else.
		continue;
	    }
	    if let Some(end) = line.rfind('\'') {
		if start < end {
		    let port = line[(start+1)..end].trim();
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
    fn test_list_devices(){
	let devices = list_devices().unwrap();
	for dev in devices.iter() {
	    println!("Device: {dev}");
	}
	assert!(devices.len() > 0);
    }
    #[test]
    fn test_raw_aconnect() {
        let connect = raw_aconnect().unwrap();
        println!("{connect}");
        assert!(connect.len() != 0);
    }
}
