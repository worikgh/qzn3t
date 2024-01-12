# Patterns on LPX Novation

Group and light up LEDs on LPX Novation, and output MIDI signals - all pads in a group/have same colour, output same MIDI note.

## Sections - Colour and Note

* Defined using sets of pads. Allows arbitrary, even discontinuous, sections
* All the pads in a section have the same propertie (colours and MIDI note)
* No section can intersect with another, each pad is in at most one section
* There can be one section with no defined pads. It is the default for pads not included

### Properties of a Section

* Main Colour: Each section has a unique main colour
* Active Colour: Each section has an "active" colour.  When any pad in
  the section is pressed (has issued an "on" but not an "off" MIDI
  signal) the section  is the active colour.
* MIDI Note - the note to output

## Input

The definition of the oad patern is in a file that is the first argument: `lpx_ctl <Pattern File>`

It is a JSON file.

An array of JSON Objects.  Each object, is a `Section` has the
following properties:

* pad: Number (int).  11 - 88.  Top left of the section
* main_colour: [Number, Number, Number] ([usize;3]) RGB colour.  Each
  in range 0-127, each unique for every section
* active_colour: [Number, Number, Number] ([usize;3]) RGB colour.
  Each in range 0-127
* midi_note: The note to attach note-on and note-off MIDI events to.
  It is unique for every section
