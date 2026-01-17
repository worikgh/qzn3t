// Copyright (c) 2025 Worik Turei Stanton
// License: GPL-3.0

//! Manipulate the pipes that drive the audio through the pedals.
use jack::Error;
#[derive(Debug)]
pub struct JackConnections {
    client: jack::Client,
}
impl JackConnections {
    pub fn unmake_connection(&mut self, src: &str, dst: &str) -> Result<(), Error> {
        self.client.disconnect_ports_by_name(src, dst)
    }
    pub fn make_connection(&mut self, src: &str, dst: &str) -> Result<(), Error> {
        self.client.connect_ports_by_name(src, dst)?;
        Ok(())
    }
    pub fn new(client_name: &str) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(JackConnections {
            client: jack::Client::new(client_name, jack::ClientOptions::NO_START_SERVER)?.0,
        })
    }
}
