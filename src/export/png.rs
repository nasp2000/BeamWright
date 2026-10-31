#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]

use std::path::Path;

use crate::document::shape::Shape;
use crate::document::{Document, Object};
use crate::errors::Result;
use crate::geometry::Point2;

/// Raster output settings (§14.3).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PngOptions {
    /// Output resolution in dots per inch.
    pub dpi: f64,
    /// Transparent background instead of white.
    pub transparent: bool,
}

impl Default for PngOptions {
    fn default() -> Self {
        Self {
            dpi: 96.0,
            transparent: false,
        }
    }
}

impl PngOptions {
    pub fn validate(&self) -> Result<()> {
        if !self.dpi.is_finite() || !(10.0..=2400.0).contains(&self.dpi) {
            return Err(crate::errors::BeamError::InvalidInput(
                "PNG resolution must be between 10 and 2400 dpi".to_string(),
            ));
        }
        Ok(())
    }
}

/// Summary of a PNG export.
#[derive(Debug, Clone, PartialEq)]
pub struct PngReport {
    pub width_px: u32,
    pub height_px: u32,
    pub warnings: Vec<String>,
}

struct Canvas {
    width: usize,
    height: usize,
    pixels: Vec<u8>,
    scale: f64,
    height_mm: f64,
}

impl Canvas {
    fn new(document: &Document, options: &PngOptions) -> Self {
        let scale = options.dpi / 25.4;
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let width = ((document.width_mm * scale).round() as usize).max(1);
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let height = ((document.height_mm * scale).round() as usize).max(1);
        let background = if options.transparent {
            [0, 0, 0, 0]
        } else {
            [255, 255, 255, 255]
        };
        let mut pixels = Vec::with_capacity(width * height * 4);
        for _ in 0..width * height {
            pixels.extend_from_slice(&background);
        }
        Self {
            width,
            height,
            pixels,
            scale,
            height_mm: document.height_mm,
        }
    }

    /// Document millimetres to pixel coordinates (y axis flipped).
    fn px(&self, point: Point2) -> (f64, f64) {
        (
            point.x * self.scale,
            (self.height_mm - point.y) * self.scale,
        )
    }

    #[allow(
        clippy::cast_possible_wrap,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss
    )]
    fn blend(&mut self, x: i64, y: i64, color: [u8; 4]) {
        if x < 0 || y < 0 || x >= self.width as i64 || y >= self.height as i64 {
            return;
        }
        let index = (y as usize * self.width + x as usize) * 4;
        let alpha = u16::from(color[3]);
        if alpha == 255 || self.pixels[index + 3] == 0 {
            self.pixels[index..index + 4].copy_from_slice(&color);
            return;
        }
        for (channel, value) in color.iter().take(3).enumerate() {
            let dst = u16::from(self.pixels[index + channel]);
            let src = u16::from(*value);
            #[allow(clippy::cast_possible_truncation)]
            let value = ((src * alpha + dst * (255 - alpha)) / 255) as u8;
            self.pixels[index + channel] = value;
        }
        self.pixels[index + 3] = 255;
    }

    fn fill_polygon(&mut self, points: &[Point2], color: [u8; 4]) {
        if points.len() < 3 {
            return;
        }
        let pixels: Vec<(f64, f64)> = points.iter().map(|point| self.px(*point)).collect();
        let min_y = pixels
            .iter()
            .map(|(_, y)| *y)
            .fold(f64::INFINITY, f64::min)
            .floor()
            .max(0.0);
        let max_y = pixels
            .iter()
            .map(|(_, y)| *y)
            .fold(f64::NEG_INFINITY, f64::max)
            .ceil()
            .min((self.height - 1) as f64);
        let mut row = min_y as i64;
        while f64::from(row as i32) <= max_y {
            let scan = row as f64 + 0.5;
            let mut crossings: Vec<f64> = Vec::new();
            for index in 0..pixels.len() {
                let (x0, y0) = pixels[index];
                let (x1, y1) = pixels[(index + 1) % pixels.len()];
                if (y0 <= scan) != (y1 <= scan) {
                    let t = (scan - y0) / (y1 - y0);
                    crossings.push(x0 + t * (x1 - x0));
                }
            }
            crossings.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            for pair in crossings.chunks_exact(2) {
                let start = pair[0].ceil() as i64;
                let end = pair[1].floor() as i64;
                for x in start..=end {
                    self.blend(x, row, color);
                }
            }
            row += 1;
        }
    }

    fn stroke_polyline(&mut self, points: &[Point2], width_px: f64, color: [u8; 4]) {
        if points.len() < 2 {
            return;
        }
        let radius = (width_px / 2.0).max(0.5);
        let pixels: Vec<(f64, f64)> = points.iter().map(|point| self.px(*point)).collect();
        for window in pixels.windows(2) {
            let (x0, y0) = window[0];
            let (x1, y1) = window[1];
            let steps = ((x1 - x0).hypot(y1 - y0)).ceil().max(1.0) as usize;
            for step in 0..=steps {
                let t = step as f64 / steps as f64;
                self.stamp(x0 + (x1 - x0) * t, y0 + (y1 - y0) * t, radius, color);
            }
        }
    }

    fn stamp(&mut self, x: f64, y: f64, radius: f64, color: [u8; 4]) {
        let extent = radius.ceil() as i64;
        let cx = x.round() as i64;
        let cy = y.round() as i64;
        for dy in -extent..=extent {
            for dx in -extent..=extent {
                if f64::from(dx as i32).hypot(f64::from(dy as i32)) <= radius {
                    self.blend(cx + dx, cy + dy, color);
                }
            }
        }
    }
}

