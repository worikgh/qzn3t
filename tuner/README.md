# Tuner

Part of the Qzn3t collection of musical utilities

Utilises the `pitch-detector` crate

```
	let (sender, receiver) = mpsc::channel::<TunerData>();
	_ = get_results(args, sender);
	loop {
		let tuner_data = match receiver.recv() {
			Ok(r) => r,
			Err(err) => {
				eprintln!("DBG tuner: get_results send error: {err}");
				break;
			}
		};

```
