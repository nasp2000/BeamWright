use std::fmt::Write as _;

use crate::document::Document;
use crate::errors::Result;
use crate::geometry::{Path, path::Segment};

/// Export the document as DXF, aiming at the widely supported R12 `ENTITIES`
/// subset.
///
/// Each visible object becomes one `LWPOLYLINE` with the transform baked into the
/// vertices. Bézier segments are flattened. Layers are written as entity layer
/// names and colours; no layer table is emitted. See `docs/formats.md`.
pub fn export_dxf(document: &Document) -> Result<String> {
    let mut out = String::new();
    out.push_str("0\nSECTION\n2\nENTITIES\n");
    let mut layers: Vec<_> = document.layers.iter().collect();
    layers.sort_by_key(|layer| layer.order);
    for layer in layers {
        if !layer.visible {
            continue;
        }
        let (red, green, blue) = layer.rgb();
        for object in document
            .objects
            .iter()
            .filter(|object| object.layer_id == layer.id && object.visible)
        {
            let path = object.world_path();
            let points = flatten(&path, 16);
            if points.len() < 2 {
                continue;
            }
            out.push_str("0\nLWPOLYLINE\n");
            let _ = writeln!(out, "8\n{}", layer.name);
            let _ = writeln!(out, "62\n{}", true_colour(red, green, blue));
            let _ = writeln!(out, "90\n{}", points.len());
            let _ = writeln!(out, "70\n{}", u32::from(path.closed));
            for (x, y) in points {
                let _ = writeln!(out, "10\n{}", fmt_num(x));
                let _ = writeln!(out, "20\n{}", fmt_num(y));
            }
        }
    }
    out.push_str("0\nENDSEC\n0\nEOF\n");
    Ok(out)
}

/// Pack an RGB colour as a DXF true-colour value.
fn true_colour(red: u8, green: u8, blue: u8) -> u32 {
    (u32::from(red) << 16) | (u32::from(green) << 8) | u32::from(blue)
}

#[allow(clippy::cast_precision_loss)]
fn flatten(path: &Path, steps_per_curve: usize) -> Vec<(f64, f64)> {
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

fn fmt_num(value: f64) -> String {
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
    use super::*;
    use crate::document::model::{Object, Shape};
    use crate::geometry::Transform2;

    #[test]
    fn empty_document_has_header_and_footer() {
        let text = export_dxf(&Document::default()).unwrap();
        assert!(text.starts_with("0\nSECTION\n2\nENTITIES\n"));
        assert!(text.ends_with("0\nENDSEC\n0\nEOF\n"));
    }

    #[test]
    fn rectangle_is_closed_polyline_with_colour() {
        let mut document = Document::default();
        document.layers[0].color = "#ff0000".to_string();
        let layer_id = document.layers[0].id.clone();
        document.add_object(Object::new(
            layer_id,
            Shape::Rect {
                width: 10.0,
                height: 5.0,
                corner_radius: 0.0,
            },
        ));
        let text = export_dxf(&document).unwrap();
        assert!(text.contains("0\nLWPOLYLINE\n"));
        assert!(text.contains("90\n4\n"));
        assert!(text.contains("70\n1\n"));
        assert!(text.contains("62\n16711680\n"));
        assert!(text.contains("10\n10\n20\n5\n"));
    }

    #[test]
    fn transform_is_baked() {
        let mut document = Document::default();
        let layer_id = document.layers[0].id.clone();
        let object = Object::new(
            layer_id,
            Shape::Rect {
                width: 2.0,
                height: 2.0,
                corner_radius: 0.0,
            },
        )
        .with_transform(Transform2::translate(100.0, 50.0));
        document.add_object(object);
        let text = export_dxf(&document).unwrap();
        assert!(text.contains("10\n100\n20\n50\n"));
    }

    #[test]
    fn hidden_layers_and_objects_are_skipped() {
        let mut document = Document::default();
        let layer_id = document.layers[0].id.clone();
        document.add_object(Object::new(
            layer_id.clone(),
            Shape::Ellipse { rx: 1.0, ry: 1.0 },
        ));
        document.layer_mut(&layer_id).unwrap().visible = false;
        let text = export_dxf(&document).unwrap();
        assert!(!text.contains("LWPOLYLINE"));
    }

    #[test]
    fn curves_are_flattened_to_several_vertices() {
        let mut document = Document::default();
        let layer_id = document.layers[0].id.clone();
        document.add_object(Object::new(layer_id, Shape::Ellipse { rx: 4.0, ry: 2.0 }));
        let text = export_dxf(&document).unwrap();
        assert!(text.contains("90\n64\n"));
    }

    #[test]
    fn dxf_round_trip_preserves_a_rectangle() {
        let mut document = Document::default();
        let layer_id = document.layers[0].id.clone();
        document.add_object(Object::new(
            layer_id,
            Shape::Rect {
                width: 20.0,
                height: 10.0,
                corner_radius: 0.0,
            },
        ));
        let text = export_dxf(&document).unwrap();
        let imported = crate::import::dxf::import_dxf(&text).unwrap();
        assert_eq!(imported.objects.len(), 1);
        let (min, max) = imported.objects[0].bounds().unwrap();
        assert!((min.x).abs() < 1e-6);
        assert!((max.x - 20.0).abs() < 1e-6);
        assert!((max.y - 10.0).abs() < 1e-6);
    }
}
