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
    /// Atkinson error diffusion, a lighter dither that keeps highlights clean.
    Atkinson,
    /// Stucki error diffusion, a finer dither than Floyd–Steinberg.
    Stucki,
    /// Jarvis–Judice–Ninke error diffusion with a wide kernel.
    Jarvis,
    /// Clustered-dot halftone with a 4×4 screen.
    Halftone,
    /// Clustered-dot halftone rotated 45°, for a newsprint look.
    Newsprint,
    /// Edge detection: burns the outlines of the image.
    Sketch,
}

impl ImageMode {
    pub fn label(self) -> &'static str {
        match self {
            ImageMode::Threshold => "Threshold",
            ImageMode::FloydSteinberg => "Floyd–Steinberg",
            ImageMode::OrderedDither => "Ordered dither",
            ImageMode::Atkinson => "Atkinson",
            ImageMode::Stucki => "Stucki",
            ImageMode::Jarvis => "Jarvis–Judice–Ninke",
            ImageMode::Halftone => "Halftone dots",
            ImageMode::Newsprint => "Newsprint",
            ImageMode::Sketch => "Sketch (edges)",
        }
    }

    pub fn all() -> [ImageMode; 9] {
        [
            ImageMode::Threshold,
            ImageMode::FloydSteinberg,
            ImageMode::OrderedDither,
            ImageMode::Atkinson,
            ImageMode::Stucki,
            ImageMode::Jarvis,
            ImageMode::Halftone,
            ImageMode::Newsprint,
            ImageMode::Sketch,
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
    /// Clockwise rotation applied to the processed image: 0, 90, 180 or 270.
    #[serde(default)]
    pub rotation_degrees: f64,
    /// Working width in pixels; 0 keeps the source resolution.
    #[serde(default)]
    pub target_width_px: u32,
    /// Working height in pixels; 0 keeps the source resolution.
    #[serde(default)]
    pub target_height_px: u32,
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
            rotation_degrees: 0.0,
            target_width_px: 0,
            target_height_px: 0,
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
        if ![0.0, 90.0, 180.0, 270.0].contains(&self.rotation_degrees) {
            return Err("image rotation must be 0, 90, 180 or 270 degrees".to_string());
        }
        let has_width = self.target_width_px > 0;
        let has_height = self.target_height_px > 0;
        if has_width != has_height {
            return Err("working size needs both width and height".to_string());
        }
        Ok(())
    }
}

/// A rectangular region of the source image, in source pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageCrop {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImageObject {
    pub path: String,
    pub width_px: u32,
    pub height_px: u32,
    pub dpi: f64,
    #[serde(default)]
    pub settings: ImageSettings,
    /// Optional crop of the source image, applied before rotation.
    #[serde(default)]
    pub crop: Option<ImageCrop>,
}

impl ImageObject {
    pub fn new(path: impl Into<String>, width_px: u32, height_px: u32, dpi: f64) -> Self {
        Self {
            path: path.into(),
            width_px,
            height_px,
            dpi,
            settings: ImageSettings::default(),
            crop: None,
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
        if let Some(crop) = self.crop {
            if crop.width == 0 || crop.height == 0 {
                return Err("image crop must have a positive size".to_string());
            }
            if crop.x >= self.width_px || crop.y >= self.height_px {
                return Err("image crop starts outside the source".to_string());
            }
            if crop.width > self.width_px - crop.x || crop.height > self.height_px - crop.y {
                return Err("image crop extends past the source".to_string());
            }
        }
        Ok(())
    }

    /// Size in pixels after cropping.
    pub fn cropped_size_px(&self) -> (u32, u32) {
        match self.crop {
            Some(crop) => (crop.width, crop.height),
            None => (self.width_px, self.height_px),
        }
    }

    /// Size in pixels after cropping and rotation.
    pub fn rotated_size_px(&self) -> (u32, u32) {
        let (width, height) = self.cropped_size_px();
        let rotation = self.settings.rotation_degrees;
        let quarter_turn = (rotation - 90.0).abs() < 0.5 || (rotation - 270.0).abs() < 0.5;
        if quarter_turn {
            (height, width)
        } else {
            (width, height)
        }
    }

    /// Working size in pixels, after crop, rotation and optional resampling.
    pub fn effective_size_px(&self) -> (u32, u32) {
        if self.settings.target_width_px > 0 {
            (
                self.settings.target_width_px,
                self.settings.target_height_px,
            )
        } else {
            self.rotated_size_px()
        }
    }

    /// Physical width after crop, rotation and resampling.
    pub fn width_mm(&self) -> f64 {
        f64::from(self.effective_size_px().0) * 25.4 / self.dpi
    }

    /// Physical height after crop, rotation and resampling.
    pub fn height_mm(&self) -> f64 {
        f64::from(self.effective_size_px().1) * 25.4 / self.dpi
    }

    /// Effective resolution of the working image, in dots per inch.
    pub fn effective_dpi(&self) -> f64 {
        self.dpi
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
    fn crop_rotation_and_working_size_change_the_physical_size() {
        let mut image = ImageObject::new("img.png", 400, 200, 100.0);
        assert!((image.width_mm() - 101.6).abs() < 1e-9);
        image.crop = Some(ImageCrop {
            x: 100,
            y: 50,
            width: 200,
            height: 100,
        });
        assert_eq!(image.cropped_size_px(), (200, 100));
        assert!((image.width_mm() - 50.8).abs() < 1e-9);
        image.settings.rotation_degrees = 90.0;
        assert_eq!(image.effective_size_px(), (100, 200));
        image.settings.target_width_px = 50;
        image.settings.target_height_px = 25;
        assert_eq!(image.effective_size_px(), (50, 25));
        assert!((image.width_mm() - 12.7).abs() < 1e-9);
        assert!(image.validate().is_ok());
    }

    #[test]
    fn crop_and_rotation_are_validated() {
        let mut image = ImageObject::new("img.png", 100, 50, 96.0);
        image.crop = Some(ImageCrop {
            x: 0,
            y: 0,
            width: 0,
            height: 10,
        });
        assert!(image.validate().is_err());
        image.crop = Some(ImageCrop {
            x: 90,
            y: 0,
            width: 20,
            height: 10,
        });
        assert!(image.validate().is_err());
        image.crop = Some(ImageCrop {
            x: 10,
            y: 10,
            width: 20,
            height: 20,
        });
        assert!(image.validate().is_ok());
        image.settings.rotation_degrees = 45.0;
        assert!(image.validate().is_err());
        image.settings.rotation_degrees = 180.0;
        image.settings.target_width_px = 10;
        image.settings.target_height_px = 0;
        assert!(image.validate().is_err());
    }

    #[test]
    fn old_images_without_crop_still_load() {
        let old = r#"{"path":"a.png","width_px":10,"height_px":5,"dpi":96.0}"#;
        let image: ImageObject = serde_json::from_str(old).unwrap();
        assert!(image.crop.is_none());
        assert_eq!(image.settings.rotation_degrees, 0.0);
        assert_eq!(image.effective_size_px(), (10, 5));
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
