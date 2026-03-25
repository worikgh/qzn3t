# Qzn3t Engine

A library and a set of executable files that provide sound utilities

This is not a DAW

Depends on [Jack Audio Conection Kit](https://jackaudio.org)

## Design

* Asynchronous using an explicit event loop.
  * 100 times a second(?) Not audio rate, use 10ms blocks of audio
  * A control programme starts `Qzn3t`
  * The event loop runs in a thread
  * Control with a "blackboard" protected with a `Mutex`
	* Event loop uses `try_lock` to read it
	* Control programme uses `lock` to write to it, and blocks while the event loop reads
  * One Jack client per `Engine` instance.
* Initialised with all the resources it needs.
  * Create `Engine` with the `new(Session)` constructor
  * `Session` contains all the information needed to create an `Engine` instance
	* The name of the Jack client
	* The path to where the output files are saved (.json for metadata and .raw for audio)
	* The mode, recording or playing. (? Can it do both at once? Need more paths).
	* In port or out port names, depending on `mode` used to name the Jack pipes
* When recording the event loop processes all audio data available
  * Up to 10ms audio data can be easily stored in the `mpsc::channel<f32>` from the Jack client to Qzn3t
* When playing the event loop sends data at the rate it is played.  (? Stay 10ms ahead to avoid glitches ?)
* `AudioBuffer`s store audio data in `Vec<f32>` per channel
  * A `FileBacker` can be attached to a `AudioBuffer` saves the audio data to disc
* `Engine` holds zero, one or two `AudioBuffer` objects
  * If it has none it is not recording or playing, it must be idle.
  * It can play audio from a buffer out through Jack ports
  * It can record jack inputs to a buffer
  * It can record jack inputs while playing from another buffer
	* In this later case it has two `AudioBuffer`, one that is playing and the one being recorded to.
* `AudioBuffer` objects can be combined.
  * If one is longer that the other then the shorter is padded with silence
	* TODO: Supply an index so a buffers, when combined, do not have to start at same sample time.  Option to prfix with silence.
	* TODO: Create an iterator for accessing the `AudioBuffer` 
* Time is handled very naively.  `Engine` records the `std::time::Instant::now()` it is created.
  * `AudioBuffer` has a sample rate stored so the time each sample happens can be calculated relative to the start
* During playback the channels of an audio buffer are mixed into the engine's outputs

## TODO
