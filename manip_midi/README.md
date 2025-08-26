// Copyright (c) 2025 Worik Turei Stanton
// License: GPL-3.0

# Manipulate MIDI 

Process MIDI using three types of object

1. Producers.  Here `read_midi`
2. Translators. Here `translate_midi`
3. Consumers. Here `command_midi`

## Producer: Read MIDI

src: `src/read.rs`
bin: `read_midi`

Producers open a MIDI device and write MIDI to `stdout`

Arguments:
	* `--list` List the MIDI ports that can be connected to then exit
	* The name of the MIDI input port.  This does not have to be the full name.  The first port where the passed name is a part of the port's name will be used.

## Translator: Translate MIDI
src: `src/translate.rs`
bin: `translate_midi`

Translators read MIDI on `stdin` and write MIDI on `stdout`.

Messages are translated using a set of rules described in the configuration file.

Any message that is not affected by a rule is passed through uncchanges

`translate_midi` takes one argument: The name of the configuration file.

The translation process does not effect System Exclusive (SysEx) messages

### Configuration File

There are two types of rule:

1. Translate. The configuration line of the form: "t s x n m"
* `t` the character 't'
* `s` is the 4-bit status nibble  0..15 or 0x00 to 0xff.  The type of message the rule applies to
* `x` is in [0,1] the byte to affect.
* `n` A MIDI to translate, an integer in 0..127 or 0x00..0x7f
* `m` A MIDI to output, if `n` received, in 0..127 or 0x00..0x7f
2. Channel Change.  Set the channel the MIDI is output on.  Lines of the form "c [+-]N"
* `c` the character 'c'
* `[+-]` Either character '+', '-' or nothing
* `N` A number in [0,16]
  * If there is a '+' or '-' `N` is a delta and is added (or subtracted) from the channel.  If the channel goes below 1 or greater than 16 there is an error.
  * If there is no '+' or '-' then `N` must be in  [1, 16] and is the channel to set on utput


All other lines are ignored

## Consumer: Command MIDI

Consumers read on `stdin` and affect the world.  `command_midi` runs  commands in response to noteon messages. A future plan is to kill the commands in response to noteoff messages

`command_midi` takes one argument: A configuration file name

### Configuration File

The configuration file consists of two sorts of lines:

1. lines of the form: "x n s"
  * `x` the character 'x'
  * `n` A MIDI note to translate, an integer in 0..127
  * `s` A command to run if the noteon message containing `n` is received.
    * `s` must be a executable command
    * No arguments are provided for
2. one or more lines of the form: "c n"
  * `c` the character 'c'
  * `n` the channel to monitor
    * If defined more than once the last definition is used
	* If missing defaults to 0
