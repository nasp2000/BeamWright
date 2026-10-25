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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DrawingPrefs {
    pub grid_spacing_mm: f64,
    pub snap_tolerance_px: f64,
    pub show_rulers: bool,
}

impl Default for DrawingPrefs {
    fn default() -> Self {
        Self {
            grid_spacing_mm: 10.0,
            snap_tolerance_px: 8.0,
            show_rulers: true,
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
        Ok(())
    }
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
}