/// Render the document to an RGBA buffer.
pub fn render_rgba(
    document: &Document,
    options: &PngOptions,
) -> Result<(Vec<u8>, u32, u32, Vec<String>)> {
    options.validate()?;
    document
        .validate()
        .map_err(crate::errors::BeamError::InvalidInput)?;
    let mut canvas = Canvas::new(document, options);
    let mut warnings = Vec::new();
    let mut layers: Vec<_> = document.layers.iter().collect();
    layers.sort_by_key(|layer| layer.order);
    for layer in layers {
        if !layer.visible {
            continue;
        }
        let (lr, lg, lb) = layer.rgb();
        for object in document
            .objects
            .iter()
            .filter(|object| object.layer_id == layer.id && object.visible)
        {
            draw_object(&mut canvas, object, (lr, lg, lb), &mut warnings);
        }
    }
    #[allow(clippy::cast_possible_truncation)]
    let width = canvas.width as u32;
    #[allow(clippy::cast_possible_truncation)]
    let height = canvas.height as u32;
    Ok((canvas.pixels, width, height, warnings))
}

fn draw_object(
    canvas: &mut Canvas,
    object: &Object,
    layer_rgb: (u8, u8, u8),
    warnings: &mut Vec<String>,
) {
    let stroke_fallback = [layer_rgb.0, layer_rgb.1, layer_rgb.2];
    let stroke = object
        .style
        .stroke
        .as_deref()
        .and_then(crate::color::parse_hex_color)
        .map_or(stroke_fallback, |(r, g, b)| [r, g, b]);
    let fill = object
        .style
        .fill
        .as_deref()
        .and_then(crate::color::parse_hex_color);

    match &object.shape {
        Shape::Text(_) => warnings.push(format!(
            "text '{}' has no outline and was omitted from the PNG",
            object.name
        )),
        Shape::Image(image) => {
            let path = Path::new(&image.path);
            match image::open(path) {
                Ok(source) => {
                    if object.transform.b.abs() > 1e-9 || object.transform.c.abs() > 1e-9 {
                        warnings.push(format!(
                            "image '{}' is rotated; the PNG uses its bounding box",
                            object.name
                        ));
                    }
                    let (min, max) = object.bounds().unwrap_or_else(|| image.bounds());
                    let (x0, y0) = canvas.px(Point2::new(min.x, max.y));
                    let (x1, y1) = canvas.px(Point2::new(max.x, min.y));
                    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                    let width = ((x1 - x0).abs().round() as u32).max(1);
                    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                    let height = ((y1 - y0).abs().round() as u32).max(1);
                    let resized =
                        source.resize_exact(width, height, image::imageops::FilterType::Lanczos3);
                    let rgba = resized.to_rgba8();
                    #[allow(clippy::cast_possible_truncation)]
                    let start_x = x0.round() as i64;
                    #[allow(clippy::cast_possible_truncation)]
                    let start_y = y0.round() as i64;
                    for (px, py, pixel) in rgba.enumerate_pixels() {
                        canvas.blend(start_x + i64::from(px), start_y + i64::from(py), pixel.0);
                    }
                }
                Err(error) => warnings.push(format!(
                    "image '{}' could not be read: {error}",
                    object.name
                )),
            }
        }
        _ => {
            let path = object.world_path();
            let points = path.flatten(0.1);
            if let Some((r, g, b)) = fill {
                canvas.fill_polygon(&points, [r, g, b, 255]);
            }
            if object.style.stroke.is_some() {
                canvas.stroke_polyline(
                    &points,
                    object.style.stroke_width_mm * canvas.scale,
                    [stroke[0], stroke[1], stroke[2], 255],
                );
            }
        }
    }
}

