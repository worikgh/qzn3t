# The Tuner Saga

	* Use Rust crate `pitch_detector`
	* How hard can it be?
	
```rust
	use pitch_detector::{
    	note::{NoteDetectionResult, detect_note},
		pitch::HannedFftDetector,
	}
	:
	:
	let mut detector = HannedFftDetector::default();
	let note = detect_note(signal, &mut detector, sample_rate);
```

Simple?

## Results

Using a synthesised organ C note:

```
tuner:   B/4 41.776  max: 0.070919 min: -0.125513 mean: -0.000353 0.565033
tuner:   C/3 2.2532  max: 0.099963 min: -0.114054 mean: 0.000938 0.876456
tuner:   B/3 15.964  max: 0.077315 min: -0.068086 mean: 0.001769 1.135562
tuner:   C/3 12.635  max: 0.055838 min: -0.053299 mean: -0.001491 1.047649
tuner:   C/4 -35.94  max: 0.041400 min: -0.028149 mean: 0.001519 1.470752
tuner:   C/3 -45.12  max: 0.038762 min: -0.016929 mean: 0.000608 2.289744
tuner:   C/3 -3.627  max: 0.010289 min: -0.019798 mean: -0.000310 0.519710
tuner:   C/3 -22.99  max: 0.008281 min: -0.008927 mean: 0.000246 0.927538
tuner:   C/3 36.405  max: 0.005995 min: -0.005069 mean: 0.000010 1.182722
tuner:  C#/3 -40.42  max: 0.005634 min: -0.002556 mean: -0.000098 2.204298
tuner:   B/3 11.743  max: 0.002338 min: -0.002131 mean: 0.000094 1.097048
tuner:   C/3 4.5210  max: 0.001712 min: -0.001672 mean: 0.000022 1.024148
tuner:   C/3 0.2358  max: 0.000969 min: -0.001043 mean: -0.000040 0.928960
tuner:   B/3 44.931  max: 0.000830 min: -0.000756 mean: 0.000038 1.097923
tuner:  C#/3 -16.18  max: 0.000549 min: -0.000443 mean: -0.000003 1.238867
tuner:   C/3 41.866  max: 0.000287 min: -0.000367 mean: -0.000007 0.783306
tuner:   C/3 11.910  max: 0.000211 min: -0.000131 mean: 0.000007 1.611835
tuner:   C/3 10.834  max: 0.000157 min: -0.000125 mean: -0.000003 1.261370
tuner:  C#/3 40.877  max: 0.000059 min: -0.000141 mean: -0.000003 0.414934
```

