use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImageObject {
    pub path: String,
    pub width_px: u32,
    pub height_px: u32,
    pub dpi: f64,
}

impl ImageObject {
    pub fn validate(&self) -> std::result::Result<(), String> {
        if self.path.is_empty() {
            return Err("image path must not be empty".to_string());
        }
        if self.width_px == 0 || self.height_px == 0 {
            return Err("image dimensions must be positive".to_string());
        }
        if self.dpi <= 0.0 {
            return Err("dpi must be positive".to_string());
        }
        Ok(())
    }
    pub fn width_mm(&self) -> f64 {
        f64::from(self.width_px) * 25.4 / self.dpi
    }
    pub fn height_mm(&self) -> f64 {
        f64::from(self.height_px) * 25.4 / self.dpi
    }
    pub fn bounds(&self) -> (crate::geometry::Point2, crate::geometry::Point2) {
        (
            crate::geometry::Point2::origin(),
            crate::geometry::Point2::new(self.width_mm(), self.height_mm()),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::ImageObject;

    #[test]
    fn valid() {
        let i = ImageObject {
            path: "img.png".to_string(),
            width_px: 100,
            height_px: 50,
            dpi: 96.0,
        };
        assert!(i.validate().is_ok());
    }

    #[test]
    fn reject_empty_path() {
        let i = ImageObject {
            path: String::new(),
            width_px: 100,
            height_px: 50,
            dpi: 96.0,
        };
        assert!(i.validate().is_err());
    }

    #[test]
    fn reject_zero_dims() {
        let i = ImageObject {
            path: "img.png".to_string(),
            width_px: 0,
            height_px: 50,
            dpi: 96.0,
        };
        assert!(i.validate().is_err());
    }

    #[test]
    fn width_mm() {
        let i = ImageObject {
            path: "img.png".to_string(),
            width_px: 96,
            height_px: 96,
            dpi: 96.0,
        };
        assert!((i.width_mm() - 25.4).abs() < 1e-9);
    }
}
