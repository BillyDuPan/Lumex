//! Zero-allocation MIDI 1.0 packet parser.
//!
//! Parses raw byte slices into strongly typed `MidiMessage` representations
//! without heap allocations on the audio/MIDI callback thread.

use serde::{Deserialize, Serialize};

/// Strongly typed MIDI 1.0 message.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MidiMessage {
    NoteOn {
        channel: u8,
        note: u8,
        velocity: u8,
    },
    NoteOff {
        channel: u8,
        note: u8,
        velocity: u8,
    },
    ControlChange {
        channel: u8,
        controller: u8,
        value: u8,
    },
    PitchBend {
        channel: u8,
        value: u16, // 14-bit: 0..16383, center = 8192
    },
    ProgramChange {
        channel: u8,
        program: u8,
    },
    ChannelAftertouch {
        channel: u8,
        pressure: u8,
    },
    Clock,
    Start,
    Continue,
    Stop,
}

impl MidiMessage {
    /// Parse a raw MIDI byte slice into a typed message.
    ///
    /// Returns `None` for empty slices, malformed packets, or unsupported system exclusive bytes.
    #[inline]
    pub fn parse(bytes: &[u8]) -> Option<Self> {
        if bytes.is_empty() {
            return None;
        }

        let status = bytes[0];

        // 1. Realtime single-byte system messages (0xF8..=0xFF)
        match status {
            0xF8 => return Some(Self::Clock),
            0xFA => return Some(Self::Start),
            0xFB => return Some(Self::Continue),
            0xFC => return Some(Self::Stop),
            _ => {}
        }

        if bytes.len() < 2 {
            return None;
        }

        let message_type = status & 0xF0;
        let channel = status & 0x0F;

        match message_type {
            // Note Off (0x80)
            0x80 => {
                let note = bytes[1];
                let velocity = if bytes.len() > 2 { bytes[2] } else { 0 };
                Some(Self::NoteOff {
                    channel,
                    note,
                    velocity,
                })
            }

            // Note On (0x90) - Note: Velocity 0 is canonically Note Off
            0x90 => {
                let note = bytes[1];
                let velocity = if bytes.len() > 2 { bytes[2] } else { 0 };
                if velocity == 0 {
                    Some(Self::NoteOff {
                        channel,
                        note,
                        velocity: 0,
                    })
                } else {
                    Some(Self::NoteOn {
                        channel,
                        note,
                        velocity,
                    })
                }
            }

            // Control Change (0xB0)
            0xB0 if bytes.len() >= 3 => {
                let controller = bytes[1];
                let value = bytes[2];
                Some(Self::ControlChange {
                    channel,
                    controller,
                    value,
                })
            }

            // Program Change (0xC0)
            0xC0 => {
                let program = bytes[1];
                Some(Self::ProgramChange { channel, program })
            }

            // Channel Aftertouch (0xD0)
            0xD0 => {
                let pressure = bytes[1];
                Some(Self::ChannelAftertouch { channel, pressure })
            }

            // Pitch Bend (0xE0) - 14-bit value: (msb << 7) | lsb
            0xE0 if bytes.len() >= 3 => {
                let lsb = (bytes[1] & 0x7F) as u16;
                let msb = (bytes[2] & 0x7F) as u16;
                let value = (msb << 7) | lsb;
                Some(Self::PitchBend { channel, value })
            }

            _ => None,
        }
    }

    /// MIDI channel (0..15), if this message is channel-specific.
    pub fn channel(&self) -> Option<u8> {
        match self {
            Self::NoteOn { channel, .. }
            | Self::NoteOff { channel, .. }
            | Self::ControlChange { channel, .. }
            | Self::PitchBend { channel, .. }
            | Self::ProgramChange { channel, .. }
            | Self::ChannelAftertouch { channel, .. } => Some(*channel),
            Self::Clock | Self::Start | Self::Continue | Self::Stop => None,
        }
    }

    /// Returns true if this is a NoteOn message.
    pub fn is_note_on(&self) -> bool {
        matches!(self, Self::NoteOn { .. })
    }

    /// Returns true if this is a NoteOff message.
    pub fn is_note_off(&self) -> bool {
        matches!(self, Self::NoteOff { .. })
    }

    /// Returns true if this is a ControlChange message.
    pub fn is_control_change(&self) -> bool {
        matches!(self, Self::ControlChange { .. })
    }

    /// Returns true if this is a realtime transport/clock message.
    pub fn is_realtime(&self) -> bool {
        matches!(
            self,
            Self::Clock | Self::Start | Self::Continue | Self::Stop
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_note_on_and_off() {
        let note_on_bytes = [0x90, 60, 100];
        assert_eq!(
            MidiMessage::parse(&note_on_bytes),
            Some(MidiMessage::NoteOn {
                channel: 0,
                note: 60,
                velocity: 100
            })
        );

        // Velocity 0 should be parsed as NoteOff
        let note_on_zero_vel = [0x91, 64, 0];
        assert_eq!(
            MidiMessage::parse(&note_on_zero_vel),
            Some(MidiMessage::NoteOff {
                channel: 1,
                note: 64,
                velocity: 0
            })
        );

        let note_off_bytes = [0x82, 67, 64];
        assert_eq!(
            MidiMessage::parse(&note_off_bytes),
            Some(MidiMessage::NoteOff {
                channel: 2,
                note: 67,
                velocity: 64
            })
        );
    }

    #[test]
    fn test_parse_cc_and_pitch_bend() {
        let cc_bytes = [0xB0, 7, 127];
        assert_eq!(
            MidiMessage::parse(&cc_bytes),
            Some(MidiMessage::ControlChange {
                channel: 0,
                controller: 7,
                value: 127
            })
        );

        // Pitch bend center: lsb = 0x00, msb = 0x40 (8192)
        let pb_center = [0xE0, 0x00, 0x40];
        assert_eq!(
            MidiMessage::parse(&pb_center),
            Some(MidiMessage::PitchBend {
                channel: 0,
                value: 8192
            })
        );
    }

    #[test]
    fn test_parse_realtime() {
        assert_eq!(MidiMessage::parse(&[0xF8]), Some(MidiMessage::Clock));
        assert_eq!(MidiMessage::parse(&[0xFA]), Some(MidiMessage::Start));
        assert_eq!(MidiMessage::parse(&[0xFB]), Some(MidiMessage::Continue));
        assert_eq!(MidiMessage::parse(&[0xFC]), Some(MidiMessage::Stop));
    }
}
