//! Hardware USB MIDI device discovery, connection, and hotplug manager.
//!
//! Uses `midir` for cross-platform low-latency MIDI input handling (WinMM/UWP on Windows,
//! CoreMIDI on macOS).

use crate::midi::parser::MidiMessage;
use midir::{Ignore, MidiInput, MidiInputConnection};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Mutex};

#[derive(Debug)]
pub enum MidiError {
    InitError(String),
    PortNotFound(String),
    ConnectError(String),
    PortInfoError(String),
}

impl fmt::Display for MidiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InitError(err) => write!(f, "Failed to initialize MIDI input: {}", err),
            Self::PortNotFound(name) => write!(f, "MIDI port '{}' not found", name),
            Self::ConnectError(err) => write!(f, "Failed to connect to MIDI port: {}", err),
            Self::PortInfoError(err) => write!(f, "Failed to query MIDI port info: {}", err),
        }
    }
}

impl std::error::Error for MidiError {}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MidiPortInfo {
    pub index: usize,
    pub name: String,
}

pub struct MidiDeviceManager {
    client_name: String,
    active_connections: HashMap<String, MidiInputConnection<()>>,
}

impl MidiDeviceManager {
    /// Create a new MidiDeviceManager with the specified client name.
    pub fn new(client_name: &str) -> Self {
        Self {
            client_name: client_name.to_string(),
            active_connections: HashMap::new(),
        }
    }

    /// Scan and enumerate currently connected physical MIDI input ports.
    pub fn scan_ports(&self) -> Result<Vec<MidiPortInfo>, MidiError> {
        let midi_in = MidiInput::new(&self.client_name)
            .map_err(|e| MidiError::InitError(e.to_string()))?;

        let ports = midi_in.ports();
        let mut port_infos = Vec::new();

        for (index, port) in ports.iter().enumerate() {
            let name = midi_in
                .port_name(port)
                .map_err(|e| MidiError::PortInfoError(e.to_string()))?;
            port_infos.push(MidiPortInfo { index, name });
        }

        Ok(port_infos)
    }

    /// Connect to a MIDI device by name and dispatch parsed messages to a callback closure.
    pub fn connect_device<F>(
        &mut self,
        port_name: &str,
        on_message: F,
    ) -> Result<(), MidiError>
    where
        F: Fn(MidiMessage) + Send + 'static,
    {
        // If already connected, disconnect first to rebind cleanly
        self.disconnect_device(port_name);

        let mut midi_in = MidiInput::new(&self.client_name)
            .map_err(|e| MidiError::InitError(e.to_string()))?;
        midi_in.ignore(Ignore::None);

        let ports = midi_in.ports();
        let target_port = ports
            .into_iter()
            .find(|p| midi_in.port_name(p).map(|n| n == port_name).unwrap_or(false))
            .ok_or_else(|| MidiError::PortNotFound(port_name.to_string()))?;

        let handler = Arc::new(Mutex::new(on_message));
        let connection = midi_in
            .connect(
                &target_port,
                port_name,
                move |_timestamp, bytes, _| {
                    if let Some(msg) = MidiMessage::parse(bytes) {
                        if let Ok(cb) = handler.lock() {
                            cb(msg);
                        }
                    }
                },
                (),
            )
            .map_err(|e| MidiError::ConnectError(e.to_string()))?;

        self.active_connections.insert(port_name.to_string(), connection);
        Ok(())
    }

    /// Disconnect from a specific connected device port.
    pub fn disconnect_device(&mut self, port_name: &str) -> bool {
        self.active_connections.remove(port_name).is_some()
    }

    /// Disconnect from all connected MIDI devices.
    pub fn disconnect_all(&mut self) {
        self.active_connections.clear();
    }

    /// List the names of all currently active device connections.
    pub fn connected_devices(&self) -> Vec<String> {
        self.active_connections.keys().cloned().collect()
    }
}

impl Default for MidiDeviceManager {
    fn default() -> Self {
        Self::new("Lumex MIDI Manager")
    }
}
