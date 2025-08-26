// Copyright (c) 2025 Worik Turei Stanton
// License: GPL-3.0

### MIDI Message Structure
MIDI messages are composed of **status bytes** and **data bytes**:
1. **Status Byte**: The first byte of a MIDI message, which indicates the type of message and the MIDI channel.
   - The upper nibble (4 bits) specifies the message type (e.g., Note On, Note Off, Control Change).
   - The lower nibble (4 bits) specifies the MIDI channel (0–15, often represented as 1–16 in software).
2. **Data Bytes**: One or two bytes that provide additional information, such as the note number, velocity, or control value.

---

### Common MIDI Messages and Their Byte Sequences

Here are some examples of MIDI messages and what they mean:

#### 1. **Note On**
- **Purpose**: Signals that a note has been pressed.
- **Format**: `[Status Byte] [Note Number] [Velocity]`
- **Example**: `0x90 0x3C 0x40`
  - `0x90`: Status byte for "Note On" on Channel 1.
    - `9` = Note On message.
    - `0` = Channel 1.
  - `0x3C`: Note number (60 = Middle C).
  - `0x40`: Velocity (64 = medium intensity).

#### 2. **Note Off**
- **Purpose**: Signals that a note has been released.
- **Format**: `[Status Byte] [Note Number] [Velocity]`
- **Example**: `0x80 0x3C 0x40`
  - `0x80`: Status byte for "Note Off" on Channel 1.
    - `8` = Note Off message.
    - `0` = Channel 1.
  - `0x3C`: Note number (60 = Middle C).
  - `0x40`: Velocity (64 = release intensity, often ignored).

#### 3. **Control Change (CC)**
- **Purpose**: Adjusts a parameter like volume, modulation, or pan.
- **Format**: `[Status Byte] [Controller Number] [Value]`
- **Example**: `0xB0 0x07 0x40`
  - `0xB0`: Status byte for "Control Change" on Channel 1.
    - `B` = Control Change message.
    - `0` = Channel 1.
  - `0x07`: Controller number (7 = Channel Volume).
  - `0x40`: Value (64 = halfway).

#### 4. **Program Change**
- **Purpose**: Changes the instrument or patch on a MIDI device.
- **Format**: `[Status Byte] [Program Number]`
- **Example**: `0xC0 0x20`
  - `0xC0`: Status byte for "Program Change" on Channel 1.
    - `C` = Program Change message.
    - `0` = Channel 1.
  - `0x20`: Program number (32 = specific patch/instrument).

#### 5. **Pitch Bend**
- **Purpose**: Adjusts the pitch of a note up or down for expressive playing.
- **Format**: `[Status Byte] [LSB] [MSB]`
- **Example**: `0xE0 0x00 0x40`
  - `0xE0`: Status byte for "Pitch Bend" on Channel 1.
    - `E` = Pitch Bend message.
    - `0` = Channel 1.
  - `0x00`: Least Significant Byte (LSB) of the bend value.
  - `0x40`: Most Significant Byte (MSB) of the bend value.
  - Combined, this represents a pitch bend value of `8192` (center/no bend).

#### 6. **Aftertouch (Channel Pressure)**
- **Purpose**: Sends pressure information after a note is pressed.
- **Format**: `[Status Byte] [Pressure Value]`
- **Example**: `0xD0 0x40`
  - `0xD0`: Status byte for "Aftertouch" on Channel 1.
    - `D` = Aftertouch message.
    - `0` = Channel 1.
  - `0x40`: Pressure value (64 = medium pressure).

---

### Real-World Example: MIDI Data Stream
If you were monitoring a MIDI interface, you might see a sequence of bytes like this:

```
0x90 0x3C 0x40  // Note On: Channel 1, Middle C, Velocity 64
0x80 0x3C 0x40  // Note Off: Channel 1, Middle C, Velocity 64
0xB0 0x07 0x40  // Control Change: Channel 1, Volume, Value 64
0xC0 0x20       // Program Change: Channel 1, Program 32
0xE0 0x00 0x40  // Pitch Bend: Channel 1, Center (no bend)
```

---

### Running Status
MIDI uses a concept called **running status** to save bandwidth. If multiple messages of the same type are sent in sequence, the status byte can be omitted after the first message. For example:

```
0x90 0x3C 0x40  // Note On: Channel 1, Middle C, Velocity 64
0x3D 0x40       // Note On: Channel 1, C#4, Velocity 64 (status byte omitted)
```

