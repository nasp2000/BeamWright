//! Machine profiles: saved work areas for preparation only (PLAN.md §1.3).
//!
//! A profile stores a name, the work area size and notes. It deliberately holds
//! no connection, port, controller or firmware settings: `BeamWright` prepares
//! jobs and never talks to a machine.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use uuid::Uuid;

/// Current file format version.
pub const FORMAT_VERSION: u32 = 1;

/// Open-source firmware families a profile can be prepared for.
///
/// The value only labels the intended G-code dialect in the interface. No
/// connection, port, controller setting or firmware command is stored or sent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Firmware {
    /// GRBL 1.1 and compatible controllers.
    #[default]
    Grbl,
    /// GRBL-LPC.
    GrblLpc,
    /// FluidNC, the GRBL successor.
    FluidNc,
    /// Marlin.
    Marlin,
    /// SmoothieWare.
    Smoothie,
    /// Klipper.
    Klipper,
    /// LinuxCNC.
    LinuxCnc,
    /// RepRapFirmware.
    RepRapFirmware,
    /// Any other controller that reads plain G-code.
    Generic,
}

impl Firmware {
    pub fn label(self) -> &'static str {
        match self {
            Firmware::Grbl => "GRBL 1.1",
            Firmware::GrblLpc => "GRBL-LPC",
            Firmware::FluidNc => "FluidNC",
            Firmware::Marlin => "Marlin",
            Firmware::Smoothie => "SmoothieWare",
            Firmware::Klipper => "Klipper",
            Firmware::LinuxCnc => "LinuxCNC",
            Firmware::RepRapFirmware => "RepRapFirmware",
            Firmware::Generic => "Generic G-code",
        }
    }

    pub fn all() -> [Firmware; 9] {
        [
            Firmware::Grbl,
            Firmware::GrblLpc,
            Firmware::FluidNc,
            Firmware::Marlin,
            Firmware::Smoothie,
            Firmware::Klipper,
            Firmware::LinuxCnc,
            Firmware::RepRapFirmware,
            Firmware::Generic,
        ]
    }
}

/// One saved machine.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MachineProfile {
    pub id: String,
    pub name: String,
    pub work_width_mm: f64,
    pub work_height_mm: f64,
    /// Intended firmware family, used as a label and for choosing the dialect.
    #[serde(default)]
    pub firmware: Firmware,
    #[serde(default)]
    pub notes: String,
}

impl MachineProfile {
    pub fn new(name: impl Into<String>, width_mm: f64, height_mm: f64) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: name.into(),
            work_width_mm: width_mm,
            work_height_mm: height_mm,
            firmware: Firmware::default(),
            notes: String::new(),
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.id.trim().is_empty() {
            return Err("machine id must not be empty".to_string());
        }
        if self.name.trim().is_empty() {
            return Err("machine name must not be empty".to_string());
        }
        if !self.work_width_mm.is_finite() || !(10.0..=100_000.0).contains(&self.work_width_mm) {
            return Err("work width must be between 10 and 100000 mm".to_string());
        }
        if !self.work_height_mm.is_finite() || !(10.0..=100_000.0).contains(&self.work_height_mm) {
            return Err("work height must be between 10 and 100000 mm".to_string());
        }
        Ok(())
    }
}

impl Default for MachineProfile {
    fn default() -> Self {
        Self::new("My machine", 300.0, 200.0)
    }
}

/// Saved machines plus the first-run flag for the assistant.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MachineLibrary {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default)]
    pub machines: Vec<MachineProfile>,
    /// Set once the first-run assistant has been completed or skipped.
    #[serde(default)]
    pub first_run_done: bool,
}

fn default_version() -> u32 {
    FORMAT_VERSION
}

impl Default for MachineLibrary {
    fn default() -> Self {
        Self {
            version: FORMAT_VERSION,
            machines: Vec::new(),
            first_run_done: false,
        }
    }
}

impl MachineLibrary {
    pub fn validate(&self) -> Result<(), String> {
        if self.version != FORMAT_VERSION {
            return Err(format!(
                "unsupported machine library version {} (expected {FORMAT_VERSION})",
                self.version
            ));
        }
        let mut names = std::collections::HashSet::new();
        for machine in &self.machines {
            machine.validate()?;
            if !names.insert(machine.name.to_lowercase()) {
                return Err(format!("duplicate machine name: {}", machine.name));
            }
        }
        Ok(())
    }

