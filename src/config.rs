/// Application-level configuration (paths, preferences). Stub for Phase 1.
pub struct AppConfig {
    pub default_unit: &'static str,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self { default_unit: "mm" }
    }
}