Here, the second message assumes the same status byte (`0x90`) as the previous message.

---

### MIDI Over USB
When MIDI is transmitted over USB, the raw MIDI messages are encapsulated in USB-MIDI packets. Each packet contains a header and the MIDI data. For example:

```
0x09 0x90 0x3C 0x40  // USB-MIDI packet for Note On
```

The `0x09` is a USB-MIDI header indicating the type of packet.

---

### Tools for Monitoring MIDI
To see MIDI messages in real-time, you can use software tools like:
- **MIDI Monitor** (macOS)
- **MIDI-OX** (Windows)
- **aseqdump** (Linux)

These tools display the raw MIDI messages as they arrive, making it easier to debug or understand what your MIDI device is sending.

---

### Summary
When you monitor a MIDI interface, you’ll see a stream of bytes representing MIDI messages. Each message has a specific structure, and understanding the status byte and data bytes allows you to decode what’s happening (e.g., which notes are being played, which controls are being adjusted, etc.). This raw data is what software like DAWs (Digital Audio Workstations) or MIDI sequencers interpret to produce sound or control devices.

---

### Comprehensive List of MIDI Messages

#### 1. **Channel Voice Messages**
These messages are specific to a MIDI channel and are used to control notes, controllers, and other parameters.

| Message Type              | Status Byte | Data Bytes        | Description                                                     |
|---------------------------|-------------|-------------------|-----------------------------------------------------------------|
| **Note On**               | `0x9n`      | Note, Velocity    | Indicates a note is pressed. Velocity = 0 can act as Note Off.  |
| **Note Off**              | `0x8n`      | Note, Velocity    | Indicates a note is released.                                   |
| **Polyphonic Aftertouch** | `0xAn`      | Note, Pressure    | Pressure applied to a specific note after it’s pressed.         |
| **Control Change (CC)**   | `0xBn`      | Controller, Value | Adjusts parameters like volume, modulation, pan, etc.           |
| **Program Change**        | `0xCn`      | Program Number    | Changes the instrument or patch on a MIDI device.               |
| **Channel Aftertouch**    | `0xDn`      | Pressure          | Pressure applied to the entire channel after a note is pressed. |
| **Pitch Bend**            | `0xEn`      | LSB, MSB          | Adjusts the pitch of all notes on the channel up or down.       |

- `n` in the status byte represents the MIDI channel (0–15, often displayed as 1–16).

---

#### 2. **System Common Messages**
These messages are not specific to a channel and are used for synchronization and setup.

| Message Type                 | Status Byte | Data Bytes  | Description                                               |
|------------------------------|-------------|-------------|-----------------------------------------------------------|
| **System Exclusive (SysEx)** | `0xF0`      | Variable    | Manufacturer-specific data. Ends with `0xF7`.             |
| **Time Code Quarter Frame**  | `0xF1`      | Data        | Part of MIDI Time Code (MTC) for synchronization.         |
| **Song Position Pointer**    | `0xF2`      | LSB, MSB    | Indicates the current position in a song (in MIDI beats). |
| **Song Select**              | `0xF3`      | Song Number | Selects a song or sequence.                               |
| **Tune Request**             | `0xF6`      | None        | Requests analog synthesizers to retune oscillators.       |
| **End of SysEx**             | `0xF7`      | None        | Marks the end of a SysEx message.                         |

---

#### 3. **System Real-Time Messages**
These messages are used for synchronization and timing. They are single-byte messages and can occur at any time, even between other MIDI messages.

| Message Type       | Status Byte | Description                                                 |
|--------------------|-------------|-------------------------------------------------------------|
| **Timing Clock**   | `0xF8`      | Sent 24 times per quarter note for synchronization.         |
| **Start**          | `0xFA`      | Starts playback of a sequence.                              |
| **Continue**       | `0xFB`      | Resumes playback of a paused sequence.                      |
| **Stop**           | `0xFC`      | Stops playback of a sequence.                               |
| **Active Sensing** | `0xFE`      | Optional message to indicate the device is still connected. |
| **Reset**          | `0xFF`      | Resets all devices to their default state.                  |

---

#### 4. **System Exclusive (SysEx) Messages**
SysEx messages are used for manufacturer-specific data and can vary widely in format and purpose. They begin with `0xF0` and end with `0xF7`.