    /// Load the library, or an empty first-run library when the file is absent.
    pub fn load_or_default(path: &Path) -> Result<Self, String> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let text = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
        let library: Self =
            serde_json::from_str(&text).map_err(|error| format!("invalid machines: {error}"))?;
        library.validate()?;
        Ok(library)
    }

    /// Save atomically, validating first.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        self.validate()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        let text = serde_json::to_string_pretty(self).map_err(|error| error.to_string())?;
        let temporary = path.with_extension("json.tmp");
        std::fs::write(&temporary, text).map_err(|error| error.to_string())?;
        std::fs::rename(&temporary, path).map_err(|error| error.to_string())?;
        Ok(())
    }

    /// Common open-source presets, excluding names that already exist.
    pub fn common_presets() -> Vec<MachineProfile> {
        let mut presets = vec![
            MachineProfile::new("GRBL 300 x 200", 300.0, 200.0),
            MachineProfile::new("GRBL 400 x 400", 400.0, 400.0),
            MachineProfile::new("FluidNC 600 x 400", 600.0, 400.0),
            MachineProfile::new("Marlin 220 x 220", 220.0, 220.0),
            MachineProfile::new("SmoothieWare 500 x 500", 500.0, 500.0),
            MachineProfile::new("Klipper 350 x 350", 350.0, 350.0),
            MachineProfile::new("LinuxCNC 1000 x 1000", 1000.0, 1000.0),
            MachineProfile::new("RepRapFirmware 400 x 300", 400.0, 300.0),
        ];
        for (preset, firmware) in presets.iter_mut().zip([
            Firmware::Grbl,
            Firmware::Grbl,
            Firmware::FluidNc,
            Firmware::Marlin,
            Firmware::Smoothie,
            Firmware::Klipper,
            Firmware::LinuxCnc,
            Firmware::RepRapFirmware,
        ]) {
            preset.firmware = firmware;
        }
        presets
    }

    /// Add every common preset whose name is not already used.
    pub fn add_common_presets(&mut self) -> usize {
        let mut added = 0;
        for preset in Self::common_presets() {
            if self.add(preset).is_ok() {
                added += 1;
            }
        }
        added
    }

    pub fn machine(&self, id: &str) -> Option<&MachineProfile> {
        self.machines.iter().find(|machine| machine.id == id)
    }

    pub fn add(&mut self, machine: MachineProfile) -> Result<(), String> {
        machine.validate()?;
        if self
            .machines
            .iter()
            .any(|existing| existing.name.eq_ignore_ascii_case(&machine.name))
        {
            return Err(format!("a profile named '{}' already exists", machine.name));
        }
        self.machines.push(machine);
        Ok(())
    }

    /// Write the library to a user-chosen file.
    pub fn export_to(&self, path: &Path) -> Result<(), String> {
        self.save(path)
    }

    /// Merge another library file, skipping names that already exist.
    pub fn import_into(&mut self, path: &Path) -> Result<usize, String> {
        let text = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
        let imported: Self =
            serde_json::from_str(&text).map_err(|error| format!("invalid machines: {error}"))?;
        imported.validate()?;
        let mut added = 0;
        for machine in imported.machines {
            if self
                .machines
                .iter()
                .any(|existing| existing.name.eq_ignore_ascii_case(&machine.name))
            {
                continue;
            }
            self.machines.push(machine);
            added += 1;
        }
        Ok(added)
    }

    pub fn remove(&mut self, id: &str) -> bool {
        let before = self.machines.len();
        self.machines.retain(|machine| machine.id != id);
        self.machines.len() != before
    }
}

/// File used inside the application data directory.
pub const MACHINES_FILE: &str = "machines.json";

pub fn default_path() -> PathBuf {
    crate::config::paths::data_dir().join(MACHINES_FILE)
}

#[cfg(test)]
#[allow(clippy::assert_is_empty)]
mod tests {
    use super::{FORMAT_VERSION, MachineLibrary, MachineProfile};

    #[test]
    fn a_profile_needs_a_name_and_a_work_area() {
        assert!(
            MachineProfile::new("Laser", 300.0, 200.0)
                .validate()
                .is_ok()
        );
        assert!(MachineProfile::new("", 300.0, 200.0).validate().is_err());
        assert!(MachineProfile::new("Laser", 0.0, 200.0).validate().is_err());
        assert!(
            MachineProfile::new("Laser", 300.0, f64::NAN)
                .validate()
                .is_err()
        );
    }

    #[test]
    fn firmware_labels_are_unique() {
        let labels: Vec<&str> = Firmware::all().iter().map(|item| item.label()).collect();
        let mut sorted = labels.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), labels.len());
    }

    #[test]
    fn common_presets_are_valid_and_distinct() {
        let presets = MachineLibrary::common_presets();
        assert!(presets.len() >= 8);
        for preset in &presets {
            preset.validate().unwrap();
        }
        let mut library = MachineLibrary::default();
        assert_eq!(library.add_common_presets(), presets.len());
        assert_eq!(library.add_common_presets(), 0);
    }

    #[test]
    fn names_are_unique_ignoring_case() {
        let mut library = MachineLibrary::default();
        library
            .add(MachineProfile::new("Blue", 300.0, 200.0))
            .unwrap();
        assert!(
            library
                .add(MachineProfile::new("blue", 400.0, 300.0))
                .is_err()
        );
        assert_eq!(library.machines.len(), 1);
        let id = library.machines[0].id.clone();
        assert!(library.remove(&id));
        assert!(library.machines.is_empty());
    }

    #[test]
    fn the_library_round_trips_with_the_version_and_flag() {
        let path = std::env::temp_dir().join("bw_machines_roundtrip.json");
        let _ = std::fs::remove_file(&path);
        let mut library = MachineLibrary::default();
        library
            .add(MachineProfile::new("Blue", 400.0, 300.0))
            .unwrap();
        library.first_run_done = true;
        library.save(&path).unwrap();
        let loaded = MachineLibrary::load_or_default(&path).unwrap();
        assert_eq!(loaded.version, FORMAT_VERSION);
        assert_eq!(loaded, library);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn old_files_without_the_flag_still_load() {
        let json = r#"{"version":1,"machines":[]}"#;
        let library: MachineLibrary = serde_json::from_str(json).unwrap();
        assert!(!library.first_run_done);
        assert!(library.validate().is_ok());
    }

    #[test]
    fn a_missing_file_is_a_first_run() {
        let path = std::env::temp_dir().join("bw_machines_missing.json");
        let _ = std::fs::remove_file(&path);
        let library = MachineLibrary::load_or_default(&path).unwrap();
        assert!(library.machines.is_empty());
        assert!(!library.first_run_done);
    }
}
