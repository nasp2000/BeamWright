use super::DrawingPrefs;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralPrefs {
    pub default_unit: String,
    pub theme: String,
    pub default_file_format: String,
    pub confirm_destructive: bool,
    pub backup_enabled: bool,
    pub default_directory: String,
}

impl Default for GeneralPrefs {
    fn default() -> Self {
        Self {
            default_unit: "mm".to_string(),
            theme: "light".to_string(),
            default_file_format: "nc".to_string(),
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
        if self.default_file_format.is_empty() {
            return Err("default file format must not be empty".to_string());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppPreferences {
    pub general: GeneralPrefs,
    pub drawing: DrawingPrefs,
}

impl AppPreferences {
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
    fn round_trip_serde() {
        let p = AppPreferences::default();
        let s = serde_json::to_string(&p).unwrap();
        let q: AppPreferences = serde_json::from_str(&s).unwrap();
        assert_eq!(q.general.theme, p.general.theme);
    }
}
