use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    /// Approximate bounding box in local coordinates.
    pub fn bounds(&self) -> (crate::geometry::Point2, crate::geometry::Point2) {
        (
            crate::geometry::Point2::new(self.x, self.y),
            crate::geometry::Point2::new(self.x + self.approx_width(), self.y + self.size),
        )
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

    #[test]
    fn bounds() {
        let t = TextObject {
            content: "AB".to_string(),
            font: "sans".to_string(),
            size: 10.0,
            x: 2.0,
            y: 3.0,
        };
        let (min, max) = t.bounds();
        assert!((min.x - 2.0).abs() < 1e-9);
        assert!((min.y - 3.0).abs() < 1e-9);
        assert!(max.x > min.x);
        assert!((max.y - 13.0).abs() < 1e-9);
    }

    #[test]
    fn empty_text_has_zero_width() {
        let t = TextObject {
            content: String::new(),
            font: "sans".to_string(),
            size: 10.0,
            x: 0.0,
            y: 0.0,
        };
        assert!((t.approx_width()).abs() < 1e-9);
    }
}
