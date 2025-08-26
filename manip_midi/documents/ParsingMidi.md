// Copyright (c) 2025 Worik Turei Stanton
// License: GPL-3.0

### **MIDI Status Types with Parsing Details**

#### **Channel Voice Messages**
- **Note On** (`0x9n`):
  - **Format**: `[0x9n] [Note] [Velocity]`
  - **Next Status**: After 2 data bytes.
  - **Running Status**: If the next message is also Note On, the status byte can be omitted.

- **Note Off** (`0x8n`):
  - **Format**: `[0x8n] [Note] [Velocity]`
  - **Next Status**: After 2 data bytes.
  - **Running Status**: If the next message is also Note Off, the status byte can be omitted.

- **Polyphonic Aftertouch** (`0xAn`):
  - **Format**: `[0xAn] [Note] [Pressure]`
  - **Next Status**: After 2 data bytes.

- **Control Change (CC)** (`0xBn`):
  - **Format**: `[0xBn] [Controller] [Value]`
  - **Next Status**: After 2 data bytes.

- F  1111
- E  1110
- D  1101
- C  1100
- B  1011
- A  1010
- 9  1001
- 8  1000
- 7  0111
- 6  0110
- 5  0101
- 4  0100
- 3  0011
- 2  0010

- **Program Change** (`0xCn`):
  - **Format**: `[0xCn] [Program]`
  - **Next Status**: After 1 data byte.

- **Channel Aftertouch** (`0xDn`):
  - **Format**: `[0xDn] [Pressure]`
  - **Next Status**: After 1 data byte.

- **Pitch Bend** (`0xEn`):
  - **Format**: `[0xEn] [LSB] [MSB]`
  - **Next Status**: After 2 data bytes.

---

#### **System Common Messages**
- **System Exclusive (SysEx)** (`0xF0`):
  - **Format**: `[0xF0] [Data...] [0xF7]`
  - **Next Status**: After `0xF7` (end of SysEx).
  - **Note**: Variable length; parse until `0xF7`.

- **Time Code Quarter Frame** (`0xF1`):
  - **Format**: `[0xF1] [Data]`
  - **Next Status**: After 1 data byte.

- **Song Position Pointer** (`0xF2`):
  - **Format**: `[0xF2] [LSB] [MSB]`
  - **Next Status**: After 2 data bytes.

- **Song Select** (`0xF3`):
  - **Format**: `[0xF3] [Song]`
  - **Next Status**: After 1 data byte.

- **Tune Request** (`0xF6`):
  - **Format**: `[0xF6]`
  - **Next Status**: Immediately after.

- **End of SysEx** (`0xF7`):
  - **Format**: `[0xF7]`
  - **Next Status**: Immediately after.

---

#### **System Real-Time Messages**
- **Timing Clock** (`0xF8`):
  - **Format**: `[0xF8]`
  - **Next Status**: Immediately after.

- **Start** (`0xFA`):
  - **Format**: `[0xFA]`
  - **Next Status**: Immediately after.

- **Continue** (`0xFB`):
  - **Format**: `[0xFB]`
  - **Next Status**: Immediately after.

- **Stop** (`0xFC`):
  - **Format**: `[0xFC]`
  - **Next Status**: Immediately after.

- **Active Sensing** (`0xFE`):
  - **Format**: `[0xFE]`
  - **Next Status**: Immediately after.

- **Reset** (`0xFF`):
  - **Format**: `[0xFF]`
  - **Next Status**: Immediately after.

---

### **Key Parsing Rules**
1. **Status Byte Detection**:
   - A byte with the most significant bit (MSB) set (`0x80`–`0xFF`) is a status byte.
   - If the MSB is not set (`0x00`–`0x7F`), it’s a data byte.

2. **Running Status**:
   - If no status byte is present, reuse the previous status byte.
   - Example: `0x90 0x3C 0x40 0x3D 0x40` (second Note On reuses `0x90`).

3. **Message Length**:
   - Most messages have a fixed length:
     - **Note On/Off**: 2 data bytes.
     - **Control Change**: 2 data bytes.
     - **Program Change**: 1 data byte.
     - **Pitch Bend**: 2 data bytes.
   - SysEx messages are variable-length and end with `0xF7`.

4. **System Real-Time Messages**:
   - These can appear **anywhere**, even between other messages.
   - They do not affect running status.

---

### **Example MIDI Parser Logic**

```rust
fn parse_midi(data: &[u8], transpose_offset: i8) {
    let mut i = 0;
    let mut status: Option<u8> = None;

    while i < data.len() {
        let byte = data[i];
        if byte & 0x80 != 0 { // Status byte (MSB set)
            status = Some(byte);
            i += 1;
        } else { // Data byte (running status)
            // Use the previous status byte
        }

        if let Some(status_byte) = status {
            match status_byte & 0xF0 {
                0x90 => { // Note On
                    if i + 1 < data.len() {
                        let note = data[i];
                        let velocity = data[i + 1];
                        i += 2;
                        let transposed_note = (note as i8).wrapping_add(transpose_offset) as u8;
                        println!("Note On: Channel {}, Note: {}, Velocity: {}", status_byte & 0x0F, transposed_note, velocity);
                    }
                }
                0x80 => { // Note Off
                    if i + 1 < data.len() {
                        let note = data[i];
                        let velocity = data[i + 1];
                        i += 2;
                        let transposed_note = (note as i8).wrapping_add(transpose_offset) as u8;
                        println!("Note Off: Channel {}, Note: {}, Velocity: {}", status_byte & 0x0F, transposed_note, velocity);
                    }
                }
                _ => { // Handle other message types
                    i += match status_byte & 0xF0 {
                        0xA0 | 0xB0 | 0xE0 => 2, // Poly Aftertouch, Control Change, Pitch Bend
                        0xC0 | 0xD0 => 1,        // Program Change, Channel Aftertouch
                        0xF0 => { // SysEx or System Common
                            if status_byte == 0xF0 {
                                // SysEx: Parse until 0xF7
                                while i < data.len() && data[i] != 0xF7 {
                                    i += 1;
                                }
                                if i < data.len() {
                                    i += 1; // Skip 0xF7
                                }
                                0 // No additional data bytes to skip
                            } else {
                                match status_byte {
                                    0xF1 | 0xF3 => 1, // Time Code Quarter Frame, Song Select
                                    0xF2 => 2,        // Song Position Pointer
                                    _ => 0,           // Other System Common/Real-Time
                                }
                            }
                        }
                        _ => 0, // Unknown or unsupported message
                    };
                }
            }
        } else {
            // No status byte, skip this byte
            i += 1;
        }
    }
}

fn main() {
    let midi_data = vec![
        0x90, 0x3C, 0x40, // Note On: Channel 1, Note 60, Velocity 64
        0x3D, 0x40,       // Note On: Channel 1, Note 61, Velocity 64 (running status)
        0x80, 0x3C, 0x40, // Note Off: Channel 1, Note 60, Velocity 64
    ];
    let transpose_offset = 2; // Transpose notes up by 2 semitones
    parse_midi(&midi_data, transpose_offset);
}
```

---

