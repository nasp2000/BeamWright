use crate::errors::{BeamError, Result};
use crate::objects::{ImageMode, ImageSettings};

/// Maximum number of pixels accepted from an imported image.
pub const MAX_PIXELS: u64 = 50_000_000;

/// Power map produced from an image and its settings.
///
/// Most modes are binary and store only 0 or 255; grayscale engraving stores the
/// prepared tone as the power of every pixel.
#[derive(Debug, Clone, PartialEq)]
pub struct RasterBitmap {
    pub width: usize,
    pub height: usize,
    /// Laser power per pixel: 0 does not burn, 255 is full power.
    pub power: Vec<u8>,
}

impl RasterBitmap {
    pub fn burned(&self) -> usize {
        self.power.iter().filter(|value| **value > 0).count()
    }

    pub fn get(&self, x: usize, y: usize) -> bool {
        self.power_at(x, y) > 0
    }

    pub fn power_at(&self, x: usize, y: usize) -> u8 {
        self.power[y * self.width + x]
    }
}

/// Convert a luminance buffer using the image settings.
///
/// Pipeline: black/white clipping, brightness, contrast, tone curve (shadows,
/// midtones, highlights), gamma, enhancement, invert, then threshold or
/// dithering. The tone curve runs before gamma so the three controls shape the
/// tonal range rather than undoing the gamma correction. With all three at zero
/// the curve is an exact identity and the pipeline is unchanged.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]
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

    let mut adjusted: Vec<f32> = luma
        .iter()
        .map(|value| adjust(f32::from(*value), settings))
        .collect();
    if settings.enhance_radius_px > 0 && settings.enhance_amount > 0.0 {
        enhance(
            &mut adjusted,
            width,
            height,
            settings.enhance_radius_px as usize,
            settings.enhance_amount,
        );
    }

    let mut burn = vec![false; adjusted.len()];
    match settings.mode {
        ImageMode::Threshold => {
            for (index, value) in adjusted.iter().enumerate() {
                burn[index] = *value < f32::from(settings.threshold);
            }
        }
        ImageMode::FloydSteinberg => {
            burn = diffuse(&adjusted, width, height, &FLOYD_STEINBERG, 16);
        }
        ImageMode::Atkinson => {
            burn = diffuse(&adjusted, width, height, &ATKINSON, 8);
        }
        ImageMode::Stucki => {
            burn = diffuse(&adjusted, width, height, &STUCKI, 42);
        }
        ImageMode::Jarvis => {
            burn = diffuse(&adjusted, width, height, &JARVIS, 48);
        }
        ImageMode::Sierra => {
            burn = diffuse(&adjusted, width, height, &SIERRA, 32);
        }
        ImageMode::Burkes => {
            burn = diffuse(&adjusted, width, height, &BURKES, 32);
        }
        ImageMode::OrderedDither => {
            for y in 0..height {
                for x in 0..width {
                    let index = y * width + x;
                    let limit = (f32::from(BAYER_8[y % 8][x % 8]) + 0.5) * 255.0 / 64.0;
                    burn[index] = adjusted[index] < limit;
                }
            }
        }
        ImageMode::Halftone => {
            for y in 0..height {
                for x in 0..width {
                    let index = y * width + x;
                    let limit = (f32::from(CLUSTER_4[y % 4][x % 4]) + 0.5) * 255.0 / 16.0;
                    burn[index] = adjusted[index] < limit;
                }
            }
        }
        ImageMode::Newsprint => {
            let (sin, cos) = settings.halftone_angle_degrees.to_radians().sin_cos();
            for y in 0..height {
                for x in 0..width {
                    let index = y * width + x;
                    let rx = (x as f64 * cos + y as f64 * sin).rem_euclid(8.0) as usize;
                    let ry = (-(x as f64) * sin + y as f64 * cos).rem_euclid(8.0) as usize;
                    let u = (rx + ry) % 8;
                    let v = (rx + 2 * ry) % 8;
                    let limit = (f32::from(BAYER_8[u][v]) + 0.5) * 255.0 / 64.0;
                    burn[index] = adjusted[index] < limit;
                }
            }
        }
        ImageMode::Sketch => {
            burn = sketch(&adjusted, width, height, settings.threshold);
        }
        // Grayscale keeps the prepared tone and derives the power map below.
        ImageMode::Grayscale => {}
    }
    let power = if settings.mode == ImageMode::Grayscale {
        adjusted
            .iter()
            .map(|value| (255.0 - value).round().clamp(0.0, 255.0) as u8)
            .collect()
    } else {
        burn.iter()
            .map(|burned| if *burned { 255 } else { 0 })
            .collect()
    };
    Ok(RasterBitmap {
        width,
        height,
        power,
    })
}