/// Write the document as a PNG file, atomically.
pub fn export_png(document: &Document, path: &Path, options: &PngOptions) -> Result<PngReport> {
    let (pixels, width, height, warnings) = render_rgba(document, options)?;
    let buffer = image::RgbaImage::from_raw(width, height, pixels)
        .ok_or_else(|| crate::errors::BeamError::InvalidInput("invalid PNG buffer".to_string()))?;
    let temporary = path.with_extension("png.tmp");
    buffer
        .save_with_format(&temporary, image::ImageFormat::Png)
        .map_err(|error| {
            crate::errors::BeamError::InvalidInput(format!("cannot write PNG: {error}"))
        })?;
    std::fs::rename(&temporary, path)?;
    Ok(PngReport {
        width_px: width,
        height_px: height,
        warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::Style;
    use crate::document::model::{Object, Shape};
    use crate::geometry::Transform2;

    fn document_with_rect(fill: Option<&str>, stroke: bool) -> Document {
        let mut document = Document {
            width_mm: 10.0,
            height_mm: 10.0,
            ..Document::default()
        };
        let layer_id = document.layers[0].id.clone();
        let mut object = Object::new(
            layer_id,
            Shape::Rect {
                width: 10.0,
                height: 10.0,
                corner_radius: 0.0,
            },
        );
        object.style = Style {
            stroke: stroke.then(|| "#000000".to_string()),
            fill: fill.map(str::to_string),
            stroke_width_mm: 0.5,
        };
        document.add_object(object);
        document
    }

    #[test]
    fn filled_rectangle_covers_the_page() {
        let document = document_with_rect(Some("#ff0000"), false);
        let options = PngOptions {
            dpi: 10.0,
            transparent: false,
        };
        let (pixels, width, height, warnings) = render_rgba(&document, &options).unwrap();
        assert_eq!((width, height), (4, 4));
        assert_eq!(warnings.len(), 0);
        let index = (2 * 4 + 2) * 4;
        assert_eq!(&pixels[index..index + 4], &[255, 0, 0, 255]);
    }

    #[test]
    fn transparent_background_keeps_alpha_zero() {
        let document = document_with_rect(None, false);
        let options = PngOptions {
            dpi: 10.0,
            transparent: true,
        };
        let (pixels, ..) = render_rgba(&document, &options).unwrap();
        assert_eq!(pixels[3], 0);
    }

    #[test]
    fn hidden_layer_is_not_drawn() {
        let mut document = document_with_rect(Some("#000000"), false);
        document.layers[0].visible = false;
        let options = PngOptions {
            dpi: 10.0,
            transparent: true,
        };
        let (pixels, ..) = render_rgba(&document, &options).unwrap();
        assert_eq!(pixels[3], 0);
    }

    #[test]
    fn stroke_is_drawn_without_fill() {
        let document = document_with_rect(None, true);
        let options = PngOptions {
            dpi: 10.0,
            transparent: true,
        };
        let (pixels, ..) = render_rgba(&document, &options).unwrap();
        // The corner pixel belongs to the stroke.
        assert_eq!(pixels[3], 255);
    }

    #[test]
    fn invalid_options_are_rejected() {
        let document = Document::default();
        let options = PngOptions {
            dpi: 0.0,
            transparent: false,
        };
        assert!(render_rgba(&document, &options).is_err());
    }

    #[test]
    fn transform_is_applied() {
        let mut document = document_with_rect(Some("#ff0000"), false);
        document.objects[0].transform = Transform2::translate(5.0, 0.0);
        let options = PngOptions {
            dpi: 10.0,
            transparent: true,
        };
        let (pixels, width, ..) = render_rgba(&document, &options).unwrap();
        // The 10 mm rectangle shifted by 5 mm starts at about pixel 2.
        assert_eq!(pixels[(2 * width as usize) * 4 + 3], 0);
        assert_eq!(pixels[(2 * width as usize + 3) * 4 + 3], 255);
    }

    #[test]
    fn text_is_reported() {
        let mut document = Document::default();
        let layer_id = document.layers[0].id.clone();
        document.add_object(Object::new(
            layer_id,
            Shape::Text(crate::objects::TextObject {
                content: "hi".to_string(),
                font: "sans".to_string(),
                size: 4.0,
                x: 0.0,
                y: 0.0,
                ..crate::objects::TextObject::default()
            }),
        ));
        let (_, _, _, warnings) =
            render_rgba(&Document::default(), &PngOptions::default()).unwrap();
        assert_eq!(warnings.len(), 0);
        let (_, _, _, warnings) = render_rgba(&document, &PngOptions::default()).unwrap();
        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn writes_a_readable_png() {
        let document = document_with_rect(Some("#00ff00"), false);
        let path = std::env::temp_dir().join("beamwright_export_test.png");
        let _ = std::fs::remove_file(&path);
        let report = export_png(&document, &path, &PngOptions::default()).unwrap();
        assert!(report.width_px > 0);
        let decoded = image::open(&path).unwrap();
        assert_eq!(decoded.width(), report.width_px);
        assert!(!path.with_extension("png.tmp").exists());
        let _ = std::fs::remove_file(&path);
    }
}
