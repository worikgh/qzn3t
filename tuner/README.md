# Tuner

Part of the Qzn3t collection of musical utilities

Utilises the `pitch-detector` crate

## Usage

The function `get_results(&TunerArgs, mpsc::Sender<TunerData>)` starts a [jack](https://docs.rs/jack/latest/jack/) client and an import port `qzn3t_tuner:input`.

The `&TunerArgs` parameter controls the paramneters of the [`pitch_detector`](https://docs.rs/pitch-detector/latest/pitch_detector/)

```rust
pub struct TunerArgs {
	pub interval: u64,
	pub buffer_size: u64,
	pub max_vol_min: f32,
	pub mean_min: f32, // The absolute mean volume must be smaller than this
}
```
* `interval`: The period in milli-seconds between sampling the Jack data
* `buffer_size` The size of the ring buffer used to store Jack samples
* `max_vol_min`: The minimum value of the maximum volume in the sample for there to be an attempt to gage the frequency
* `mean_min`: This needs ot be deprecated

Then it sends a stream of `TunerData` objects down the supplied channel.


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
