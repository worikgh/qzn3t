# Patterns on a Touch Pad (like LPX Novation)

Group and light up LEDs, and output MIDI signals - all pads in a group/have same colour, output same MIDI note.


Provides two MIDI ports:
1. `PadCtlNote` for MIDI noteon/noteoff messages
  Uses `0x90` MIDI messages with velocity == `0` for noteoff
2. `PadCtlCtl` for MIDI control messages
  Uses `0xb0` messages.  The third of the triple is `0x7f` for "pressed", `0` for "released"

## Sections - Colour and Note

* Defined using sets of pads. Allows arbitrary, even discontinuous, sections
* All the pads in a section have the same properties (colours and MIDI note)
* No section can intersect with another, each pad is in at most one section
* Each pad can occur at most once in a section
* There can be, at most, one section with no defined pads. It is the default for pads not included

### Properties of a Section

* Main Colour: Each section has a main colour that is displayed when the pad is not pressed. 
* Active Colour: Each section has an "active" colour.  When any pad in the section is pressed (has issued an "on" but not an "off" MIDI signal) the section  is the active colour.
* MIDI Note - the note to output

Two sections can have the same colours and or notes, but hey are still independent of each other.

## Default Section

If a section is defined with no pads it is the default section and all pads that are not specified are in this section

## Input

The definition of the sections is in a file that is the first argument: `lpx_ctl <Section File>`
  
Line Orientated
---

Comments: Lines where the first non-space character is a '#'

* Each line defines a `Section`
* Each line is a comma separated record
* Each record consists of four fields
  1. A space delimited list of pads in the section
  2. The main colour in HEX RGB (#RRGGBB R, G, B in [0-9a-f])
  3. The active colour in HEX RGB (#RRGGBB R, G, B in [0-9a-f])
  4. The MIDI note for the section

Example:

This example has three sections: Two with 3-pads and one default
```plaintext
11 12 13, #ff0000, #00ff00, 60
21 22 23, #0000ff, #ffff00, 61
, #ff00ff, #ffffff, 62
```

---
TODO: Rewrite this using [`manip_midi`](../manip_midi/README.md)
---
