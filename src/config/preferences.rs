use super::DrawingPrefs;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralPrefs {
    pub default_unit: String,
    pub theme: String,
    pub confirm_destructive: bool,
    pub backup_enabled: bool,
    pub default_directory: String,
}

impl Default for GeneralPrefs {
    fn default() -> Self {
        Self {
            default_unit: "mm".to_string(),
            theme: "dark".to_string(),
            confirm_destructive: true,
            backup_enabled: true,
            default_directory: String::new(),
        }
    }
}

impl GeneralPrefs {
    pub fn validate(&self) -> Result<(), String> {
        if self.default_unit.is_empty() {
            return Err("default unit must not be empty".to_string());
        }
        if self.theme.is_empty() {
            return Err("theme must not be empty".to_string());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppPreferences {
    pub general: GeneralPrefs,
    pub drawing: DrawingPrefs,
}

fn default_snap_step() -> f64 {
    1.0
}

fn default_true() -> bool {
    true
}

/// File used inside the application data directory.
pub const PREFERENCES_FILE: &str = "preferences.json";

pub fn default_path() -> std::path::PathBuf {
    crate::config::paths::data_dir().join(PREFERENCES_FILE)
}

impl AppPreferences {
    /// Load preferences, or defaults when the file does not exist.
    pub fn load_or_default(path: &std::path::Path) -> Result<Self, String> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let text = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
        let preferences: Self =
            serde_json::from_str(&text).map_err(|error| format!("invalid preferences: {error}"))?;
        preferences.validate()?;
        Ok(preferences)
    }

    /// Save preferences atomically.
    pub fn save(&self, path: &std::path::Path) -> Result<(), String> {
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

    pub fn validate(&self) -> Result<(), String> {
        self.general.validate()?;
        self.drawing.validate()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_valid() {
        assert!(AppPreferences::default().validate().is_ok());
    }

    #[test]
    fn reject_empty_unit() {
        let mut p = AppPreferences::default();
        p.general.default_unit = String::new();
        assert!(p.validate().is_err());
    }

    #[test]
    fn reject_bad_drawing() {
        let mut p = AppPreferences::default();
        p.drawing.grid_spacing_mm = 0.0;
        assert!(p.validate().is_err());
    }

    #[test]
    fn the_first_run_is_dark() {
        let preferences = AppPreferences::default();
        assert_eq!(preferences.general.theme, "dark");
        assert!(preferences.validate().is_ok());
    }

    #[test]
    fn missing_file_loads_defaults() {
        let path = std::env::temp_dir().join("bw_prefs_missing.json");
        let _ = std::fs::remove_file(&path);
        let preferences = AppPreferences::load_or_default(&path).unwrap();
        assert_eq!(preferences.general.theme, "dark");
        assert_eq!(preferences.drawing.grid_spacing_mm, 10.0);
    }

    #[test]
    fn save_and_load_round_trip() {
        let path = std::env::temp_dir().join("bw_prefs_roundtrip.json");
        let _ = std::fs::remove_file(&path);
        let mut preferences = AppPreferences::default();
        preferences.drawing.grid_spacing_mm = 5.0;
        preferences.drawing.snap_enabled = true;
        preferences.drawing.snap_nodes = true;
        preferences.drawing.snap_centers = true;
        preferences.drawing.snap_intersections = true;
        preferences.drawing.snap_guides = true;
        preferences.drawing.guides_visible = false;
        preferences.drawing.selection_color = "#FF0000".to_string();
        preferences.drawing.handle_style = "circle".to_string();
        preferences.drawing.zoom_step = 1.5;
        preferences.drawing.select_whole_group = false;
        preferences.drawing.scroll_zoom_step = 1.2;
        preferences.drawing.max_image_pixels = 20_000_000;
        preferences.general.theme = "dark".to_string();
        preferences.save(&path).unwrap();
        let loaded = AppPreferences::load_or_default(&path).unwrap();
        assert_eq!(loaded.drawing.grid_spacing_mm, 5.0);
        assert!(loaded.drawing.snap_enabled);
        assert!(loaded.drawing.snap_nodes);
        assert!(!loaded.drawing.snap_endpoints);
        assert!(loaded.drawing.snap_centers);
        assert!(loaded.drawing.snap_intersections);
        assert!(loaded.drawing.snap_guides);
        assert!(!loaded.drawing.guides_visible);
        assert_eq!(loaded.drawing.selection_color, "#FF0000");
        assert_eq!(loaded.drawing.handle_style, "circle");
        assert!((loaded.drawing.zoom_step - 1.5).abs() < 1e-9);
        assert!(!loaded.drawing.select_whole_group);
        assert!((loaded.drawing.scroll_zoom_step - 1.2).abs() < 1e-9);
        assert_eq!(loaded.drawing.max_image_pixels, 20_000_000);
        assert!(!loaded.drawing.snap_midpoints);
        assert_eq!(loaded.general.theme, "dark");
        assert!(!path.with_extension("json.tmp").exists());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn invalid_drawing_appearance_is_rejected() {
        let mut preferences = AppPreferences::default();
        preferences.drawing.selection_color = "red".to_string();
        assert!(preferences.validate().is_err());
        preferences.drawing.selection_color = "#000000".to_string();
        preferences.drawing.handle_style = "triangle".to_string();
        assert!(preferences.validate().is_err());
        preferences.drawing.handle_style = "square".to_string();
        preferences.drawing.zoom_step = 1.0;
        assert!(preferences.validate().is_err());
        preferences.drawing.zoom_step = 1.25;
        preferences.drawing.scroll_zoom_step = 5.0;
        assert!(preferences.validate().is_err());
        preferences.drawing.scroll_zoom_step = 1.15;
        preferences.drawing.max_image_pixels = 10;
        assert!(preferences.validate().is_err());
    }

    #[test]
    fn invalid_preferences_file_is_rejected() {
        let path = std::env::temp_dir().join("bw_prefs_invalid.json");
        std::fs::write(&path, "{not json").unwrap();
        assert!(AppPreferences::load_or_default(&path).is_err());
        let _ = std::fs::remove_file(&path);
        let mut preferences = AppPreferences::default();
        preferences.drawing.snap_step_mm = 0.0;
        assert!(preferences.save(&path).is_err());
    }

    #[test]
    fn round_trip_serde() {
        let p = AppPreferences::default();
        let s = serde_json::to_string(&p).unwrap();
        let q: AppPreferences = serde_json::from_str(&s).unwrap();
        assert_eq!(q.general.theme, p.general.theme);
    }
}
