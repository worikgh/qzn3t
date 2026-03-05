// Copyright (c) 2026 Worik Turei Stanton
// License: GPL-3.0

use jack::{AsyncClient, AudioIn, AudioOut, Client, ClientOptions, Port, PortFlags, Unowned};
#[allow(unused_imports)]
use qzn3t_audio_buffer::AudioBuffer;
use qzn3terror::Qzn3tError;
#[allow(unused_imports)]
use std::{
    fmt,
    path::Path,
    sync::{
	Arc,
	atomic::{AtomicBool, Ordering},
	mpsc,
    },
};

#[derive(Debug)]
#[allow(dead_code)]
pub struct Engine {
    /// The Jack Client
    client: Option<AsyncClient<Notifications, ProcessAudio>>,
    /// Audio data being sent into the engine
    receivers: Vec<mpsc::Receiver<f32>>,
    /// Audio data output from engine
    senders: Vec<mpsc::Sender<f32>>,
    /// "Power" switch.  When reset the client shuts down
    run_f: Arc<AtomicBool>,
    /// When `Engine` is active it will have an `AudioBuffer`
    audio_buffer: Option<AudioBuffer>,
}
impl Engine {
    /// The `Engine` constructor.
    pub fn new() -> Result<Self, Qzn3tError> {
	// The "power switch".  Run flag...
	let run_f = Arc::new(AtomicBool::new(true));

	Ok(Self {
	    client: None,
	    senders: vec![],
	    receivers: vec![],
	    run_f,
	    audio_buffer: None,
	})
    }

    /// Set up a new AsyncClient.
    #[allow(dead_code)]
    fn add_client(&mut self, in_p: &[&str], out_p: &[&str]) -> Result<(), Qzn3tError> {
	if let Some(c) = self.client.take() {
	    c.deactivate()?;
	}
	let name = "qzn3t";
	let (client, _status) = match Client::new(name, ClientOptions::NO_START_SERVER) {
	    Ok(c) => c,
	    Err(err) => {
		let msg = format!("jack_rec read_port: Error creating client: {err}");
		return Err(Qzn3tError::JackClient(msg));
	    }
	};

	let (process_audio, senders, receivers) =
	    Self::create_process(&client, in_p, out_p, self.run_f.clone())?;
	let async_client = match client.activate_async(Notifications, process_audio) {
	    Ok(ac) => ac,
	    Err(err) => {
		return Err(Qzn3tError::JackClient(format!(
		    "Failed to create asynchronous client for {name}.  {err}"
		)));
	    }
	};
	self.client = Some(async_client);
	self.senders = senders;
	self.receivers = receivers;
	Ok(())
    }

    /// Start saving the audio data from the inputs set up
    #[allow(unused_variables)]
    pub fn start_recording(&mut self, ch_count: usize, path: &Path) -> Result<(), Qzn3tError> {
	// Get/set up the input device
	unimplemented!();
    }

    // Getters
    pub fn client(&self) -> Option<&Client> {
	match &self.client {
	    Some(c) => Some(c.as_client()),
	    None => None,
	}
    }

    /// This is the name by which this engine's Jack client is known
    pub fn name(&self) -> Option<&str> {
	if let Some(c) = self.client() {
	    Some(c.name())
	} else {
	    None
	}
    }
}

/// Private methods
impl Engine {
    /// Set up the Process structure for Jack
    #[allow(clippy::type_complexity)]
    #[allow(dead_code)]
    fn create_process(
	client: &Client,
	in_p: &[&str],
	out_p: &[&str],
	run_f: Arc<AtomicBool>,
    ) -> Result<
	(
	    ProcessAudio,
	    Vec<mpsc::Sender<f32>>,
	    Vec<mpsc::Receiver<f32>>,
	),
	Qzn3tError,
    > {
	let mut ports_receivers = vec![];
	let mut ports_senders = vec![];
	let mut senders = vec![];
	let mut receivers = vec![];
	for p in in_p.iter() {
	    let port = client.register_port(p, AudioIn::default())?;
	    let (tx, rx) = mpsc::channel::<f32>();
	    receivers.push(rx);
	    ports_senders.push((port, tx));
	}
	for p in out_p.iter() {
	    let port = client.register_port(p, AudioOut::default())?;
	    let (tx, rx) = mpsc::channel::<f32>();
	    senders.push(tx);
	    ports_receivers.push((port, rx));
	}
	Ok((
	    ProcessAudio {
		run_f,
		// Get data from/ send data to owner
		ports_receivers,
		ports_senders,
	    },
	    // For owner to use to send/reveive data
	    senders,
	    receivers,
	))
    }

}

