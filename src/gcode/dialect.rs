#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaserMode {
    ConstantPower, // M3
    DynamicPower,  // M4
}

impl LaserMode {
    pub fn command(self) -> &'static str {
        match self {
            LaserMode::ConstantPower => "M3",
            LaserMode::DynamicPower => "M4",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct GCodeOptions {
    pub laser_mode: LaserMode,
    pub s_range: (u32, u32),
    pub decimals: u8,
    pub line_ending_crlf: bool,
    pub include_comments: bool,
    pub include_travel: bool,
    pub dwell_ms: u32,
}

impl Default for GCodeOptions {
    fn default() -> Self {
        Self {
            laser_mode: LaserMode::DynamicPower,
            s_range: (0, 1000),
            decimals: 3,
            line_ending_crlf: false,
            include_comments: true,
            include_travel: true,
            dwell_ms: 0,
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
    }

    #[test]
    fn default_options() {
        let o = GCodeOptions::default();
        assert_eq!(o.s_range, (0, 1000));
        assert_eq!(o.decimals, 3);
        assert!(!o.line_ending_crlf);
    }

    #[test]
    fn equality() {
        assert_eq!(LaserMode::DynamicPower, LaserMode::DynamicPower);
        assert_ne!(LaserMode::ConstantPower, LaserMode::DynamicPower);
    }
}
