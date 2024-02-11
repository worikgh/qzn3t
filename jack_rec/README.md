# Record Jackd Outputs

A very simple programme that finds all Jackd ports sending data to the output (defined as ports named "playback_N") and records all the data sent through them.

## Argumnts

The one argument is a prefix to use when output files.  If a prefix is used twice, the second run could easilly overwrite data from the first run.

If no argument passed a timestamp (to the second) is used as a prefix

## Outputs 

Each channel being monitored is output to a file named with the `prefix` (above) and the name of the port.

The data is 1-channel raw audio.

When `jack_rec` finishes it prints a JSON object containing the sample rate and an array of paths to the recorded files.  This is what is needed to convert them to more useful formats



## Control

The programme runs all recordings in threads, a thread (via `jack::AsyncClient` and `jack::ProcessHandler`).  The main thread blocks on stdin, effectively waiting for a key press.
