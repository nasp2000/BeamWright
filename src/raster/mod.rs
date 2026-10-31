use crate::errors::{BeamError, Result};
use crate::objects::{ImageMode, ImageSettings};

/// Maximum number of pixels accepted from an imported image.
pub const MAX_PIXELS: u64 = 50_000_000;

/// Burn/no-burn bitmap produced from an image and its settings.
#[derive(Debug, Clone, PartialEq)]
pub struct RasterBitmap {
    pub width: usize,
    pub height: usize,
    /// `true` where the laser burns.
    pub burn: Vec<bool>,
}

impl RasterBitmap {
    pub fn burned(&self) -> usize {
        self.burn.iter().filter(|value| **value).count()
    }

    pub fn get(&self, x: usize, y: usize) -> bool {
        self.burn[y * self.width + x]
    }
}

/// Convert a luminance buffer using the image settings.
///
/// Pipeline: brightness, contrast, gamma, invert, then threshold or dithering.
pub fn process_luma(
    luma: &[u8],
    width: usize,
    height: usize,
    settings: &ImageSettings,
) -> Result<RasterBitmap> {
    if width == 0 || height == 0 {
        return Err(BeamError::InvalidInput("image has no pixels".to_string()));
    }
    if luma.len() != width * height {
        return Err(BeamError::InvalidInput(
            "image buffer does not match its dimensions".to_string(),
        ));
    }
    settings.validate().map_err(BeamError::InvalidInput)?;

    let adjusted: Vec<f32> = luma
        .iter()
        .map(|value| adjust(f32::from(*value), settings))
        .collect();

    let mut burn = vec![false; adjusted.len()];
    match settings.mode {
        ImageMode::Threshold => {
            for (index, value) in adjusted.iter().enumerate() {
                burn[index] = *value < f32::from(settings.threshold);
            }
        }
        ImageMode::FloydSteinberg => {
            let mut values = adjusted.clone();
            for y in 0..height {
                for x in 0..width {
                    let index = y * width + x;
                    let old = values[index];
                    let new = if old < 128.0 { 0.0 } else { 255.0 };
                    burn[index] = new == 0.0;
                    let error = old - new;
                    if x + 1 < width {
                        values[index + 1] += error * 7.0 / 16.0;
                    }
                    if y + 1 < height {
                        let below = index + width;
                        if x > 0 {
                            values[below - 1] += error * 3.0 / 16.0;
                        }
                        values[below] += error * 5.0 / 16.0;
                        if x + 1 < width {
                            values[below + 1] += error * 1.0 / 16.0;
                        }
                    }
                }
            }
        }
        ImageMode::OrderedDither => {
            const MATRIX: [[u8; 8]; 8] = [
                [0, 32, 8, 40, 2, 34, 10, 42],
                [48, 16, 56, 24, 50, 18, 58, 26],
                [12, 44, 4, 36, 14, 46, 6, 38],
                [60, 28, 52, 20, 62, 30, 54, 22],
                [3, 35, 11, 43, 1, 33, 9, 41],
                [51, 19, 59, 27, 49, 17, 57, 25],
                [15, 47, 7, 39, 13, 45, 5, 37],
                [63, 31, 55, 23, 61, 29, 53, 21],
            ];
            for y in 0..height {
                for x in 0..width {
                    let index = y * width + x;
                    let limit = (f32::from(MATRIX[y % 8][x % 8]) + 0.5) * 255.0 / 64.0;
                    burn[index] = adjusted[index] < limit;
                }
            }
        }
    }
    Ok(RasterBitmap {
        width,
        height,
        burn,
    })
}

fn adjust(value: f32, settings: &ImageSettings) -> f32 {
    let mut value = value + f32::from(settings.brightness) * 2.55;
    value = (value - 128.0) * settings.contrast + 128.0;
    value = 255.0 * (value.clamp(0.0, 255.0) / 255.0).powf(1.0 / settings.gamma);
    if settings.invert {
        value = 255.0 - value;
    }
    value.clamp(0.0, 255.0)
}

