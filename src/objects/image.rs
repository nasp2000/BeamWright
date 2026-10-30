use serde::{Deserialize, Serialize};

/// How a raster image is converted to burn/no-burn dots.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ImageMode {
    /// Burn pixels darker than the threshold.
    #[default]
    Threshold,
    /// Floyd–Steinberg error diffusion.
    FloydSteinberg,
    /// Ordered dithering with an 8×8 matrix.
    OrderedDither,
}

impl ImageMode {
    pub fn label(self) -> &'static str {
        match self {
            ImageMode::Threshold => "Threshold",
            ImageMode::FloydSteinberg => "Floyd–Steinberg",
            ImageMode::OrderedDither => "Ordered dither",
        }
    }

    pub fn all() -> [ImageMode; 3] {
        [
            ImageMode::Threshold,
            ImageMode::FloydSteinberg,
            ImageMode::OrderedDither,
        ]
    }
}

/// Non-destructive preparation settings stored with the image object.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ImageSettings {
    pub mode: ImageMode,
    /// Pixels whose adjusted value is below this are burned (or left unburned
    /// when `invert` is set).
    pub threshold: u8,
    /// Brightness offset, -100..=100.
    pub brightness: i16,
    /// Contrast multiplier around mid-grey, 0.25..=3.0.
    pub contrast: f32,
    /// Gamma exponent, 0.2..=3.0.
    pub gamma: f32,
    pub invert: bool,
    /// Distance between raster lines in the job, in millimetres.
    pub line_interval_mm: f64,
}

impl Default for ImageSettings {
    fn default() -> Self {
        Self {
            mode: ImageMode::Threshold,
            threshold: 128,
            brightness: 0,
            contrast: 1.0,
            gamma: 1.0,
            invert: false,
            line_interval_mm: 0.1,
        }
    }
}

impl ImageSettings {
    pub fn validate(&self) -> Result<(), String> {
        if !(-100..=100).contains(&self.brightness) {
            return Err("brightness must be between -100 and 100".to_string());
        }
        if !self.contrast.is_finite() || !(0.25..=3.0).contains(&self.contrast) {
            return Err("contrast must be between 0.25 and 3.0".to_string());
        }
        if !self.gamma.is_finite() || !(0.2..=3.0).contains(&self.gamma) {
            return Err("gamma must be between 0.2 and 3.0".to_string());
        }
        if !self.line_interval_mm.is_finite() || !(0.01..=2.0).contains(&self.line_interval_mm) {
            return Err("line interval must be between 0.01 and 2 mm".to_string());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImageObject {
    pub path: String,
    pub width_px: u32,
    pub height_px: u32,
    pub dpi: f64,
    #[serde(default)]
    pub settings: ImageSettings,
}

impl ImageObject {
    pub fn new(path: impl Into<String>, width_px: u32, height_px: u32, dpi: f64) -> Self {
        Self {
            path: path.into(),
            width_px,
            height_px,
            dpi,
            settings: ImageSettings::default(),
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.path.is_empty() {
            return Err("image path must not be empty".to_string());
        }
        if self.width_px == 0 || self.height_px == 0 {
            return Err("image dimensions must be positive".to_string());
        }
        if !self.dpi.is_finite() || self.dpi <= 0.0 {
            return Err("dpi must be positive".to_string());
        }
        self.settings.validate()?;
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
    use super::*;

    #[test]
    fn valid_image() {
        let image = ImageObject::new("img.png", 100, 50, 96.0);
        assert!(image.validate().is_ok());
        assert_eq!(image.settings, ImageSettings::default());
    }

    #[test]
    fn reject_empty_path() {
        let mut image = ImageObject::new("img.png", 100, 50, 96.0);
        image.path.clear();
        assert!(image.validate().is_err());
    }

    #[test]
    fn reject_zero_dimensions_and_dpi() {
        let mut image = ImageObject::new("img.png", 0, 50, 96.0);
        assert!(image.validate().is_err());
        image.width_px = 100;
        image.dpi = 0.0;
        assert!(image.validate().is_err());
    }

    #[test]
    fn settings_ranges_are_validated() {
        let mut settings = ImageSettings::default();
        assert!(settings.validate().is_ok());
        settings.brightness = 101;
        assert!(settings.validate().is_err());
        settings.brightness = 0;
        settings.contrast = 0.0;
        assert!(settings.validate().is_err());
        settings.contrast = 1.0;
        settings.gamma = 0.0;
        assert!(settings.validate().is_err());
        settings.gamma = 1.0;
        settings.line_interval_mm = 0.0;
        assert!(settings.validate().is_err());
    }

    #[test]
    fn physical_size_uses_dpi() {
        let image = ImageObject::new("img.png", 96, 48, 96.0);
        assert!((image.width_mm() - 25.4).abs() < 1e-9);
        assert!((image.height_mm() - 12.7).abs() < 1e-9);
    }

    #[test]
    fn modes_have_unique_labels() {
        let labels: Vec<&str> = ImageMode::all().iter().map(|mode| mode.label()).collect();
        let mut sorted = labels.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), labels.len());
    }
}
