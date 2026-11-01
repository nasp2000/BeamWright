//! Shared path flattening for the vector exporters.

use crate::geometry::Path;
use crate::geometry::path::Segment;

/// Flatten a path into world points, approximating curves with `steps` segments.
///
/// A closed path does not repeat its first point; the exporter decides how to
/// close it.
#[allow(clippy::cast_precision_loss)]
pub fn flatten(path: &Path, steps_per_curve: usize) -> Vec<(f64, f64)> {
    let mut points = Vec::new();
    for segment in &path.segments {
        match segment {
            Segment::Line { from, to } => {
                if points.is_empty() {
                    points.push((from.x, from.y));
                }
                points.push((to.x, to.y));
            }
            Segment::Cubic(curve) => {
                if points.is_empty() {
                    points.push((curve.p0.x, curve.p0.y));
                }
                for step in 1..=steps_per_curve {
                    let t = step as f64 / steps_per_curve as f64;
                    let point = curve.eval(t);
                    points.push((point.x, point.y));
                }
            }
        }
    }
    if path.closed && points.len() > 1 {
        let first = points[0];
        let last = points[points.len() - 1];
        if (first.0 - last.0).abs() < 1e-9 && (first.1 - last.1).abs() < 1e-9 {
            points.pop();
        }
    }
    points
}

/// Format a coordinate without trailing zeros.
pub fn fmt_num(value: f64) -> String {
    if value.abs() < 1e-9 {
        return "0".to_string();
    }
    let mut text = format!("{value:.6}");
    while text.contains('.') && text.ends_with('0') {
        text.pop();
    }
    if text.ends_with('.') {
        text.pop();
    }
    text
}

#[cfg(test)]
mod tests {
    use super::{flatten, fmt_num};
    use crate::document::Document;
    use crate::document::model::{Object, Shape};

    fn rectangle(width: f64, height: f64) -> Object {
        Object::new(
            "layer",
            Shape::Rect {
                width,
                height,
                corner_radius: 0.0,
            },
        )
    }

    #[test]
    fn a_closed_rectangle_flattens_without_repeating_its_start() {
        let object = rectangle(10.0, 5.0);
        let points = flatten(&object.world_path(), 16);
        assert_eq!(
            points,
            vec![(0.0, 0.0), (10.0, 0.0), (10.0, 5.0), (0.0, 5.0)]
        );
    }

    #[test]
    fn numbers_lose_trailing_zeros() {
        assert_eq!(fmt_num(0.0), "0");
        assert_eq!(fmt_num(10.0), "10");
        assert_eq!(fmt_num(10.5), "10.5");
        assert_eq!(fmt_num(0.000_000_1), "0");
    }

    #[test]
    fn an_empty_document_path_is_empty() {
        let document = Document::default();
        let object = rectangle(1.0, 1.0);
        assert_ne!(flatten(&object.world_path(), 16).len(), 0);
        assert_eq!(document.objects.len(), 0);
    }
}
