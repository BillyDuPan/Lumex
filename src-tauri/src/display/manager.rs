//! Display Manager for Lumex
//!
//! Provides monitor enumeration, display metadata tracking, resolution,
//! refresh rate, and target monitor selection index.

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Runtime};

/// Information about a connected display device.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MonitorInfo {
    pub index: usize,
    pub id: String,
    pub name: String,
    pub primary: bool,
    pub width: u32,
    pub height: u32,
    pub x: i32,
    pub y: i32,
    pub scale_factor: f64,
    pub refresh_rate_millihertz: Option<u32>,
}

impl MonitorInfo {
    /// Calculate the aspect ratio (width / height) safely.
    pub fn aspect_ratio(&self) -> f32 {
        if self.height > 0 {
            self.width as f32 / self.height as f32
        } else {
            1.0
        }
    }

    /// Check if display is landscape orientation.
    pub fn is_landscape(&self) -> bool {
        self.width >= self.height
    }
}

/// Manages connected displays, active monitor configurations, and target selection.
#[derive(Debug, Clone, Default)]
pub struct DisplayManager {
    selected_index: usize,
    monitors: Vec<MonitorInfo>,
}

impl DisplayManager {
    /// Create a new, uninitialized display manager.
    pub fn new() -> Self {
        Self {
            selected_index: 0,
            monitors: Vec::new(),
        }
    }

    /// Create a display manager with a pre-configured list of monitors (useful for testing).
    pub fn from_monitors(monitors: Vec<MonitorInfo>) -> Self {
        // Automatically default to the first secondary monitor if available, otherwise 0
        let default_idx = monitors
            .iter()
            .position(|m| !m.primary)
            .unwrap_or(0);

        Self {
            selected_index: default_idx,
            monitors,
        }
    }

    /// Enumerate all connected monitors via Tauri AppHandle.
    pub fn enumerate<R: Runtime>(app: &AppHandle<R>) -> Result<Vec<MonitorInfo>, String> {
        let primary_name = app
            .primary_monitor()
            .map_err(|e| format!("Failed to query primary monitor: {}", e))?
            .and_then(|m| m.name().cloned());

        let raw_monitors = app
            .available_monitors()
            .map_err(|e| format!("Failed to query available monitors: {}", e))?;

        let mut list = Vec::with_capacity(raw_monitors.len());
        for (i, m) in raw_monitors.into_iter().enumerate() {
            let name = m
                .name()
                .cloned()
                .unwrap_or_else(|| format!("Display {}", i + 1));
            let is_primary = primary_name.as_ref().map_or(i == 0, |p| p == &name);
            let size = m.size();
            let pos = m.position();

            list.push(MonitorInfo {
                index: i,
                id: format!("display-{}", i),
                name,
                primary: is_primary,
                width: size.width,
                height: size.height,
                x: pos.x,
                y: pos.y,
                scale_factor: m.scale_factor(),
                refresh_rate_millihertz: Some(60_000), // Standard 60Hz baseline
            });
        }

        Ok(list)
    }

    /// Refresh display state and re-enumerate monitors.
    pub fn refresh<R: Runtime>(&mut self, app: &AppHandle<R>) -> Result<&[MonitorInfo], String> {
        self.monitors = Self::enumerate(app)?;
        // If current selection is now out of bounds, clamp to valid range
        if !self.monitors.is_empty() && self.selected_index >= self.monitors.len() {
            self.selected_index = self
                .monitors
                .iter()
                .position(|m| !m.primary)
                .unwrap_or(0);
        }
        Ok(&self.monitors)
    }

    /// Select monitor by index.
    pub fn select_monitor(&mut self, index: usize) -> Result<&MonitorInfo, String> {
        if index < self.monitors.len() {
            self.selected_index = index;
            Ok(&self.monitors[index])
        } else {
            Err(format!(
                "Invalid monitor index {}: available count is {}",
                index,
                self.monitors.len()
            ))
        }
    }

    /// Get information on the currently selected target monitor.
    pub fn get_selected(&self) -> Option<&MonitorInfo> {
        self.monitors.get(self.selected_index)
    }

    /// Get index of the currently selected monitor.
    pub fn selected_index(&self) -> usize {
        self.selected_index
    }

    /// Get slice of all enumerated monitors.
    pub fn monitors(&self) -> &[MonitorInfo] {
        &self.monitors
    }

