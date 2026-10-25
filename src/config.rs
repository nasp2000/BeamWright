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
