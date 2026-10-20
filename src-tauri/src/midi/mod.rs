//! Hardware USB MIDI Subsystem
//!
//! Provides device discovery, connection management, zero-allocation packet parsing,
//! and modulation matrix parameter mapping.

pub mod device;
pub mod matrix;
pub mod parser;

pub use device::{MidiDeviceManager, MidiError, MidiPortInfo};
pub use matrix::{CurveType, MappingMode, MidiMapping, MidiModulationMatrix};
pub use parser::MidiMessage;
