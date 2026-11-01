use super::DrawingPrefs;
use serde::{Deserialize, Serialize};

/// One category of the preferences window.
///
/// Every setting is shown exactly once, in the category it belongs to, so the
/// window never becomes a flat list of unrelated controls. The categories and
/// their keywords are also the search index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    General,
    Interface,
    Drawing,
    Images,
    Files,
    Machines,
    Advanced,
}

impl Section {
    /// Every category, in the order shown in the window.
    pub const ALL: [Section; 7] = [
        Section::General,
        Section::Interface,
        Section::Drawing,
        Section::Images,
        Section::Files,
        Section::Machines,
        Section::Advanced,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Section::General => "General",
            Section::Interface => "Interface",
            Section::Drawing => "Drawing and snapping",
            Section::Images => "Images",
            Section::Files => "Files and backup",
            Section::Machines => "Machines and profiles",
            Section::Advanced => "Advanced",
        }
    }

    /// A one-line description shown above the category's controls.
    pub fn hint(self) -> &'static str {
        match self {
            Section::General => "Units, text and confirmations.",
            Section::Interface => "Theme, canvas overlays and navigation.",
            Section::Drawing => "Grid spacing and every snapping target.",
            Section::Images => "Limits applied when a raster image is imported.",
            Section::Files => "Default folder, backups and recent projects.",
            Section::Machines => "Work areas and material libraries.",
            Section::Advanced => "Where the preferences are stored.",
        }
    }

    /// Extra words that find this category in the search box.
    pub fn keywords(self) -> &'static [&'static str] {
        match self {
            Section::General => &["unit", "font", "text", "confirm", "discard", "language"],
            Section::Interface => &[
                "theme",
                "dark",
                "light",
                "rulers",
                "selection",
                "handles",
                "colour",
                "zoom",
                "guides",
                "group",
            ],
            Section::Drawing => &[
                "grid",
                "snap",
                "nodes",
                "endpoints",
                "centres",
                "midpoints",
                "intersections",
                "bounds",
                "tolerance",
            ],
            Section::Images => &["raster", "pixel", "resolution", "import", "bitmap", "photo"],
            Section::Files => &[
                "directory",
                "folder",
                "backup",
                "recent",
                "autosave",
                "project",
            ],
            Section::Machines => &[
                "machine",
                "profile",
                "material",
                "work area",
                "firmware",
                "preset",
            ],
            Section::Advanced => &["data", "path", "reset", "defaults", "storage"],
        }
    }

    /// True when the category label or one of its keywords contains `query`.
    ///
    /// The comparison ignores case and surrounding whitespace; an empty query
    /// matches every category.
    pub fn matches(self, query: &str) -> bool {
        let query = query.trim().to_lowercase();
        if query.is_empty() {
            return true;
        }
        self.label().to_lowercase().contains(&query)
            || self
                .keywords()
                .iter()
                .any(|keyword| keyword.contains(&query))
    }

    /// The first category matching `query`, or `None` when nothing matches.
    pub fn first_match(query: &str) -> Option<Section> {
        Self::ALL.into_iter().find(|section| section.matches(query))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralPrefs {
    pub default_unit: String,
    pub theme: String,
    pub confirm_destructive: bool,
    pub backup_enabled: bool,
    pub default_directory: String,
    /// Font used for new text objects; existing text keeps its own font.
    #[serde(default = "default_text_font")]
    pub text_font: String,
    /// Recently opened or saved project files, newest first.
    #[serde(default)]
    pub recent_projects: Vec<String>,
}

impl Default for GeneralPrefs {
    fn default() -> Self {
        Self {
            default_unit: "mm".to_string(),
            theme: "dark".to_string(),
            confirm_destructive: true,
            backup_enabled: true,
            default_directory: String::new(),
            text_font: default_text_font(),
            recent_projects: Vec::new(),
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
        if self.text_font.trim().is_empty() {
            return Err("text font must not be empty".to_string());
        }
        Ok(())
    }
}

fn default_text_font() -> String {
    crate::textfont::BUNDLED_NAME.to_string()
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

    /// Restore the defaults of one category, leaving every other category alone.
    ///
    /// The recent-project list is history rather than a setting, so even a full
    /// reset keeps it.
    pub fn reset_section(&mut self, section: Section) {
        let defaults = Self::default();
        match section {
            Section::General => {
                self.general
                    .default_unit
                    .clone_from(&defaults.general.default_unit);
                self.general
                    .text_font
                    .clone_from(&defaults.general.text_font);
                self.general.confirm_destructive = defaults.general.confirm_destructive;
            }
            Section::Interface => {
                self.general.theme.clone_from(&defaults.general.theme);
                self.drawing.show_rulers = defaults.drawing.show_rulers;
                self.drawing.show_selection = defaults.drawing.show_selection;
                self.drawing
                    .selection_color
                    .clone_from(&defaults.drawing.selection_color);
                self.drawing
                    .handle_style
                    .clone_from(&defaults.drawing.handle_style);
                self.drawing.zoom_step = defaults.drawing.zoom_step;
                self.drawing.scroll_zoom_step = defaults.drawing.scroll_zoom_step;
                self.drawing.select_whole_group = defaults.drawing.select_whole_group;
                self.drawing.guides_visible = defaults.drawing.guides_visible;
            }
            Section::Drawing => {
                self.drawing.grid_spacing_mm = defaults.drawing.grid_spacing_mm;
                self.drawing.snap_tolerance_px = defaults.drawing.snap_tolerance_px;
                self.drawing.snap_step_mm = defaults.drawing.snap_step_mm;
                self.drawing.snap_enabled = defaults.drawing.snap_enabled;
                self.drawing.snap_nodes = defaults.drawing.snap_nodes;
                self.drawing.snap_endpoints = defaults.drawing.snap_endpoints;
                self.drawing.snap_centers = defaults.drawing.snap_centers;
                self.drawing.snap_midpoints = defaults.drawing.snap_midpoints;
                self.drawing.snap_intersections = defaults.drawing.snap_intersections;
                self.drawing.snap_limits = defaults.drawing.snap_limits;
                self.drawing.snap_guides = defaults.drawing.snap_guides;
            }
            Section::Images => {
                self.drawing.max_image_pixels = defaults.drawing.max_image_pixels;
            }
            Section::Files => {
                self.general
                    .default_directory
                    .clone_from(&defaults.general.default_directory);
                self.general.backup_enabled = defaults.general.backup_enabled;
            }
            Section::Machines => {}
            Section::Advanced => {
                let recent = std::mem::take(&mut self.general.recent_projects);
                *self = defaults;
                self.general.recent_projects = recent;
            }
        }
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
    fn old_preferences_receive_the_bundled_text_font() {
        let json = r#"{"default_unit":"mm","theme":"light","confirm_destructive":true,"backup_enabled":true,"default_directory":""}"#;
        let general: GeneralPrefs = serde_json::from_str(json).unwrap();
        assert_eq!(general.text_font, crate::textfont::BUNDLED_NAME);
        assert!(general.validate().is_ok());
    }

    #[test]
    fn an_empty_text_font_is_rejected() {
        let mut preferences = AppPreferences::default();
        preferences.general.text_font = "   ".to_string();
        assert!(preferences.validate().is_err());
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
        preferences.drawing.show_selection = false;
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
        assert!(!loaded.drawing.show_selection);
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

    #[test]
    fn every_section_has_a_unique_label_hint_and_keywords() {
        let mut labels = Vec::new();
        for section in Section::ALL {
            assert_ne!(section.label().trim(), "");
            assert_ne!(section.hint().trim(), "");
            assert!(matches!(section.keywords(), [_, ..]));
            assert!(
                !labels.contains(&section.label()),
                "duplicate label {}",
                section.label()
            );
            labels.push(section.label());
            for keyword in section.keywords() {
                assert_ne!(keyword.trim(), "");
                assert_eq!(*keyword, keyword.to_lowercase());
            }
        }
        assert_eq!(labels.len(), Section::ALL.len());
    }

    #[test]
    fn search_matches_labels_keywords_and_ignores_case() {
        for section in Section::ALL {
            assert!(section.matches(""));
            assert!(section.matches("   "));
            assert!(section.matches(&section.label().to_lowercase()));
            assert!(section.matches(&section.label().to_uppercase()));
            let keyword = section.keywords()[0];
            assert!(section.matches(keyword));
            assert!(section.matches(&keyword.to_uppercase()));
        }
        assert!(Section::Drawing.matches("snap"));
        assert!(Section::Drawing.matches("  SNAP  "));
        assert!(Section::Images.matches("pixel"));
        assert!(!Section::General.matches("snap"));
        assert!(!Section::General.matches("nothing-matches-this"));
    }

    #[test]
    fn first_match_finds_a_category_or_nothing() {
        assert_eq!(Section::first_match(""), Some(Section::General));
        assert_eq!(Section::first_match("pixel"), Some(Section::Images));
        assert_eq!(Section::first_match("backup"), Some(Section::Files));
        assert_eq!(Section::first_match("nothing-matches-this"), None);
    }

    #[test]
    fn reset_section_only_touches_its_own_category() {
        let mut preferences = AppPreferences::default();
        preferences.general.default_unit = "in".to_string();
        preferences.general.theme = "light".to_string();
        preferences.general.backup_enabled = false;
        preferences.drawing.grid_spacing_mm = 2.5;
        preferences.drawing.snap_enabled = true;
        preferences.drawing.max_image_pixels = 2_000_000;
        preferences.drawing.zoom_step = 1.5;

        preferences.reset_section(Section::Drawing);
        let defaults = AppPreferences::default();
        assert!(
            (preferences.drawing.grid_spacing_mm - defaults.drawing.grid_spacing_mm).abs() < 1e-9
        );
        assert_eq!(
            preferences.drawing.snap_enabled,
            defaults.drawing.snap_enabled
        );
        // Other categories are untouched.
        assert_eq!(preferences.general.default_unit, "in");
        assert_eq!(preferences.general.theme, "light");
        assert!(!preferences.general.backup_enabled);
        assert_eq!(preferences.drawing.max_image_pixels, 2_000_000);
        assert!((preferences.drawing.zoom_step - 1.5).abs() < 1e-9);
        assert!(preferences.validate().is_ok());
    }

    #[test]
    fn every_category_resets_to_a_valid_state() {
        for section in Section::ALL {
            let mut preferences = AppPreferences::default();
            preferences.general.default_unit = "in".to_string();
            preferences.general.theme = "light".to_string();
            preferences.drawing.grid_spacing_mm = 3.0;
            preferences.drawing.snap_enabled = true;
            preferences.drawing.max_image_pixels = 2_000_000;
            preferences.general.backup_enabled = false;
            preferences.reset_section(section);
            assert!(
                preferences.validate().is_ok(),
                "{} left an invalid state",
                section.label()
            );
        }
    }

    #[test]
    fn reset_all_keeps_the_recent_projects() {
        let mut preferences = AppPreferences::default();
        preferences.general.recent_projects = vec!["/tmp/a.bwp".to_string()];
        preferences.general.default_unit = "in".to_string();
        preferences.drawing.grid_spacing_mm = 4.0;
        preferences.drawing.snap_nodes = true;
        preferences.drawing.max_image_pixels = 3_000_000;
        preferences.reset_section(Section::Advanced);
        let defaults = AppPreferences::default();
        assert_eq!(preferences.general.recent_projects.len(), 1);
        assert_eq!(preferences.general.recent_projects[0], "/tmp/a.bwp");
        assert_eq!(
            preferences.general.default_unit,
            defaults.general.default_unit
        );
        assert!(
            (preferences.drawing.grid_spacing_mm - defaults.drawing.grid_spacing_mm).abs() < 1e-9
        );
        assert_eq!(preferences.drawing.snap_nodes, defaults.drawing.snap_nodes);
        assert_eq!(
            preferences.drawing.max_image_pixels,
            defaults.drawing.max_image_pixels
        );
    }
}
