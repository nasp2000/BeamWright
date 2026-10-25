#[derive(Debug, Clone, Copy)]
#[allow(clippy::struct_excessive_bools)]
pub struct SnapSettings {
    pub grid: bool,
    pub nodes: bool,
    pub endpoints: bool,
    pub centers: bool,
    pub tolerance_px: f64,
}

impl Default for SnapSettings {
    fn default() -> Self {
        Self {
            grid: true,
            nodes: true,
            endpoints: true,
            centers: true,
            tolerance_px: 8.0,
        }
    }
}

impl SnapSettings {
    pub fn any_enabled(&self) -> bool {
        self.grid || self.nodes || self.endpoints || self.centers
    }
}

#[cfg(test)]
mod tests {
    use super::SnapSettings;

    #[test]
    fn default_enabled() {
        let s = SnapSettings::default();
        assert!(s.any_enabled());
        assert!(s.grid);
    }

    #[test]
    fn all_disabled() {
        let s = SnapSettings {
            grid: false,
            nodes: false,
            endpoints: false,
            centers: false,
            tolerance_px: 8.0,
        };
        assert!(!s.any_enabled());
    }

    #[test]
    fn tolerance() {
        let s = SnapSettings::default();
        assert!(s.tolerance_px > 0.0);
    }
}
