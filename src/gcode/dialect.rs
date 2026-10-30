/// Laser power reporting mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LaserMode {
    /// `M3`: constant power while moving.
    ConstantPower,
    /// `M4`: power follows the commanded feed rate.
    #[default]
    DynamicPower,
}

impl LaserMode {
    pub fn command(self) -> &'static str {
        match self {
            LaserMode::ConstantPower => "M3",
            LaserMode::DynamicPower => "M4",
        }
    }
}

/// Options for writing a GRBL-compatible job file.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GCodeOptions {
    pub laser_mode: LaserMode,
    /// Number of decimals for coordinates.
    pub decimals: u8,
    pub line_ending_crlf: bool,
    pub include_comments: bool,
    pub include_travel: bool,
}

impl Default for GCodeOptions {
    fn default() -> Self {
        Self {
            laser_mode: LaserMode::DynamicPower,
            decimals: 3,
            line_ending_crlf: false,
            include_comments: true,
            include_travel: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_commands() {
        assert_eq!(LaserMode::ConstantPower.command(), "M3");
        assert_eq!(LaserMode::DynamicPower.command(), "M4");
        assert_eq!(LaserMode::default(), LaserMode::DynamicPower);
    }

    #[test]
    fn defaults_are_metric_absolute() {
        let options = GCodeOptions::default();
        assert_eq!(options.decimals, 3);
        assert!(!options.line_ending_crlf);
        assert!(options.include_comments);
    }
}
