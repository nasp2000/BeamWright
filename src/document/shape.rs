use serde::{Deserialize, Serialize};

use crate::geometry::{CubicBezier, Path, Point2, Transform2, path::Segment};
use crate::objects::{ImageObject, TextObject};

use crate::color::normalize_hex_color;

/// Geometric content of an object, in its own local coordinate system.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Shape {
    Path(Path),
    /// Axis-aligned rectangle anchored at the local origin.
    Rect {
        width: f64,
        height: f64,
        #[serde(default)]
        corner_radius: f64,
    },
    /// Ellipse centred on the local origin.
    Ellipse {
        rx: f64,
        ry: f64,
    },
    Text(TextObject),
    Image(ImageObject),
}

impl Shape {
    pub fn kind_name(&self) -> &'static str {
        match self {
            Shape::Path(_) => "Path",
            Shape::Rect { .. } => "Rectangle",
            Shape::Ellipse { .. } => "Ellipse",
            Shape::Text(_) => "Text",
            Shape::Image(_) => "Image",
        }
    }

    /// Convert the shape to a path in local coordinates.
    ///
    /// Text and image objects have no outline yet; they return an empty path
    /// and are rendered/exported through their own representation.
    pub fn to_path(&self) -> Path {
        match self {
            Shape::Path(p) => p.clone(),
            Shape::Rect {
                width,
                height,
                corner_radius,
            } => rect_path(*width, *height, *corner_radius),
            Shape::Ellipse { rx, ry } => ellipse_path(*rx, *ry),
            Shape::Text(_) | Shape::Image(_) => Path::new(),
        }
    }

    /// Bounding box in local coordinates.
    pub fn bounds(&self) -> Option<(Point2, Point2)> {
        match self {
            Shape::Path(p) => p.bounding_box(),
            Shape::Rect { width, height, .. } => {
                Some((Point2::origin(), Point2::new(width.abs(), height.abs())))
            }
            Shape::Ellipse { rx, ry } => Some((
                Point2::new(-rx.abs(), -ry.abs()),
                Point2::new(rx.abs(), ry.abs()),
            )),
            Shape::Text(t) => Some(t.bounds()),
            Shape::Image(i) => Some(i.bounds()),
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        match self {
            Shape::Path(p) => {
                if p.segments.is_empty() {
                    return Err("path has no segments".to_string());
                }
                for segment in &p.segments {
                    let points = match segment {
                        Segment::Line { from, to } => vec![*from, *to],
                        Segment::Cubic(c) => vec![c.p0, c.p1, c.p2, c.p3],
                    };
                    if points
                        .iter()
                        .any(|point| !point.x.is_finite() || !point.y.is_finite())
                    {
                        return Err("path contains non-finite coordinates".to_string());
                    }
                }
                Ok(())
            }
            Shape::Rect {
                width,
                height,
                corner_radius,
            } => {
                if !width.is_finite() || *width <= 0.0 {
                    return Err("rectangle width must be positive".to_string());
                }
                if !height.is_finite() || *height <= 0.0 {
                    return Err("rectangle height must be positive".to_string());
                }
                if !corner_radius.is_finite() || *corner_radius < 0.0 {
                    return Err("rectangle corner radius must be non-negative".to_string());
                }
                Ok(())
            }
            Shape::Ellipse { rx, ry } => {
                if !rx.is_finite() || *rx <= 0.0 || !ry.is_finite() || *ry <= 0.0 {
                    return Err("ellipse radii must be positive".to_string());
                }
                Ok(())
            }
            Shape::Text(t) => t.validate(),
            Shape::Image(i) => i.validate(),
        }
    }

    /// Axis-aligned bounds after applying `transform`.
    pub fn transformed_bounds(&self, transform: &Transform2) -> Option<(Point2, Point2)> {
        let (min, max) = self.bounds()?;
        let corners = [
            transform.apply(min),
            transform.apply(Point2::new(max.x, min.y)),
            transform.apply(max),
            transform.apply(Point2::new(min.x, max.y)),
        ];
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        for p in corners {
            min_x = min_x.min(p.x);
            min_y = min_y.min(p.y);
            max_x = max_x.max(p.x);
            max_y = max_y.max(p.y);
        }
        Some((Point2::new(min_x, min_y), Point2::new(max_x, max_y)))
    }
}

fn ellipse_path(rx: f64, ry: f64) -> Path {
    // Four cubic Bézier quadrants with the standard circle approximation.
    const K: f64 = 0.552_284_749_830_793_6;
    let mut path = Path::new();
    path.closed = true;
    let points = [
        (
            Point2::new(rx, 0.0),
            Point2::new(rx, ry * K),
            Point2::new(rx * K, ry),
            Point2::new(0.0, ry),
        ),
        (
            Point2::new(0.0, ry),
            Point2::new(-rx * K, ry),
            Point2::new(-rx, ry * K),
            Point2::new(-rx, 0.0),
        ),
        (
            Point2::new(-rx, 0.0),
            Point2::new(-rx, -ry * K),
            Point2::new(-rx * K, -ry),
            Point2::new(0.0, -ry),
        ),
        (
            Point2::new(0.0, -ry),
            Point2::new(rx * K, -ry),
            Point2::new(rx, -ry * K),
            Point2::new(rx, 0.0),
        ),
    ];
    for (p0, p1, p2, p3) in points {
        path.push(Segment::Cubic(CubicBezier { p0, p1, p2, p3 }));
    }
    path
}