- **Manufacturer ID**: The first byte(s) after `0xF0` identify the manufacturer.
  - Example: `0xF0 0x41 0x10 0x42 0x12 0x34 0xF7` (Roland SysEx message).
- **Data**: The content of the message depends on the manufacturer and device.
- **Uses**: Firmware updates, patch editing, device configuration, etc.

---

#### 5. **MIDI 2.0 Extensions**
MIDI 2.0 introduces new message types and capabilities, such as:
- **Per-Note Pitch Bend**: Adjusts the pitch of individual notes.
- **Per-Note Controllers**: Adds controllers specific to individual notes.
- **Increased Resolution**: Higher precision for velocity, pitch bend, and control values.

---

### Less Common MIDI Messages
Some MIDI messages are less frequently used but are part of the standard:

| Message Type              | Status Byte | Description                                                  |
|---------------------------|-------------|--------------------------------------------------------------|
| **Channel Mode Messages** | `0xBn`      | Subset of Control Change messages for channel-wide settings. |
- **All Notes Off** (`0x7B`): Turns off all notes on a channel.
- **Reset All Controllers** (`0x79`): Resets all controllers to default.
- **Local Control** (`0x7A`): Enables/disables local control of a device.
- **Omni Mode** (`0x7D`, `0x7E`): Enables/disables omni mode (respond to all channels).
- **Mono Mode** (`0x7E`): Forces a device to play monophonically.
- **Poly Mode** (`0x7F`): Forces a device to play polyphonically.

---

### Summary
The list above is much more comprehensive and covers the vast majority of MIDI message types. However, MIDI is a flexible protocol, and some devices or manufacturers may implement custom or proprietary messages (often via SysEx). If you’re working with a specific device, its documentation will provide details about any unique messages it supports.

For most applications, the **Channel Voice Messages** and **System Real-Time Messages** are the most relevant. The other message types (e.g., SysEx, System Common) are used for advanced functionality like synchronization, configuration, and manufacturer-specific features.

---

## Status Messages and Their Meanings

### **Channel Voice Messages** (Specific to a MIDI channel)
- **Note On** (`0x9n`): Signals a note is pressed. Includes note number and velocity.
- **Note Off** (`0x8n`): Signals a note is released. Includes note number and velocity.
- **Polyphonic Aftertouch** (`0xAn`): Pressure applied to a specific note after it’s pressed.
- **Control Change (CC)** (`0xBn`): Adjusts parameters like volume, modulation, or pan.
- **Program Change** (`0xCn`): Changes the instrument or patch on a MIDI device.
- **Channel Aftertouch** (`0xDn`): Pressure applied to the entire channel after a note is pressed.
- **Pitch Bend** (`0xEn`): Adjusts the pitch of all notes on the channel up or down.

---

### **System Common Messages** (Not channel-specific)
- **System Exclusive (SysEx)** (`0xF0`): Manufacturer-specific data. Ends with `0xF7`.
- **Time Code Quarter Frame** (`0xF1`): Part of MIDI Time Code (MTC) for synchronization.
- **Song Position Pointer** (`0xF2`): Indicates the current position in a song (in MIDI beats).
- **Song Select** (`0xF3`): Selects a song or sequence.
- **Tune Request** (`0xF6`): Requests analog synthesizers to retune oscillators.
- **End of SysEx** (`0xF7`): Marks the end of a SysEx message.

---

### **System Real-Time Messages** (Timing and synchronization)
- **Timing Clock** (`0xF8`): Sent 24 times per quarter note for synchronization.
- **Start** (`0xFA`): Starts playback of a sequence.
- **Continue** (`0xFB`): Resumes playback of a paused sequence.
- **Stop** (`0xFC`): Stops playback of a sequence.
- **Active Sensing** (`0xFE`): Optional message to indicate the device is still connected.
- **Reset** (`0xFF`): Resets all devices to their default state.

---

### **Channel Mode Messages** (Subset of Control Change)
- **All Notes Off** (`0x7B`): Turns off all notes on a channel.
- **Reset All Controllers** (`0x79`): Resets all controllers to default.
- **Local Control** (`0x7A`): Enables/disables local control of a device.
- **Omni Mode** (`0x7D`, `0x7E`): Enables/disables omni mode (respond to all channels).
- **Mono Mode** (`0x7E`): Forces a device to play monophonically.
- **Poly Mode** (`0x7F`): Forces a device to play polyphonically.

---

This list covers the core MIDI status types and their purposes. Let me know if you need further clarification!
