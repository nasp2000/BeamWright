use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::color::{normalize_hex_color, parse_hex_color};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Layer {
    pub id: String,
    pub name: String,
    pub color: String,
    pub visible: bool,
    pub locked: bool,
    pub order: u32,
}

impl Layer {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: name.into(),
            color: "#000000".to_string(),
            visible: true,
            locked: false,
            order: 0,
        }
    }

    pub fn with_color(mut self, color: &str) -> Self {
        if normalize_hex_color(color).is_some() {
            self.color = color.to_string();
        }
        self
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.id.trim().is_empty() {
            return Err("layer id must not be empty".to_string());
        }
        if self.name.trim().is_empty() {
            return Err("layer name must not be empty".to_string());
        }
        if normalize_hex_color(&self.color).is_none() {
            return Err(format!("invalid layer colour: {}", self.color));
        }
        Ok(())
    }

    pub fn rgb(&self) -> (u8, u8, u8) {
        parse_hex_color(&self.color).unwrap_or((0, 0, 0))
    }
}

impl Default for Layer {
    fn default() -> Self {
        Self::new("Layer 1")
    }
}

#[cfg(test)]
#[allow(clippy::assert_is_empty)]
mod tests {
    use super::*;

    #[test]
    fn default_valid() {
        let layer = Layer::default();
        assert!(layer.validate().is_ok());
        assert!(!layer.id.is_empty());
    }

    #[test]
    fn reject_empty_name() {
        let layer = Layer {
            name: String::new(),
            ..Layer::default()
        };
        assert!(layer.validate().is_err());
    }

    #[test]
    fn reject_invalid_color() {
        let layer = Layer {
            color: "purple-ish".to_string(),
            ..Layer::default()
        };
        assert!(layer.validate().is_err());
    }

    #[test]
    fn with_color_normalizes_check() {
        let layer = Layer::new("Cut").with_color("#ff0000");
        assert_eq!(layer.rgb(), (255, 0, 0));
    }

    #[test]
    fn unique_ids() {
        assert_ne!(Layer::new("A").id, Layer::new("B").id);
    }
}
