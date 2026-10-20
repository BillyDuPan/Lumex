//! Modulation matrix and MIDI parameter binding engine.
//!
//! Maps physical hardware MIDI controllers (CC, Note, Pitch Bend) to internal
//! instrument parameters with configurable response curves (Linear, Exponential,
//! Logarithmic, S-Curve) and mapping modes (Absolute, Relative, Toggle, Momentary).

use crate::midi::parser::MidiMessage;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Mode of parameter modulation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MappingMode {
    /// 7-bit (0..127) linearly or curve-mapped to [min_val, max_val].
    Absolute,
    /// Endless rotary encoder relative step adjustments (+/- delta).
    Relative,
    /// Note On toggles between min_val and max_val.
    Toggle,
    /// High (max_val) while note held, low (min_val) on release.
    Momentary,
}

/// Mathematical response curve applied to normalized control values.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CurveType {
    Linear,
    Exponential, // Great for zoom and frequency (steep at top)
    Logarithmic, // Great for decibel scaling / fine low-end control
    SCurve,      // Smooth cubic acceleration at limits
}

impl CurveType {
    /// Apply response curve to normalized value in [0.0, 1.0].
    #[inline]
    pub fn apply(&self, norm: f32) -> f32 {
        let n = norm.clamp(0.0, 1.0);
        match self {
            Self::Linear => n,
            Self::Exponential => n.powf(2.5),
            Self::Logarithmic => n.sqrt(),
            Self::SCurve => n * n * (3.0 - 2.0 * n),
        }
    }
}

/// A binding rule connecting a MIDI control source to an instrument parameter.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MidiMapping {
    pub id: String,
    pub channel: Option<u8>, // None = wildcard (any channel 0..15), Some(ch) = specific channel
    pub controller_or_note: u8,
    pub is_note: bool,
    pub target_param: String,
    pub mode: MappingMode,
    pub min_val: f32,
    pub max_val: f32,
    pub curve: CurveType,
}

impl MidiMapping {
    /// Evaluate raw 7-bit value (0..127) against this mapping's curve and range.
    #[inline]
    pub fn evaluate_7bit(&self, raw_val: u8) -> f32 {
        let norm = (raw_val as f32) / 127.0;
        let curved = self.curve.apply(norm);
        self.min_val + curved * (self.max_val - self.min_val)
    }

    /// Evaluate raw 14-bit value (0..16383) against this mapping's curve and range.
    #[inline]
    pub fn evaluate_14bit(&self, raw_val: u16) -> f32 {
        let norm = (raw_val as f32) / 16383.0;
        let curved = self.curve.apply(norm);
        self.min_val + curved * (self.max_val - self.min_val)
    }

    /// Check if an incoming message matches this mapping's channel and identifier.
    pub fn matches(&self, msg: &MidiMessage) -> bool {
        match msg {
            MidiMessage::NoteOn { channel, note, .. } | MidiMessage::NoteOff { channel, note, .. } => {
                if !self.is_note {
                    return false;
                }
                if let Some(ch) = self.channel {
                    if ch != *channel {
                        return false;
                    }
                }
                self.controller_or_note == *note
            }
            MidiMessage::ControlChange {
                channel,
                controller,
                ..
            } => {
                if self.is_note {
                    return false;
                }
                if let Some(ch) = self.channel {
                    if ch != *channel {
                        return false;
                    }
                }
                self.controller_or_note == *controller
            }
            _ => false,
        }
    }
}

/// Modulation matrix storing mappings and evaluating parameter updates.
pub struct MidiModulationMatrix {
    mappings: Vec<MidiMapping>,
    parameters: HashMap<String, f32>,
    toggle_states: HashMap<String, bool>,
}

impl MidiModulationMatrix {
    pub fn new() -> Self {
        Self {
            mappings: Vec::new(),
            parameters: HashMap::new(),
            toggle_states: HashMap::new(),
        }
    }

    /// Add a new mapping rule to the matrix.
    pub fn add_mapping(&mut self, mapping: MidiMapping) {
        // If parameter doesn't exist, initialize with min_val
        if !self.parameters.contains_key(&mapping.target_param) {
            self.parameters.insert(mapping.target_param.clone(), mapping.min_val);
        }
        self.mappings.push(mapping);
    }

    /// Remove a mapping rule by ID.
    pub fn remove_mapping(&mut self, id: &str) -> bool {
        let len_before = self.mappings.len();
        self.mappings.retain(|m| m.id != id);
        self.mappings.len() < len_before
    }

    /// Clear all mapping rules.
    pub fn clear_mappings(&mut self) {
        self.mappings.clear();
    }

    /// Retrieve all registered mappings.
    pub fn mappings(&self) -> &[MidiMapping] {
        &self.mappings
    }

    /// Get current value of a parameter.
    pub fn get_parameter(&self, name: &str) -> Option<f32> {
        self.parameters.get(name).copied()
    }

    /// Set parameter value directly.
    pub fn set_parameter(&mut self, name: &str, value: f32) {
        self.parameters.insert(name.to_string(), value);
    }

