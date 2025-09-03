# Record Jackd Outputs

A very simple programme that finds all Jackd ports sending data to the output (defined as ports named "playback_N") and records all the data sent through them.

## Argumnts


```
  -p, --prefix <PREFIX>         The output file prefix. Specified at most once.
  -i, --jackd-pipe <PIPE_NAME>  Name of Jackd pipes to monitor. Can occur zero, one or many times.
  -h, --help                    Print help
```


Output File Prefix
---

A prefix to use creating output files.  Can be specified at most once.  If not specified will default to YYYMMDD_hhmmss

Reusing the same prefix will overwrite data.

Jackd Pipes
---

The names of the jack pipes to record.  If not supplied it records all pipes that are sending audio to a `system:playback_N` pipe.


## Outputs

Each channel being monitored is output to a file named with the `prefix` (above) and the name of the port.

The data is 1-channel raw audio.

When `jack_rec` finishes it prints a JSON object containing the sample rate and an array of paths to the recorded files.  This is what is needed to convert them to more useful formats

## Control

The programme runs all recordings in threads, a thread (via `jack::AsyncClient` and `jack::ProcessHandler`).  The main thread blocks on stdin, effectively waiting for a key press.

So any key stope recording
