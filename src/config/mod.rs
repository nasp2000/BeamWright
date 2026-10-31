pub mod paths;
pub mod preferences;
#[derive(Debug, Clone)]
pub struct AppConfig {
    pub default_unit: &'static str,
    pub theme: &'static str,
    pub backup_enabled: bool,
    pub default_dir: Option<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            default_unit: "mm",
            theme: "dark",
            backup_enabled: true,
            default_dir: None,
        }
    }
}

impl AppConfig {
    pub fn validate(&self) -> std::result::Result<(), String> {
        if self.default_unit.is_empty() {
            return Err("default unit must not be empty".to_string());
        }
        if self.theme.is_empty() {
            return Err("theme must not be empty".to_string());
        }
        Ok(())
    }
}

// Snapping targets are independent toggles, so the booleans are not a sign of
// a missing state machine.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DrawingPrefs {
    pub grid_spacing_mm: f64,
    pub snap_tolerance_px: f64,
    pub show_rulers: bool,
    #[serde(default = "default_snap_step")]
    pub snap_step_mm: f64,
    #[serde(default = "default_true")]
    pub snap_enabled: bool,
    /// Snap drawing to path nodes.
    #[serde(default)]
    pub snap_nodes: bool,
    /// Snap drawing to the ends of open paths.
    #[serde(default)]
    pub snap_endpoints: bool,
    /// Snap drawing to object centres.
    #[serde(default)]
    pub snap_centers: bool,
    /// Snap drawing to segment midpoints.
    #[serde(default)]
    pub snap_midpoints: bool,
    /// Snap drawing to crossings between object outlines.
    #[serde(default)]
    pub snap_intersections: bool,
    /// Snap drawing to object bounding-box corners and edges.
    #[serde(default)]
    pub snap_limits: bool,
}

impl Default for DrawingPrefs {
    fn default() -> Self {
        Self {
            grid_spacing_mm: 10.0,
            snap_tolerance_px: 8.0,
            show_rulers: true,
            snap_step_mm: 1.0,
            snap_enabled: false,
            snap_nodes: false,
            snap_endpoints: false,
            snap_centers: false,
            snap_midpoints: false,
            snap_intersections: false,
            snap_limits: false,
        }
    }
}

impl DrawingPrefs {
    pub fn validate(&self) -> std::result::Result<(), String> {
        if self.grid_spacing_mm <= 0.0 {
            return Err("grid spacing must be positive".to_string());
        }
        if self.snap_tolerance_px < 0.0 {
            return Err("snap tolerance must be non-negative".to_string());
        }
        if !self.snap_step_mm.is_finite() || self.snap_step_mm <= 0.0 {
            return Err("snap step must be positive".to_string());
        }
        Ok(())
    }
}

fn default_snap_step() -> f64 {
    1.0
}

fn default_true() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::AppConfig;

    #[test]
    fn default_valid() {
        assert!(AppConfig::default().validate().is_ok());
    }

    #[test]
    fn reject_empty_unit() {
        let c = AppConfig {
            default_unit: "",
            ..AppConfig::default()
        };
        assert!(c.validate().is_err());
    }

    #[test]
    fn reject_empty_theme() {
        let c = AppConfig {
            theme: "",
            ..AppConfig::default()
        };
        assert!(c.validate().is_err());
    }
}

#[cfg(test)]
mod drawing_tests {
    use super::DrawingPrefs;

    #[test]
    fn default_valid() {
        assert!(DrawingPrefs::default().validate().is_ok());
    }

    #[test]
    fn reject_bad_spacing() {
        let p = DrawingPrefs {
            grid_spacing_mm: 0.0,
            ..DrawingPrefs::default()
        };
        assert!(p.validate().is_err());
        let p = DrawingPrefs {
            grid_spacing_mm: -1.0,
            ..DrawingPrefs::default()
        };
        assert!(p.validate().is_err());
    }

    #[test]
    fn reject_negative_tolerance() {
        let p = DrawingPrefs {
            snap_tolerance_px: -1.0,
            ..DrawingPrefs::default()
        };
        assert!(p.validate().is_err());
    }

    #[test]
    fn reject_bad_snap_step() {
        let p = DrawingPrefs {
            snap_step_mm: 0.0,
            ..DrawingPrefs::default()
        };
        assert!(p.validate().is_err());
    }
}
