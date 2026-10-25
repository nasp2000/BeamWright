use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Unit {
    Millimeters,
    Centimeters,
    Inches,
    Pixels,
}

impl Unit {
    pub fn to_mm(self, value: f64) -> f64 {
        match self {
            Unit::Centimeters => value * 10.0,
            Unit::Inches => value * 25.4,
            Unit::Millimeters | Unit::Pixels => value, // Pixels requires dpi context; identity here
        }
    }
}
