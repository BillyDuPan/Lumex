//! Tests for zero-allocation MIDI packet parsing and modulation matrix evaluation.

use app_lib::midi::matrix::{CurveType, MappingMode, MidiMapping, MidiModulationMatrix};
use app_lib::midi::parser::MidiMessage;

#[test]
fn test_synthetic_packet_parsing() {
    // 1. Note On / Off
    assert_eq!(
        MidiMessage::parse(&[0x90, 60, 100]),
        Some(MidiMessage::NoteOn {
            channel: 0,
            note: 60,
            velocity: 100
        })
    );

    // Note On with velocity 0 must parse canonically as Note Off
    assert_eq!(
        MidiMessage::parse(&[0x90, 60, 0]),
        Some(MidiMessage::NoteOff {
            channel: 0,
            note: 60,
            velocity: 0
        })
    );

    assert_eq!(
        MidiMessage::parse(&[0x81, 72, 64]),
        Some(MidiMessage::NoteOff {
            channel: 1,
            note: 72,
            velocity: 64
        })
    );

    // 2. Control Change
    assert_eq!(
        MidiMessage::parse(&[0xB0, 16, 127]),
        Some(MidiMessage::ControlChange {
            channel: 0,
            controller: 16,
            value: 127
        })
    );

    // 3. Pitch Bend (14-bit)
    // Center: lsb=0, msb=64 -> 8192
    assert_eq!(
        MidiMessage::parse(&[0xE0, 0x00, 0x40]),
        Some(MidiMessage::PitchBend {
            channel: 0,
            value: 8192
        })
    );
    // Min: lsb=0, msb=0 -> 0
    assert_eq!(
        MidiMessage::parse(&[0xE0, 0x00, 0x00]),
        Some(MidiMessage::PitchBend {
            channel: 0,
            value: 0
        })
    );
    // Max: lsb=127, msb=127 -> 16383
    assert_eq!(
        MidiMessage::parse(&[0xE0, 0x7F, 0x7F]),
        Some(MidiMessage::PitchBend {
            channel: 0,
            value: 16383
        })
    );

    // 4. Realtime transport
    assert_eq!(MidiMessage::parse(&[0xF8]), Some(MidiMessage::Clock));
    assert_eq!(MidiMessage::parse(&[0xFA]), Some(MidiMessage::Start));
    assert_eq!(MidiMessage::parse(&[0xFB]), Some(MidiMessage::Continue));
    assert_eq!(MidiMessage::parse(&[0xFC]), Some(MidiMessage::Stop));

    // 5. Malformed packets
    assert_eq!(MidiMessage::parse(&[]), None);
    assert_eq!(MidiMessage::parse(&[0x90]), None); // truncated NoteOn
}

#[test]
fn test_modulation_matrix_curves() {
    let mut matrix = MidiModulationMatrix::new();

    // Map Linear CC 16 -> speed (0.0 to 10.0)
    matrix.add_mapping(MidiMapping {
        id: "speed_linear".to_string(),
        channel: Some(0),
        controller_or_note: 16,
        is_note: false,
        target_param: "speed".to_string(),
        mode: MappingMode::Absolute,
        min_val: 0.0,
        max_val: 10.0,
        curve: CurveType::Linear,
    });

    // Map Exponential CC 17 -> zoom_speed (0.0 to 10.0)
    matrix.add_mapping(MidiMapping {
        id: "zoom_exp".to_string(),
        channel: Some(0),
        controller_or_note: 17,
        is_note: false,
        target_param: "zoom_speed".to_string(),
        mode: MappingMode::Absolute,
        min_val: 0.0,
        max_val: 10.0,
        curve: CurveType::Exponential,
    });

    // Map Logarithmic CC 18 -> glow (0.0 to 10.0)
    matrix.add_mapping(MidiMapping {
        id: "glow_log".to_string(),
        channel: Some(0),
        controller_or_note: 18,
        is_note: false,
        target_param: "glow".to_string(),
        mode: MappingMode::Absolute,
        min_val: 0.0,
        max_val: 10.0,
        curve: CurveType::Logarithmic,
    });

    // Evaluate min (0)
    matrix.process_message(&MidiMessage::ControlChange {
        channel: 0,
        controller: 16,
        value: 0,
    });
    assert_eq!(matrix.get_parameter("speed"), Some(0.0));

    // Evaluate max (127)
    matrix.process_message(&MidiMessage::ControlChange {
        channel: 0,
        controller: 16,
        value: 127,
    });
    assert_eq!(matrix.get_parameter("speed"), Some(10.0));

    // Evaluate mid (64 / 127 ≈ 0.504)
    matrix.process_message(&MidiMessage::ControlChange {
        channel: 0,
        controller: 16,
        value: 64,
    });
    let linear_val = matrix.get_parameter("speed").unwrap();

    matrix.process_message(&MidiMessage::ControlChange {
        channel: 0,
        controller: 17,
        value: 64,
    });
    let exp_val = matrix.get_parameter("zoom_speed").unwrap();

    matrix.process_message(&MidiMessage::ControlChange {
        channel: 0,
        controller: 18,
        value: 64,
    });
    let log_val = matrix.get_parameter("glow").unwrap();

    // Verify curve properties at midpoint: Exp < Linear < Log
    assert!(
        exp_val < linear_val,
        "Exponential response must be lower than linear at mid"
    );
    assert!(
        log_val > linear_val,
        "Logarithmic response must be higher than linear at mid"
    );
}

