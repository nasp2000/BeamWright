/// G-code dialect written by the job.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Dialect {
    #[default]
    Grbl,
    Marlin,
    Smoothie,
    LinuxCnc,
}

impl Dialect {
    pub fn label(self) -> &'static str {
        match self {
            Dialect::Grbl => "GRBL",
            Dialect::Marlin => "Marlin",
            Dialect::Smoothie => "SmoothieWare",
            Dialect::LinuxCnc => "LinuxCNC",
        }
    }

    pub fn all() -> [Dialect; 4] {
        [
            Dialect::Grbl,
            Dialect::Marlin,
            Dialect::Smoothie,
            Dialect::LinuxCnc,
        ]
    }

    /// Stable identifier used in saved settings.
    pub fn id(self) -> &'static str {
        match self {
            Dialect::Grbl => "grbl",
            Dialect::Marlin => "marlin",
            Dialect::Smoothie => "smoothie",
            Dialect::LinuxCnc => "linuxcnc",
        }
    }

    pub fn from_id(id: &str) -> Option<Dialect> {
        Dialect::all()
            .into_iter()
            .find(|dialect| dialect.id() == id)
    }

    /// Only GRBL distinguishes dynamic (`M4`) from constant (`M3`) power.
    pub fn supports_dynamic_power(self) -> bool {
        self == Dialect::Grbl
    }

    pub fn laser_command(self, constant: bool) -> &'static str {
        if self.supports_dynamic_power() && !constant {
            "M4"
        } else {
            "M3"
        }
    }
}

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
    pub dialect: Dialect,
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
            dialect: Dialect::Grbl,
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