/// Load an image and apply the object's crop, rotation and working size.
///
/// The pipeline is non-destructive: the file is never written. Crop and rotation
/// happen before the tone and dithering settings, and resampling uses nearest
/// neighbour so a working resolution never invents grey detail.
pub fn process_object(
    path: &std::path::Path,
    image: &crate::objects::ImageObject,
) -> Result<RasterBitmap> {
    image.validate().map_err(BeamError::InvalidInput)?;
    let mut picture = image::open(path)
        .map_err(|error| BeamError::InvalidInput(format!("cannot read image: {error}")))?;
    if let Some(crop) = image.crop {
        picture = picture.crop_imm(crop.x, crop.y, crop.width, crop.height);
    }
    let rotation = image.settings.rotation_degrees;
    picture = if (rotation - 90.0).abs() < 0.5 {
        picture.rotate90()
    } else if (rotation - 180.0).abs() < 0.5 {
        picture.rotate180()
    } else if (rotation - 270.0).abs() < 0.5 {
        picture.rotate270()
    } else {
        picture
    };
    if image.settings.target_width_px > 0 && image.settings.target_height_px > 0 {
        picture = picture.resize_exact(
            image.settings.target_width_px,
            image.settings.target_height_px,
            image::imageops::FilterType::Nearest,
        );
    }
    let pixels = u64::from(picture.width()) * u64::from(picture.height());
    if pixels > MAX_PIXELS {
        return Err(BeamError::InvalidInput(format!(
            "image has {pixels} pixels; the limit is {MAX_PIXELS}"
        )));
    }
    let luma = picture.to_luma8();
    let width = usize::try_from(luma.width()).unwrap_or(0);
    let height = usize::try_from(luma.height()).unwrap_or(0);
    process_luma(luma.as_raw(), width, height, &image.settings)
}

/// Load an image file and produce its burn bitmap.
pub fn process_file(path: &std::path::Path, settings: &ImageSettings) -> Result<RasterBitmap> {
    let image = image::open(path)
        .map_err(|error| BeamError::InvalidInput(format!("cannot read image: {error}")))?;
    let pixels = u64::from(image.width()) * u64::from(image.height());
    if pixels > MAX_PIXELS {
        return Err(BeamError::InvalidInput(format!(
            "image has {pixels} pixels; the limit is {MAX_PIXELS}"
        )));
    }
    let luma = image.to_luma8();
    process_luma(
        luma.as_raw(),
        luma.width() as usize,
        luma.height() as usize,
        settings,
    )
}

#[cfg(test)]
mod object_tests {
    use super::*;
    use crate::objects::{ImageCrop, ImageObject};