#[allow(clippy::items_after_statements)]
fn rect_path(width: f64, height: f64, corner_radius: f64) -> Path {
    let width = width.abs();
    let height = height.abs();
    let radius = corner_radius.clamp(0.0, width.min(height) / 2.0);
    let mut path = Path::new();
    path.closed = true;
    if radius <= 1e-9 {
        let corners = [
            Point2::origin(),
            Point2::new(width, 0.0),
            Point2::new(width, height),
            Point2::new(0.0, height),
        ];
        for i in 0..4 {
            path.push(Segment::Line {
                from: corners[i],
                to: corners[(i + 1) % 4],
            });
        }
        return path;
    }
    const K: f64 = 0.552_284_749_830_793_6;
    // Start at the bottom edge, just after the bottom-left corner.
    let mut current = Point2::new(radius, 0.0);
    let corners = [
        (
            Point2::new(width - radius, 0.0),
            Point2::new(width, 0.0),
            Point2::new(width, radius),
        ),
        (
            Point2::new(width, height - radius),
            Point2::new(width, height),
            Point2::new(width - radius, height),
        ),
        (
            Point2::new(radius, height),
            Point2::new(0.0, height),
            Point2::new(0.0, height - radius),
        ),
        (
            Point2::new(0.0, radius),
            Point2::origin(),
            Point2::new(radius, 0.0),
        ),
    ];
    for (line_end, corner, next) in corners {
        path.push(Segment::Line {
            from: current,
            to: line_end,
        });
        let p0 = line_end;
        let p3 = next;
        path.push(Segment::Cubic(CubicBezier {
            p0,
            p1: Point2::new(p0.x + (corner.x - p0.x) * K, p0.y + (corner.y - p0.y) * K),
            p2: Point2::new(p3.x + (corner.x - p3.x) * K, p3.y + (corner.y - p3.y) * K),
            p3,
        }));
        current = next;
    }
    path
}

/// Stroke and fill style of an object.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Style {
    pub stroke: Option<String>,
    pub fill: Option<String>,
    pub stroke_width_mm: f64,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            stroke: Some("#000000".to_string()),
            fill: None,
            stroke_width_mm: 0.2,
        }
    }
}

impl Style {
    pub fn validate(&self) -> Result<(), String> {
        if let Some(stroke) = &self.stroke {
            if normalize_hex_color(stroke).is_none() {
                return Err(format!("invalid stroke colour: {stroke}"));
            }
        }
        if let Some(fill) = &self.fill {
            if normalize_hex_color(fill).is_none() {
                return Err(format!("invalid fill colour: {fill}"));
            }
        }
        if !self.stroke_width_mm.is_finite() || self.stroke_width_mm < 0.0 {
            return Err("stroke width must be finite and non-negative".to_string());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rect_path_closed() {
        let shape = Shape::Rect {
            width: 10.0,
            height: 5.0,
            corner_radius: 0.0,
        };
        let path = shape.to_path();
        assert_eq!(path.segment_count(), 4);
        assert!(path.is_closed());
        let (min, max) = shape.bounds().unwrap();
        assert!((min.x).abs() < 1e-9 && (max.x - 10.0).abs() < 1e-9);
    }

    #[test]
    fn ellipse_bounds() {
        let shape = Shape::Ellipse { rx: 4.0, ry: 2.0 };
        let (min, max) = shape.bounds().unwrap();
        assert!((min.x + 4.0).abs() < 1e-9);
        assert!((max.y - 2.0).abs() < 1e-9);
        assert_eq!(shape.to_path().segment_count(), 4);
    }

    #[test]
    fn invalid_rect() {
        let shape = Shape::Rect {
            width: 0.0,
            height: 5.0,
            corner_radius: 0.0,
        };
        assert!(shape.validate().is_err());
    }

    #[test]
    fn transformed_bounds() {
        let shape = Shape::Rect {
            width: 10.0,
            height: 2.0,
            corner_radius: 0.0,
        };
        let t = Transform2::rotate_around(std::f64::consts::FRAC_PI_2, 0.0, 0.0);
        let (min, max) = shape.transformed_bounds(&t).unwrap();
        assert!((min.x + 2.0).abs() < 1e-9);
        assert!((max.x).abs() < 1e-9);
        assert!((max.y - 10.0).abs() < 1e-9);
    }

    #[test]
    fn style_validation() {
        let mut style = Style::default();
        assert!(style.validate().is_ok());
        style.stroke = Some("not-a-colour".to_string());
        assert!(style.validate().is_err());
    }
}