const BAYER_8: [[u8; 8]; 8] = [
    [0, 32, 8, 40, 2, 34, 10, 42],
    [48, 16, 56, 24, 50, 18, 58, 26],
    [12, 44, 4, 36, 14, 46, 6, 38],
    [60, 28, 52, 20, 62, 30, 54, 22],
    [3, 35, 11, 43, 1, 33, 9, 41],
    [51, 19, 59, 27, 49, 17, 57, 25],
    [15, 47, 7, 39, 13, 45, 5, 37],
    [63, 31, 55, 23, 61, 29, 53, 21],
];

/// Clustered-dot screen used by the halftone modes.
const CLUSTER_4: [[u8; 4]; 4] = [[12, 5, 6, 13], [4, 0, 1, 7], [11, 3, 2, 8], [15, 10, 9, 14]];

const FLOYD_STEINBERG: [(i32, i32, i32); 4] = [(1, 0, 7), (-1, 1, 3), (0, 1, 5), (1, 1, 1)];

const ATKINSON: [(i32, i32, i32); 6] = [
    (1, 0, 1),
    (2, 0, 1),
    (-1, 1, 1),
    (0, 1, 1),
    (1, 1, 1),
    (0, 2, 1),
];

const STUCKI: [(i32, i32, i32); 12] = [
    (1, 0, 8),
    (2, 0, 4),
    (-2, 1, 2),
    (-1, 1, 4),
    (0, 1, 8),
    (1, 1, 4),
    (2, 1, 2),
    (-2, 2, 1),
    (-1, 2, 2),
    (0, 2, 4),
    (1, 2, 2),
    (2, 2, 1),
];

const SIERRA: [(i32, i32, i32); 10] = [
    (1, 0, 5),
    (2, 0, 3),
    (-2, 1, 2),
    (-1, 1, 4),
    (0, 1, 5),
    (1, 1, 4),
    (2, 1, 2),
    (-1, 2, 2),
    (0, 2, 3),
    (1, 2, 2),
];

const BURKES: [(i32, i32, i32); 7] = [
    (1, 0, 8),
    (2, 0, 4),
    (-2, 1, 2),
    (-1, 1, 4),
    (0, 1, 8),
    (1, 1, 4),
    (2, 1, 2),
];

const JARVIS: [(i32, i32, i32); 12] = [
    (1, 0, 7),
    (2, 0, 5),
    (-2, 1, 3),
    (-1, 1, 5),
    (0, 1, 7),
    (1, 1, 5),
    (2, 1, 3),
    (-2, 2, 1),
    (-1, 2, 3),
    (0, 2, 5),
    (1, 2, 3),
    (2, 2, 1),
];

/// Error diffusion with the given kernel and divisor.
#[allow(
    clippy::cast_possible_wrap,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]
fn diffuse(
    adjusted: &[f32],
    width: usize,
    height: usize,
    kernel: &[(i32, i32, i32)],
    divisor: i32,
) -> Vec<bool> {
    let mut values = adjusted.to_vec();
    let mut burn = vec![false; adjusted.len()];
    for y in 0..height {
        for x in 0..width {
            let index = y * width + x;
            let old = values[index];
            let new = if old < 128.0 { 0.0 } else { 255.0 };
            burn[index] = new == 0.0;
            let error = old - new;
            for (dx, dy, weight) in kernel {
                let nx = x as i32 + dx;
                let ny = y as i32 + dy;
                if nx >= 0 && nx < width as i32 && ny >= 0 && ny < height as i32 {
                    let target = ny as usize * width + nx as usize;
                    values[target] += error * (*weight as f32) / (divisor as f32);
                }
            }
        }
    }
    burn
}