    fn write_image(name: &str, width: u32, height: u32) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(name);
        let mut picture = image::GrayImage::new(width, height);
        for (x, _y, pixel) in picture.enumerate_pixels_mut() {
            *pixel = if x < width / 2 {
                image::Luma([0])
            } else {
                image::Luma([255])
            };
        }
        picture.save(&path).unwrap();
        path
    }

    #[test]
    fn crop_and_rotation_change_the_bitmap_dimensions() {
        let path = write_image("bw_raster_transform.png", 8, 4);
        let mut object = ImageObject::new(path.to_string_lossy(), 8, 4, 96.0);
        object.crop = Some(ImageCrop {
            x: 2,
            y: 1,
            width: 4,
            height: 2,
        });
        let bitmap = process_object(&path, &object).unwrap();
        assert_eq!((bitmap.width, bitmap.height), (4, 2));
        object.settings.rotation_degrees = 90.0;
        let bitmap = process_object(&path, &object).unwrap();
        assert_eq!((bitmap.width, bitmap.height), (2, 4));
        object.settings.target_width_px = 1;
        object.settings.target_height_px = 2;
        let bitmap = process_object(&path, &object).unwrap();
        assert_eq!((bitmap.width, bitmap.height), (1, 2));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn invalid_crop_is_rejected_before_processing() {
        let path = write_image("bw_raster_bad_crop.png", 4, 4);
        let mut object = ImageObject::new(path.to_string_lossy(), 4, 4, 96.0);
        object.crop = Some(ImageCrop {
            x: 3,
            y: 3,
            width: 4,
            height: 4,
        });
        assert!(process_object(&path, &object).is_err());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn crop_keeps_the_burned_pixels_of_the_region() {
        let path = write_image("bw_raster_crop_pixels.png", 8, 4);
        let mut object = ImageObject::new(path.to_string_lossy(), 8, 4, 96.0);
        object.crop = Some(ImageCrop {
            x: 0,
            y: 0,
            width: 4,
            height: 4,
        });
        let bitmap = process_object(&path, &object).unwrap();
        // The left half is black, so every cropped pixel burns.
        assert!(bitmap.burn.iter().all(|value| *value));
        let _ = std::fs::remove_file(&path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(mode: ImageMode) -> ImageSettings {
        ImageSettings {
            mode,
            ..ImageSettings::default()
        }
    }

    #[test]
    fn threshold_splits_dark_and_light() {
        let luma = [0u8, 100, 200, 255];
        let bitmap = process_luma(&luma, 2, 2, &settings(ImageMode::Threshold)).unwrap();
        assert!(bitmap.get(0, 0));
        assert!(bitmap.get(1, 0));
        assert!(!bitmap.get(0, 1));
        assert!(!bitmap.get(1, 1));
        assert_eq!(bitmap.burned(), 2);
    }

    #[test]
    fn invert_flips_the_result() {
        let luma = [0u8, 255];
        let mut config = settings(ImageMode::Threshold);
        config.invert = true;
        let bitmap = process_luma(&luma, 2, 1, &config).unwrap();
        assert!(!bitmap.get(0, 0));
        assert!(bitmap.get(1, 0));
    }

    #[test]
    fn brightness_moves_values_across_the_threshold() {
        let luma = [120u8];
        let mut config = settings(ImageMode::Threshold);
        config.threshold = 128;
        assert!(process_luma(&luma, 1, 1, &config).unwrap().get(0, 0));
        config.brightness = 20;
        assert!(!process_luma(&luma, 1, 1, &config).unwrap().get(0, 0));
    }

    #[test]
    fn floyd_steinberg_preserves_mid_grey_ratio() {
        let luma = [128u8; 16 * 16];
        let bitmap = process_luma(&luma, 16, 16, &settings(ImageMode::FloydSteinberg)).unwrap();
        let burned = bitmap.burned();
        assert!(burned > 80, "too few burned pixels: {burned}");
        assert!(burned < 176, "too many burned pixels: {burned}");
    }

    #[test]
    fn ordered_dither_is_deterministic() {
        let luma = [128u8; 8 * 8];
        let first = process_luma(&luma, 8, 8, &settings(ImageMode::OrderedDither)).unwrap();
        let second = process_luma(&luma, 8, 8, &settings(ImageMode::OrderedDither)).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.burned(), 32);
    }

    #[test]
    fn invalid_buffers_are_rejected() {
        assert!(process_luma(&[], 0, 0, &ImageSettings::default()).is_err());
        assert!(process_luma(&[0, 1], 3, 3, &ImageSettings::default()).is_err());
        let config = ImageSettings {
            contrast: 9.0,
            ..ImageSettings::default()
        };
        assert!(process_luma(&[0], 1, 1, &config).is_err());
    }

    #[test]
    fn missing_file_is_an_error() {
        let path = std::env::temp_dir().join("beamwright_missing_raster.png");
        let _ = std::fs::remove_file(&path);
        assert!(process_file(&path, &ImageSettings::default()).is_err());
    }

    #[test]
    fn processes_a_png_file() {
        let path = std::env::temp_dir().join("beamwright_raster_test.png");
        let mut image = image::GrayImage::new(4, 2);
        for (x, _y, pixel) in image.enumerate_pixels_mut() {
            *pixel = if x < 2 {
                image::Luma([0])
            } else {
                image::Luma([255])
            };
        }
        image.save(&path).unwrap();
        let bitmap = process_file(&path, &ImageSettings::default()).unwrap();
        assert_eq!(bitmap.width, 4);
        assert_eq!(bitmap.height, 2);
        assert_eq!(bitmap.burned(), 4);
        let _ = std::fs::remove_file(&path);
    }
}