    /// Process an incoming MIDI message and return all mutated `(target_param, new_value)` pairs.
    pub fn process_message(&mut self, msg: &MidiMessage) -> Vec<(String, f32)> {
        let mut updates = Vec::new();

        for mapping in &self.mappings {
            if !mapping.matches(msg) {
                continue;
            }

            match msg {
                MidiMessage::ControlChange { value, .. } => {
                    match mapping.mode {
                        MappingMode::Absolute => {
                            let new_val = mapping.evaluate_7bit(*value);
                            self.parameters.insert(mapping.target_param.clone(), new_val);
                            updates.push((mapping.target_param.clone(), new_val));
                        }
                        MappingMode::Relative => {
                            // Standard two's complement / relative encoder format:
                            // Values 1..63 = positive steps, values 65..127 = negative steps (value - 128)
                            let delta = if *value >= 64 {
                                (*value as i32 - 128) as f32
                            } else {
                                *value as f32
                            };
                            let cur = self
                                .parameters
                                .get(&mapping.target_param)
                                .copied()
                                .unwrap_or(mapping.min_val);
                            let step = (mapping.max_val - mapping.min_val) / 127.0;
                            let new_val = (cur + delta * step).clamp(mapping.min_val, mapping.max_val);
                            self.parameters.insert(mapping.target_param.clone(), new_val);
                            updates.push((mapping.target_param.clone(), new_val));
                        }
                        MappingMode::Toggle => {
                            if *value > 63 {
                                let state = self
                                    .toggle_states
                                    .entry(mapping.target_param.clone())
                                    .or_insert(false);
                                *state = !*state;
                                let new_val = if *state { mapping.max_val } else { mapping.min_val };
                                self.parameters.insert(mapping.target_param.clone(), new_val);
                                updates.push((mapping.target_param.clone(), new_val));
                            }
                        }
                        MappingMode::Momentary => {
                            let new_val = if *value > 63 { mapping.max_val } else { mapping.min_val };
                            self.parameters.insert(mapping.target_param.clone(), new_val);
                            updates.push((mapping.target_param.clone(), new_val));
                        }
                    }
                }

                MidiMessage::NoteOn { velocity, .. } => {
                    match mapping.mode {
                        MappingMode::Absolute => {
                            let new_val = mapping.evaluate_7bit(*velocity);
                            self.parameters.insert(mapping.target_param.clone(), new_val);
                            updates.push((mapping.target_param.clone(), new_val));
                        }
                        MappingMode::Toggle => {
                            let state = self
                                .toggle_states
                                .entry(mapping.target_param.clone())
                                .or_insert(false);
                            *state = !*state;
                            let new_val = if *state { mapping.max_val } else { mapping.min_val };
                            self.parameters.insert(mapping.target_param.clone(), new_val);
                            updates.push((mapping.target_param.clone(), new_val));
                        }
                        MappingMode::Momentary => {
                            let new_val = mapping.max_val;
                            self.parameters.insert(mapping.target_param.clone(), new_val);
                            updates.push((mapping.target_param.clone(), new_val));
                        }
                        MappingMode::Relative => {}
                    }
                }

                MidiMessage::NoteOff { .. } => {
                    if mapping.mode == MappingMode::Momentary {
                        let new_val = mapping.min_val;
                        self.parameters.insert(mapping.target_param.clone(), new_val);
                        updates.push((mapping.target_param.clone(), new_val));
                    }
                }

                _ => {}
            }
        }

        updates
    }
}

impl Default for MidiModulationMatrix {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_curves() {
        assert_eq!(CurveType::Linear.apply(0.0), 0.0);
        assert_eq!(CurveType::Linear.apply(1.0), 1.0);
        assert_eq!(CurveType::Linear.apply(0.5), 0.5);

        // Exponential curve: 0.5^2.5 ≈ 0.1767 (< 0.5)
        let exp_mid = CurveType::Exponential.apply(0.5);
        assert!(exp_mid < 0.25);

        // Logarithmic curve: sqrt(0.5) ≈ 0.707 (> 0.5)
        let log_mid = CurveType::Logarithmic.apply(0.5);
        assert!(log_mid > 0.70);

        // SCurve: 0.5 * 0.5 * (3 - 1) = 0.5
        assert_eq!(CurveType::SCurve.apply(0.5), 0.5);
    }

    #[test]
    fn test_matrix_cc_absolute() {
        let mut matrix = MidiModulationMatrix::new();
        matrix.add_mapping(MidiMapping {
            id: "kaleido_segments".to_string(),
            channel: Some(0),
            controller_or_note: 16,
            is_note: false,
            target_param: "fx.kaleidoscope.segments".to_string(),
            mode: MappingMode::Absolute,
            min_val: 2.0,
            max_val: 16.0,
            curve: CurveType::Linear,
        });

        let msg = MidiMessage::ControlChange {
            channel: 0,
            controller: 16,
            value: 127,
        };
        let updates = matrix.process_message(&msg);
        assert_eq!(updates.len(), 1);
        assert_eq!(updates[0].0, "fx.kaleidoscope.segments");
        assert!((updates[0].1 - 16.0).abs() < 1e-4);
    }
}
