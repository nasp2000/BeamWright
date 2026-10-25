use serde::{Deserialize, Serialize};

use super::units::Unit;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub name: String,
    pub unit: Unit,
    pub width_mm: f64,
    pub height_mm: f64,
    pub format_version: u32,
}

impl Default for Document {
    fn default() -> Self {
        Self {
            name: "Untitled".to_string(),
            unit: Unit::Millimeters,
            width_mm: 300.0,
            height_mm: 200.0,
            format_version: 1,
        }
    }
}
