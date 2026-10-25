use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Layer {
    pub id: String,
    pub name: String,
    pub color: String,
    pub visible: bool,
    pub locked: bool,
    pub order: u32,
}

impl Default for Layer {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: "Layer 1".to_string(),
            color: "#000000".to_string(),
            visible: true,
            locked: false,
            order: 0,
        }
    }
}

impl Layer {
    pub fn validate(&self) -> std::result::Result<(), String> {
        if self.name.trim().is_empty() {
            return Err("layer name must not be empty".to_string());
        }
        if self.color.is_empty() {
            return Err("layer color must not be empty".to_string());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_valid() {
        assert!(Layer::default().validate().is_ok());
    }

    #[test]
    fn reject_empty_name() {
        let l = Layer {
            name: String::new(),
            ..Layer::default()
        };
        assert!(l.validate().is_err());
    }

    #[test]
    fn reject_empty_color() {
        let l = Layer {
            color: String::new(),
            ..Layer::default()
        };
        assert!(l.validate().is_err());
    }
}