//-------------------
// Jackd code: TODO: move this to its own unit
pub struct Notifications;
impl jack::NotificationHandler for Notifications {
    fn thread_init(&self, _: &jack::Client) {
	// eprintln!("DBG NotificationHandler thread_init");
    }
    unsafe fn shutdown(&mut self, _status: jack::ClientStatus, _reason: &str) {
	// eprintln!("DBG NotificationHandler shutdown {_status:?} {_reason}");
    }
    fn freewheel(&mut self, _: &jack::Client, _is_freewheel_enabled: bool) {
	// eprintln!("DBG NotificationHandler freewheel {_is_freewheel_enabled}");
    }
    fn client_registration(&mut self, _: &jack::Client, _name: &str, _is_registered: bool) {
	// eprintln!("DBG NotificationHandler client_registration: {_name}/{_is_registered}");
    }
    fn port_registration(
	&mut self,
	_: &jack::Client,
	_port_id: jack::PortId,
	_is_registered: bool,
    ) {
	// eprintln!("DBG NotificationHandler port_registration: {_port_id}/{_is_registered}");
    }
    fn port_rename(
	&mut self,
	_: &jack::Client,
	_port_id: jack::PortId,
	_old_name: &str,
	_new_name: &str,
    ) -> jack::Control {
	// eprintln!("DBG NotificationHandler port_rename: {_port_id} {_old_name} -> {_new_name}");
	jack::Control::Continue
    }
    fn ports_connected(
	&mut self,
	_: &jack::Client,
	_port_id_a: jack::PortId,
	_port_id_b: jack::PortId,
	_are_connected: bool,
    ) {
	// eprintln!(
	//     "DBG NotificationHandler: ports_connected {_port_id_a}/{_port_id_b} {_are_connected}"
	// );
    }
    fn graph_reorder(&mut self, _: &jack::Client) -> jack::Control {
	// eprintln!("DBG NotificationHandler graph_reorder");
	jack::Control::Continue
    }

    fn xrun(&mut self, _: &jack::Client) -> jack::Control {
	eprintln!("Error NotificationHandler xrun");
	jack::Control::Continue
    }

    fn sample_rate(&mut self, _: &jack::Client, _srate: jack::Frames) -> jack::Control {
	jack::Control::Continue
    }
}

/// Handler for audio output.  Receive multi-channel audio data on a
/// set of `mpsc::Channel`s and send them to matching Jack ports
#[derive(Debug)]
pub struct ProcessAudio {
    run_f: Arc<AtomicBool>,
    /// Receive Audiop from the code that owns this `Engine` and send
    /// it out on Jack outputs
    ports_receivers: Vec<(jack::Port<jack::AudioOut>, mpsc::Receiver<f32>)>,
    /// Receive Audiop on Jack inputs and send it to the code that
    /// owns this `Engine`
    ports_senders: Vec<(jack::Port<jack::AudioIn>, mpsc::Sender<f32>)>,
}

impl jack::ProcessHandler for ProcessAudio {
    /// Full duplex audio.  Audio Output: If there are audio data in
    /// the receiver channels send it out on the associated port, if
    /// no audio available send 0_f32.  Audio Input: Any data
    /// available on the input jack ports send them out on the sender
    /// channels
    fn process(&mut self, _c: &jack::Client, ps: &jack::ProcessScope) -> jack::Control {
	for (port, receiver) in self.ports_receivers.iter_mut() {
	    let out = port.as_mut_slice(ps);
	    for s in out.iter_mut() {
		*s = match receiver.try_recv() {
		    Ok(s) => s,
		    Err(mpsc::TryRecvError::Empty) => 0.0,
		    Err(mpsc::TryRecvError::Disconnected) => {
			eprintln!("jack_rec. Error: Disconnected audio channel");
			return jack::Control::Quit;
		    }
		};
	    }
	}
	for (port, sender) in self.ports_senders.iter_mut() {
	    let in_a_p: &[f32] = port.as_slice(ps);
	    for v in in_a_p {
		if let Err(err) = sender.send(*v) {
		    eprintln!(
			"Error jack_rec: Cannot send data ({v}) through a channel. Error: {err} "
		    );
		    return jack::Control::Quit;
		}
	    }
	}

	if !self.run_f.load(Ordering::SeqCst) {
	    jack::Control::Quit
	} else {
	    jack::Control::Continue
	}
    }
}