    /// Total count of detected monitors.
    pub fn monitor_count(&self) -> usize {
        self.monitors.len()
    }

    /// Get information for the primary monitor.
    pub fn primary_monitor(&self) -> Option<&MonitorInfo> {
        self.monitors.iter().find(|m| m.primary)
    }

    /// Get information for all non-primary (secondary/projector) monitors.
    pub fn secondary_monitors(&self) -> Vec<&MonitorInfo> {
        self.monitors.iter().filter(|m| !m.primary).collect()
    }

    /// Retrieve the corresponding `tauri::Monitor` handle for the selected target.
    pub fn target_monitor_for_projector<R: Runtime>(
        &self,
        app: &AppHandle<R>,
    ) -> Result<tauri::Monitor, String> {
        let available = app
            .available_monitors()
            .map_err(|e| format!("Failed to query available monitors: {}", e))?;

        if available.is_empty() {
            return Err("No available monitors found".to_string());
        }

        // 1. Try selected index if valid
        if self.selected_index < available.len() {
            return Ok(available[self.selected_index].clone());
        }

        // 2. Try first non-primary monitor
        let primary_name = app
            .primary_monitor()
            .ok()
            .flatten()
            .and_then(|m| m.name().cloned());

        if let Some(sec) = available.iter().find(|m| {
            m.name()
                .map_or(false, |n| primary_name.as_ref() != Some(n))
        }) {
            return Ok(sec.clone());
        }

        // 3. Fallback to first available
        Ok(available[0].clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display_manager_defaults_and_selection() {
        let dm = DisplayManager::new();
        assert_eq!(dm.monitor_count(), 0);
        assert!(dm.get_selected().is_none());

        let fake_monitors = vec![
            MonitorInfo {
                index: 0,
                id: "disp-0".into(),
                name: "Primary Display".into(),
                primary: true,
                width: 1920,
                height: 1080,
                x: 0,
                y: 0,
                scale_factor: 1.0,
                refresh_rate_millihertz: Some(60_000),
            },
            MonitorInfo {
                index: 1,
                id: "disp-1".into(),
                name: "Projector Beamer".into(),
                primary: false,
                width: 3840,
                height: 2160,
                x: 1920,
                y: 0,
                scale_factor: 1.0,
                refresh_rate_millihertz: Some(120_000),
            },
        ];

        let mut dm2 = DisplayManager::from_monitors(fake_monitors);
        assert_eq!(dm2.monitor_count(), 2);
        // By default, from_monitors should prefer the secondary display (index 1)
        assert_eq!(dm2.selected_index(), 1);
        let sel = dm2.get_selected().unwrap();
        assert_eq!(sel.name, "Projector Beamer");
        assert_eq!(sel.width, 3840);
        assert_eq!(sel.height, 2160);
        assert!(!sel.primary);

        // Switch selection to primary
        dm2.select_monitor(0).unwrap();
        assert_eq!(dm2.selected_index(), 0);
        assert_eq!(dm2.get_selected().unwrap().name, "Primary Display");

        // Invalid index should return Err
        assert!(dm2.select_monitor(5).is_err());
    }

    #[test]
    fn test_monitor_aspect_ratio_and_orientation() {
        let landscape = MonitorInfo {
            index: 0,
            id: "m1".into(),
            name: "16:9".into(),
            primary: true,
            width: 1920,
            height: 1080,
            x: 0,
            y: 0,
            scale_factor: 1.0,
            refresh_rate_millihertz: None,
        };
        assert!((landscape.aspect_ratio() - (16.0 / 9.0)).abs() < 1e-4);
        assert!(landscape.is_landscape());

        let portrait = MonitorInfo {
            index: 1,
            id: "m2".into(),
            name: "9:16".into(),
            primary: false,
            width: 1080,
            height: 1920,
            x: 0,
            y: 0,
            scale_factor: 1.0,
            refresh_rate_millihertz: None,
        };
        assert!(!portrait.is_landscape());
        assert!((portrait.aspect_ratio() - (9.0 / 16.0)).abs() < 1e-4);

        let zero_h = MonitorInfo {
            index: 2,
            id: "m3".into(),
            name: "zero".into(),
            primary: false,
            width: 100,
            height: 0,
            x: 0,
            y: 0,
            scale_factor: 1.0,
            refresh_rate_millihertz: None,
        };
        assert_eq!(zero_h.aspect_ratio(), 1.0);
    }
}
