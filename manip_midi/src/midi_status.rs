// Copyright (c) 2025 Worik Turei Stanton
// License: GPL-3.0

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum MidiStatus {
    // Channel specific messages
    ChannelAftertouch(u8),    // 0xD0..=0xDF
    ControlChange(u8),        // 0xB0..=0xBF
    NoteOff(u8),              // 0x80..=0x8F
    NoteOn(u8),               // 0x90..=0x9F
    PitchBend(u8),            // 0xE0..=0xEF
    PolyphonicAftertouch(u8), // 0xA0..=0xAF
    ProgramChange(u8),        // 0xC0..=0xCF
    // Channel independant messages
    ActiveSensing,        // 0xFE
    Continue,             // 0xFB
    EndOfExclusive,       // 0xF7
    Reset,                // 0xFF
    SongPositionPointer,  // 0xF2
    SongSelect,           // 0xF3
    Start,                // 0xFA
    Stop,                 // 0xFC
    SystemExclusive,      // 0xF0
    TimeCodeQuarterFrame, // 0xF1
    TimingClock,          // 0xF8
    TuneRequest,          // 0xF6
    Undefined1,           // 0xF4
    Undefined2,           // 0xF5
    Undefined3,           // 0xF9
    Undefined4,           // 0xFD
}
#[allow(dead_code)]
impl MidiStatus {
    pub fn from_byte(byte: u8) -> Option<Self> {
        match byte {
            0x80..=0x8F => Some(MidiStatus::NoteOff(byte & 0x0F)),
            0x90..=0x9F => Some(MidiStatus::NoteOn(byte & 0x0F)),
            0xA0..=0xAF => Some(MidiStatus::PolyphonicAftertouch(byte & 0x0F)),
            0xB0..=0xBF => Some(MidiStatus::ControlChange(byte & 0x0F)),
            0xC0..=0xCF => Some(MidiStatus::ProgramChange(byte & 0x0F)),
            0xD0..=0xDF => Some(MidiStatus::ChannelAftertouch(byte & 0x0F)),
            0xE0..=0xEF => Some(MidiStatus::PitchBend(byte & 0x0F)),
            0xF0 => Some(MidiStatus::SystemExclusive),
            0xF1 => Some(MidiStatus::TimeCodeQuarterFrame),
            0xF2 => Some(MidiStatus::SongPositionPointer),
            0xF3 => Some(MidiStatus::SongSelect),
            0xF4 => Some(MidiStatus::Undefined1),
            0xF5 => Some(MidiStatus::Undefined2),
            0xF6 => Some(MidiStatus::TuneRequest),
            0xF7 => Some(MidiStatus::EndOfExclusive),
            0xF8 => Some(MidiStatus::TimingClock),
            0xF9 => Some(MidiStatus::Undefined3),
            0xFA => Some(MidiStatus::Start),
            0xFB => Some(MidiStatus::Continue),
            0xFC => Some(MidiStatus::Stop),
            0xFD => Some(MidiStatus::Undefined4),
            0xFE => Some(MidiStatus::ActiveSensing),
            0xFF => Some(MidiStatus::Reset),
            _ => None,
        }
    }
    pub fn to_byte(self) -> u8 {
        match self {
            // Channel specific messages
            MidiStatus::ChannelAftertouch(channel) => 0xD0 | (channel & 0x0F),
            MidiStatus::ControlChange(channel) => 0xB0 | (channel & 0x0F),
            MidiStatus::NoteOff(channel) => 0x80 | (channel & 0x0F),
            MidiStatus::NoteOn(channel) => 0x90 | (channel & 0x0F),
            MidiStatus::PitchBend(channel) => 0xE0 | (channel & 0x0F),
            MidiStatus::PolyphonicAftertouch(channel) => 0xA0 | (channel & 0x0F),
            MidiStatus::ProgramChange(channel) => 0xC0 | (channel & 0x0F),
            // Channel independent messages
            MidiStatus::ActiveSensing => 0xFE,
            MidiStatus::Continue => 0xFB,
            MidiStatus::EndOfExclusive => 0xF7,
            MidiStatus::Reset => 0xFF,
            MidiStatus::SongPositionPointer => 0xF2,
            MidiStatus::SongSelect => 0xF3,
            MidiStatus::Start => 0xFA,
            MidiStatus::Stop => 0xFC,
            MidiStatus::SystemExclusive => 0xF0,
            MidiStatus::TimeCodeQuarterFrame => 0xF1,
            MidiStatus::TimingClock => 0xF8,
            MidiStatus::TuneRequest => 0xF6,
            MidiStatus::Undefined1 => 0xF4,
            MidiStatus::Undefined2 => 0xF5,
            MidiStatus::Undefined3 => 0xF9,
            MidiStatus::Undefined4 => 0xFD,
        }
    }
    pub fn arg_count(&self) -> u8 {
        match self {
            MidiStatus::ProgramChange(_)
            | MidiStatus::ChannelAftertouch(_)
            | MidiStatus::TimeCodeQuarterFrame
            | MidiStatus::SongSelect => {
                // Only one data byte.
                1
            }
            MidiStatus::NoteOff(_)
            | MidiStatus::NoteOn(_)
            | MidiStatus::PolyphonicAftertouch(_)
            | MidiStatus::ControlChange(_)
            | MidiStatus::PitchBend(_)
            | MidiStatus::SongPositionPointer => {
                // Two bytes.
                2
            }
            MidiStatus::SystemExclusive
            | MidiStatus::ActiveSensing
            | MidiStatus::Reset
            | MidiStatus::Stop
            | MidiStatus::TuneRequest
            | MidiStatus::EndOfExclusive
            | MidiStatus::TimingClock
            | MidiStatus::Start
            | MidiStatus::Continue
            | MidiStatus::Undefined1
            | MidiStatus::Undefined2
            | MidiStatus::Undefined3
            | MidiStatus::Undefined4 => 0,
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    // Tests for from_byte
    #[test]
    fn test_from_byte_channel_messages() {
        // Test all channel message ranges
        for status_byte in 0x80..=0xEF {
            let channel = status_byte & 0x0F;
            let status = MidiStatus::from_byte(status_byte).unwrap();
            match status {
                MidiStatus::NoteOff(c) if status_byte <= 0x8F => assert_eq!(c, channel),
                MidiStatus::NoteOn(c) if status_byte <= 0x9F => assert_eq!(c, channel),
                MidiStatus::PolyphonicAftertouch(c) if status_byte <= 0xAF => {
                    assert_eq!(c, channel)
                }
                MidiStatus::ControlChange(c) if status_byte <= 0xBF => assert_eq!(c, channel),
                MidiStatus::ProgramChange(c) if status_byte <= 0xCF => assert_eq!(c, channel),
                MidiStatus::ChannelAftertouch(c) if status_byte <= 0xDF => assert_eq!(c, channel),
                MidiStatus::PitchBend(c) if status_byte <= 0xEF => assert_eq!(c, channel),
                _ => panic!("Unexpected status for byte 0x{:X}", status_byte),
            }
        }
    }
    #[test]
    fn test_from_byte_system_messages() {
        assert_eq!(
            MidiStatus::from_byte(0xF0),
            Some(MidiStatus::SystemExclusive)
        );
        assert_eq!(
            MidiStatus::from_byte(0xF1),
            Some(MidiStatus::TimeCodeQuarterFrame)
        );
        assert_eq!(
            MidiStatus::from_byte(0xF2),
            Some(MidiStatus::SongPositionPointer)
        );
        assert_eq!(MidiStatus::from_byte(0xF3), Some(MidiStatus::SongSelect));
        assert_eq!(MidiStatus::from_byte(0xF4), Some(MidiStatus::Undefined1));
        assert_eq!(MidiStatus::from_byte(0xF5), Some(MidiStatus::Undefined2));
        assert_eq!(MidiStatus::from_byte(0xF6), Some(MidiStatus::TuneRequest));
        assert_eq!(
            MidiStatus::from_byte(0xF7),
            Some(MidiStatus::EndOfExclusive)
        );
        assert_eq!(MidiStatus::from_byte(0xF8), Some(MidiStatus::TimingClock));
        assert_eq!(MidiStatus::from_byte(0xF9), Some(MidiStatus::Undefined3));
        assert_eq!(MidiStatus::from_byte(0xFA), Some(MidiStatus::Start));
        assert_eq!(MidiStatus::from_byte(0xFB), Some(MidiStatus::Continue));
        assert_eq!(MidiStatus::from_byte(0xFC), Some(MidiStatus::Stop));
        assert_eq!(MidiStatus::from_byte(0xFD), Some(MidiStatus::Undefined4));
        assert_eq!(MidiStatus::from_byte(0xFE), Some(MidiStatus::ActiveSensing));
        assert_eq!(MidiStatus::from_byte(0xFF), Some(MidiStatus::Reset));
    }
    #[test]
    fn test_from_byte_invalid() {
        // Test some invalid bytes
        assert_eq!(MidiStatus::from_byte(0x00), None);
        assert_eq!(MidiStatus::from_byte(0x70), None);
        assert_eq!(MidiStatus::from_byte(0x7F), None);
    }
    // Tests for to_byte
    #[test]
    fn test_to_byte_channel_messages() {
        // Test all channel message variants
        for channel in 0..=0x0F {
            assert_eq!(MidiStatus::NoteOff(channel).to_byte(), 0x80 | channel);
            assert_eq!(MidiStatus::NoteOn(channel).to_byte(), 0x90 | channel);
            assert_eq!(
                MidiStatus::PolyphonicAftertouch(channel).to_byte(),
                0xA0 | channel
            );
            assert_eq!(MidiStatus::ControlChange(channel).to_byte(), 0xB0 | channel);
            assert_eq!(MidiStatus::ProgramChange(channel).to_byte(), 0xC0 | channel);
            assert_eq!(
                MidiStatus::ChannelAftertouch(channel).to_byte(),
                0xD0 | channel
            );
            assert_eq!(MidiStatus::PitchBend(channel).to_byte(), 0xE0 | channel);
        }
    }
    #[test]
    fn test_to_byte_system_messages() {
        assert_eq!(MidiStatus::SystemExclusive.to_byte(), 0xF0);
        assert_eq!(MidiStatus::TimeCodeQuarterFrame.to_byte(), 0xF1);
        assert_eq!(MidiStatus::SongPositionPointer.to_byte(), 0xF2);
        assert_eq!(MidiStatus::SongSelect.to_byte(), 0xF3);
        assert_eq!(MidiStatus::Undefined1.to_byte(), 0xF4);
        assert_eq!(MidiStatus::Undefined2.to_byte(), 0xF5);
        assert_eq!(MidiStatus::TuneRequest.to_byte(), 0xF6);
        assert_eq!(MidiStatus::EndOfExclusive.to_byte(), 0xF7);
        assert_eq!(MidiStatus::TimingClock.to_byte(), 0xF8);
        assert_eq!(MidiStatus::Undefined3.to_byte(), 0xF9);
        assert_eq!(MidiStatus::Start.to_byte(), 0xFA);
        assert_eq!(MidiStatus::Continue.to_byte(), 0xFB);
        assert_eq!(MidiStatus::Stop.to_byte(), 0xFC);
        assert_eq!(MidiStatus::Undefined4.to_byte(), 0xFD);
        assert_eq!(MidiStatus::ActiveSensing.to_byte(), 0xFE);
        assert_eq!(MidiStatus::Reset.to_byte(), 0xFF);
    }
    // Test that to_byte and from_byte are inverses
    #[test]
    fn test_round_trip() {
        // Test channel messages
        for channel in 0..=0x0F {
            let status_list = [
                MidiStatus::NoteOff(channel),
                MidiStatus::NoteOn(channel),
                MidiStatus::PolyphonicAftertouch(channel),
                MidiStatus::ControlChange(channel),
                MidiStatus::ProgramChange(channel),
                MidiStatus::ChannelAftertouch(channel),
                MidiStatus::PitchBend(channel),
            ];
            for status in status_list {
                let byte = status.to_byte();
                let round_tripped = MidiStatus::from_byte(byte).unwrap();
                assert_eq!(status, round_tripped);
            }
        }
        // Test system messages
        let system_status_list = [
            MidiStatus::SystemExclusive,
            MidiStatus::TimeCodeQuarterFrame,
            MidiStatus::SongPositionPointer,
            MidiStatus::SongSelect,
            MidiStatus::Undefined1,
            MidiStatus::Undefined2,
            MidiStatus::TuneRequest,
            MidiStatus::EndOfExclusive,
            MidiStatus::TimingClock,
            MidiStatus::Undefined3,
            MidiStatus::Start,
            MidiStatus::Continue,
            MidiStatus::Stop,
            MidiStatus::Undefined4,
            MidiStatus::ActiveSensing,
            MidiStatus::Reset,
        ];
        for status in system_status_list {
            let byte = status.to_byte();
            let round_tripped = MidiStatus::from_byte(byte).unwrap();
            assert_eq!(status, round_tripped);
        }
    }
}