/// Burn the edges found by a Sobel operator. `threshold` scales the gradient
/// that is considered an edge; flat areas stay unburned.
#[allow(clippy::cast_possible_wrap, clippy::cast_sign_loss)]
fn sketch(adjusted: &[f32], width: usize, height: usize, threshold: u8) -> Vec<bool> {
    let mut burn = vec![false; adjusted.len()];
    if width < 3 || height < 3 {
        return burn;
    }
    let limit = f32::from(threshold) * 4.0;
    let at = |x: usize, y: usize| adjusted[y * width + x];
    for y in 1..height - 1 {
        for x in 1..width - 1 {
            let gx = -at(x - 1, y - 1) + at(x + 1, y - 1) - 2.0 * at(x - 1, y) + 2.0 * at(x + 1, y)
                - at(x - 1, y + 1)
                + at(x + 1, y + 1);
            let gy = -at(x - 1, y - 1) - 2.0 * at(x, y - 1) - at(x + 1, y - 1)
                + at(x - 1, y + 1)
                + 2.0 * at(x, y + 1)
                + at(x + 1, y + 1);
            burn[y * width + x] = gx.hypot(gy) > limit;
        }
    }
    burn
}

/// Unsharp mask: add the difference between the image and a box blur.
fn enhance(values: &mut [f32], width: usize, height: usize, radius: usize, amount: f32) {
    let blurred = box_blur(values, width, height, radius);
    for (value, blurred) in values.iter_mut().zip(blurred) {
        *value = (*value + amount * (*value - blurred)).clamp(0.0, 255.0);
    }
}

fn box_blur(values: &[f32], width: usize, height: usize, radius: usize) -> Vec<f32> {
    if radius == 0 {
        return values.to_vec();
    }
    let mut horizontal = vec![0.0; values.len()];
    #[allow(clippy::cast_precision_loss)]
    for y in 0..height {
        for x in 0..width {
            let start = x.saturating_sub(radius);
            let end = (x + radius).min(width - 1);
            let sum: f32 = (start..=end).map(|sx| values[y * width + sx]).sum();
            let count = (end - start + 1) as f32;
            horizontal[y * width + x] = sum / count;
        }
    }
    let mut out = vec![0.0; values.len()];
    #[allow(clippy::cast_precision_loss)]
    for y in 0..height {
        let start = y.saturating_sub(radius);
        let end = (y + radius).min(height - 1);
        for x in 0..width {
            let sum: f32 = (start..=end).map(|sy| horizontal[sy * width + x]).sum();
            let count = (end - start + 1) as f32;
            out[y * width + x] = sum / count;
        }
    }
    out
}

fn adjust(value: f32, settings: &ImageSettings) -> f32 {
    let black = f32::from(settings.black_clip);
    let white = f32::from(settings.white_clip);
    let mut value = ((value - black) / (white - black) * 255.0).clamp(0.0, 255.0);
    value += f32::from(settings.brightness) * 2.55;
    value = (value - 128.0) * settings.contrast + 128.0;
    value = value.clamp(0.0, 255.0);
    value = tone_curve(value, settings);
    value = 255.0 * (value.clamp(0.0, 255.0) / 255.0).powf(1.0 / settings.gamma);
    if settings.invert {
        value = 255.0 - value;
    }
    value.clamp(0.0, 255.0)
}

/// Largest shift, in tone levels, that one tone-curve control can apply.
///
/// The three smooth weights below sum to one, so with every control at its
/// limit the offset is still bounded. The value is chosen so the curve remains
/// strictly increasing even when two controls pull in opposite directions:
/// the steepest possible slope of the offset is `4 * TONE_LIFT` (192), which is
/// below the identity slope of 255.
const TONE_LIFT: f32 = 48.0;

/// Smooth, monotonic tonal correction with shadow, midtone and highlight lifts.
///
/// The weights are the quadratic Bernstein basis on the normalised value:
/// `(1 - t)²` reaches its maximum at black, `2t(1 - t)` at mid-grey and `t²` at
/// white. They are smooth and sum to one, so a control affects its own range
/// most and the identity at zero is exact. The offset is added to the value and
/// clamped to 0..=255.
fn tone_curve(value: f32, settings: &ImageSettings) -> f32 {
    let shadow = settings.shadow_lift;
    let midtone = settings.midtone_shift;
    let highlight = settings.highlight_lift;
    if shadow == 0.0 && midtone == 0.0 && highlight == 0.0 {
        return value;
    }
    let t = (value / 255.0).clamp(0.0, 1.0);
    let dark_weight = (1.0 - t) * (1.0 - t);
    let mid_weight = 2.0 * t * (1.0 - t);
    let bright_weight = t * t;
    let offset =
        (shadow * dark_weight + midtone * mid_weight + highlight * bright_weight) * TONE_LIFT;
    (value + offset).clamp(0.0, 255.0)
}