#[test]
fn test_modulation_matrix_toggle_and_momentary() {
    let mut matrix = MidiModulationMatrix::new();

    // Note 60 -> Toggle blackout (0.0 or 1.0)
    matrix.add_mapping(MidiMapping {
        id: "toggle_blackout".to_string(),
        channel: None, // Wildcard channel
        controller_or_note: 60,
        is_note: true,
        target_param: "blackout".to_string(),
        mode: MappingMode::Toggle,
        min_val: 0.0,
        max_val: 1.0,
        curve: CurveType::Linear,
    });

    // Note 62 -> Momentary strobe (0.0 or 1.0)
    matrix.add_mapping(MidiMapping {
        id: "momentary_strobe".to_string(),
        channel: None,
        controller_or_note: 62,
        is_note: true,
        target_param: "strobe".to_string(),
        mode: MappingMode::Momentary,
        min_val: 0.0,
        max_val: 1.0,
        curve: CurveType::Linear,
    });

    // Initial state
    assert_eq!(matrix.get_parameter("blackout"), Some(0.0));
    assert_eq!(matrix.get_parameter("strobe"), Some(0.0));

    // First NoteOn 60 -> toggles ON
    matrix.process_message(&MidiMessage::NoteOn {
        channel: 3,
        note: 60,
        velocity: 120,
    });
    assert_eq!(matrix.get_parameter("blackout"), Some(1.0));

    // NoteOff 60 has no effect on Toggle
    matrix.process_message(&MidiMessage::NoteOff {
        channel: 3,
        note: 60,
        velocity: 0,
    });
    assert_eq!(matrix.get_parameter("blackout"), Some(1.0));

    // Second NoteOn 60 -> toggles OFF
    matrix.process_message(&MidiMessage::NoteOn {
        channel: 3,
        note: 60,
        velocity: 100,
    });
    assert_eq!(matrix.get_parameter("blackout"), Some(0.0));

    // Momentary NoteOn 62 -> HIGH
    matrix.process_message(&MidiMessage::NoteOn {
        channel: 5,
        note: 62,
        velocity: 100,
    });
    assert_eq!(matrix.get_parameter("strobe"), Some(1.0));

    // Momentary NoteOff 62 -> LOW
    matrix.process_message(&MidiMessage::NoteOff {
        channel: 5,
        note: 62,
        velocity: 0,
    });
    assert_eq!(matrix.get_parameter("strobe"), Some(0.0));
}

#[test]
fn test_channel_filtering() {
    let mut matrix = MidiModulationMatrix::new();

    // Mapping strictly on channel 4
    matrix.add_mapping(MidiMapping {
        id: "ch4_only".to_string(),
        channel: Some(4),
        controller_or_note: 7,
        is_note: false,
        target_param: "volume".to_string(),
        mode: MappingMode::Absolute,
        min_val: 0.0,
        max_val: 1.0,
        curve: CurveType::Linear,
    });

    // Message from channel 0 should be ignored
    let updates_ch0 = matrix.process_message(&MidiMessage::ControlChange {
        channel: 0,
        controller: 7,
        value: 127,
    });
    assert!(updates_ch0.is_empty());
    assert_eq!(matrix.get_parameter("volume"), Some(0.0));

    // Message from channel 4 should be processed
    let updates_ch4 = matrix.process_message(&MidiMessage::ControlChange {
        channel: 4,
        controller: 7,
        value: 127,
    });
    assert_eq!(updates_ch4.len(), 1);
    assert_eq!(matrix.get_parameter("volume"), Some(1.0));
}
