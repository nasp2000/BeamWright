use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextObject {
    pub content: String,
    pub font: String,
    pub size: f64,
    pub x: f64,
    pub y: f64,
}

impl TextObject {
    pub fn validate(&self) -> std::result::Result<(), String> {
        if self.content.is_empty() {
            return Err("text content must not be empty".to_string());
        }
        if self.size <= 0.0 {
            return Err("font size must be positive".to_string());
        }
        Ok(())
    }
    /// Rough width estimate (monospace approximation).
    #[allow(clippy::cast_precision_loss)]
    pub fn approx_width(&self) -> f64 {
        self.content.chars().count() as f64 * self.size * 0.5
    }
}

#[cfg(test)]
mod tests {
    use super::TextObject;

    #[test]
    fn valid() {
        let t = TextObject {
            content: "hi".to_string(),
            font: "sans".to_string(),
            size: 12.0,
            x: 0.0,
            y: 0.0,
        };
        assert!(t.validate().is_ok());
    }

    #[test]
    fn reject_empty_content() {
        let t = TextObject {
            content: String::new(),
            font: "sans".to_string(),
            size: 12.0,
            x: 0.0,
            y: 0.0,
        };
        assert!(t.validate().is_err());
    }

    #[test]
    fn reject_bad_size() {
        let t = TextObject {
            content: "hi".to_string(),
            font: "sans".to_string(),
            size: 0.0,
            x: 0.0,
            y: 0.0,
        };
        assert!(t.validate().is_err());
    }
}