/// Pixel dimensions of an image file, without decoding all its pixels twice.
pub fn image_dimensions(path: &std::path::Path) -> Result<(u32, u32)> {
    let reader = image::ImageReader::open(path)
        .map_err(|error| BeamError::InvalidInput(format!("cannot read image: {error}")))?;
    let reader = reader
        .with_guessed_format()
        .map_err(|error| BeamError::InvalidInput(format!("cannot read image: {error}")))?;
    reader
        .into_dimensions()
        .map_err(|error| BeamError::InvalidInput(format!("cannot read image: {error}")))
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
    if image.settings.flip_horizontal {
        picture = picture.fliph();
    }
    if image.settings.flip_vertical {
        picture = picture.flipv();
    }
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
    fn flips_mirror_the_picture() {
        let path = write_image("bw_raster_flip.png", 8, 4);
        let mut object = ImageObject::new(path.to_string_lossy(), 8, 4, 96.0);
        let normal = process_object(&path, &object).unwrap();
        assert!(normal.get(0, 0));
        assert!(!normal.get(7, 0));
        object.settings.flip_horizontal = true;
        let flipped = process_object(&path, &object).unwrap();
        assert!(!flipped.get(0, 0));
        assert!(flipped.get(7, 0));
        object.settings.flip_horizontal = false;
        object.settings.flip_vertical = true;
        let vertical = process_object(&path, &object).unwrap();
        assert_eq!(vertical, normal);
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
        assert!(bitmap.power.iter().all(|value| *value > 0));
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
    fn zero_tone_curve_leaves_a_gradient_unchanged() {
        let config = settings(ImageMode::Threshold);
        for level in 0..=255u8 {
            let adjusted = adjust(f32::from(level), &config);
            assert!(
                (adjusted - f32::from(level)).abs() < 1e-4,
                "level {level} became {adjusted}"
            );
        }
    }

    #[test]
    fn positive_shadow_lift_brightens_dark_pixels_more_than_bright_ones() {
        let mut config = settings(ImageMode::Threshold);
        config.shadow_lift = 1.0;
        let dark = adjust(16.0, &config) - 16.0;
        let bright = adjust(240.0, &config) - 240.0;
        assert!(dark > 30.0, "dark pixels moved by only {dark}");
        assert!(bright < 2.0, "bright pixels moved by {bright}");
        assert!(dark > bright);
    }

    #[test]
    fn positive_highlight_lift_brightens_bright_pixels_more_than_dark_ones() {
        let mut config = settings(ImageMode::Threshold);
        config.highlight_lift = 1.0;
        let dark = adjust(16.0, &config) - 16.0;
        let bright = adjust(200.0, &config) - 200.0;
        assert!(bright > 20.0, "bright pixels moved by only {bright}");
        assert!(dark < 2.0, "dark pixels moved by {dark}");
        assert!(bright > dark);
    }

    #[test]
    fn midtone_shift_moves_a_mid_grey() {
        let mut config = settings(ImageMode::Threshold);
        config.midtone_shift = 1.0;
        let lifted = adjust(128.0, &config);
        assert!((lifted - 152.0).abs() < 0.5, "mid-grey became {lifted}");
        config.midtone_shift = -1.0;
        let lowered = adjust(128.0, &config);
        assert!((lowered - 104.0).abs() < 0.5, "mid-grey became {lowered}");
        // A midtone shift leaves the absolute black and white ends alone.
        assert!(adjust(0.0, &config).abs() < 1e-4);
        assert!((adjust(255.0, &config) - 255.0).abs() < 1e-4);
    }

    #[test]
    fn tone_curves_stay_monotonic_and_clamped() {
        let combinations = [
            (1.0, 1.0, 1.0),
            (-1.0, -1.0, -1.0),
            (1.0, -1.0, 1.0),
            (-1.0, 1.0, -1.0),
            (1.0, -1.0, -1.0),
            (-1.0, 1.0, 1.0),
        ];
        for (shadow, midtone, highlight) in combinations {
            let mut config = settings(ImageMode::Threshold);
            config.shadow_lift = shadow;
            config.midtone_shift = midtone;
            config.highlight_lift = highlight;
            let mut previous = -1.0;
            for level in 0..=255u8 {
                let adjusted = adjust(f32::from(level), &config);
                assert!(
                    (0.0..=255.0).contains(&adjusted),
                    "level {level} left the range: {adjusted}"
                );
                assert!(
                    adjusted >= previous - 1e-4,
                    "curve is not monotonic at level {level}"
                );
                previous = adjusted;
            }
        }
        let mut config = settings(ImageMode::Threshold);
        config.highlight_lift = 1.0;
        assert!((adjust(255.0, &config) - 255.0).abs() < 1e-4);
        config.highlight_lift = 0.0;
        config.shadow_lift = -1.0;
        assert!(adjust(0.0, &config).abs() < 1e-4);
    }

    #[test]
    fn grayscale_mode_maps_tone_to_power() {
        let bitmap =
            process_luma(&[0, 64, 192, 255], 4, 1, &settings(ImageMode::Grayscale)).unwrap();
        assert_eq!(bitmap.power_at(0, 0), 255);
        assert_eq!(bitmap.power_at(1, 0), 191);
        assert_eq!(bitmap.power_at(2, 0), 63);
        assert_eq!(bitmap.power_at(3, 0), 0);
        assert!(bitmap.get(0, 0));
        assert!(!bitmap.get(3, 0));
        assert_eq!(bitmap.burned(), 3);
    }

    #[test]
    fn binary_modes_store_only_full_power_or_none() {
        let bitmap =
            process_luma(&[0, 100, 200, 255], 2, 2, &settings(ImageMode::Threshold)).unwrap();
        assert_eq!(bitmap.power, vec![255, 255, 0, 0]);
        let dithered = process_luma(
            &[0, 255, 255, 255],
            2,
            2,
            &settings(ImageMode::FloydSteinberg),
        )
        .unwrap();
        assert!(
            dithered
                .power
                .iter()
                .all(|value| *value == 0 || *value == 255),
            "a binary mode produced a partial power"
        );
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
    fn error_diffusion_modes_are_deterministic_and_keep_tone() {
        let luma = [128u8; 16 * 16];
        for mode in [ImageMode::Atkinson, ImageMode::Stucki, ImageMode::Jarvis] {
            let first = process_luma(&luma, 16, 16, &settings(mode)).unwrap();
            let second = process_luma(&luma, 16, 16, &settings(mode)).unwrap();
            assert_eq!(first, second, "{mode:?} is not deterministic");
            let burned = first.burned();
            assert!(
                (60..=196).contains(&burned),
                "{mode:?} burned {burned} of 256 pixels"
            );
        }
    }

    #[test]
    fn halftone_modes_repeat_and_keep_tone() {
        let luma = [128u8; 8 * 8];
        let halftone = process_luma(&luma, 8, 8, &settings(ImageMode::Halftone)).unwrap();
        assert_eq!(halftone.burned(), 32);
        for y in 0..4 {
            for x in 0..4 {
                assert_eq!(halftone.get(x, y), halftone.get(x + 4, y));
                assert_eq!(halftone.get(x, y), halftone.get(x, y + 4));
            }
        }
        let newsprint = process_luma(&luma, 8, 8, &settings(ImageMode::Newsprint)).unwrap();
        let again = process_luma(&luma, 8, 8, &settings(ImageMode::Newsprint)).unwrap();
        assert_eq!(newsprint, again);
        assert!(newsprint.burned() > 0 && newsprint.burned() < 64);
    }

    #[test]
    fn sketch_burns_only_edges() {
        let flat = [128u8; 8 * 8];
        assert_eq!(
            process_luma(&flat, 8, 8, &settings(ImageMode::Sketch))
                .unwrap()
                .burned(),
            0
        );
        let mut step = vec![0u8; 8 * 8];
        for y in 0..8 {
            for x in 0..8 {
                step[y * 8 + x] = if x < 4 { 0 } else { 255 };
            }
        }
        let edges = process_luma(&step, 8, 8, &settings(ImageMode::Sketch)).unwrap();
        assert!(edges.burned() > 0, "the step edge was not detected");
        assert!(!edges.get(2, 4), "flat interior burned");
        assert!(!edges.get(6, 4), "flat interior burned");
        assert!(!edges.get(0, 4), "border burned");
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
