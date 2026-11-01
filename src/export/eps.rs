use std::fmt::Write as _;

use crate::document::Document;
use crate::errors::Result;

use super::paths::{flatten, fmt_num};

/// Millimetres per PostScript point.
const MM_PER_POINT: f64 = 25.4 / 72.0;

/// Export the document as an Encapsulated PostScript file.
///
/// The bounding box is the work area in points, the origin is the bottom left
/// and every visible object becomes a stroked subpath in its layer colour. Curves
/// are flattened; text and images are not written, like the other vector
/// exporters.
#[allow(clippy::cast_possible_truncation)]
pub fn export_eps(document: &Document) -> Result<String> {
    let width = (document.width_mm / MM_PER_POINT).ceil().max(1.0) as i64;
    let height = (document.height_mm / MM_PER_POINT).ceil().max(1.0) as i64;
    let mut out = String::new();
    out.push_str("%!PS-Adobe-3.0 EPSF-3.0\n");
    out.push_str("%%Creator: BeamWright\n");
    let _ = writeln!(out, "%%BoundingBox: 0 0 {width} {height}");
    out.push_str("%%Pages: 1\n%%EndComments\n");

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
            let _ = writeln!(
                out,
                "{:.3} {:.3} {:.3} setrgbcolor",
                f64::from(red) / 255.0,
                f64::from(green) / 255.0,
                f64::from(blue) / 255.0
            );
            out.push_str("0.25 setlinewidth\n");
            let first = points[0];
            let _ = writeln!(out, "newpath {} {} moveto", point(first.0), point(first.1));
            for entry in &points[1..] {
                let _ = writeln!(out, "{} {} lineto", point(entry.0), point(entry.1));
            }
            if path.closed {
                out.push_str("closepath\n");
            }
            out.push_str("stroke\n");
        }
    }
    out.push_str("%%EOF\n");
    Ok(out)
}

fn point(mm: f64) -> String {
    fmt_num(mm / MM_PER_POINT)
}

#[cfg(test)]
mod tests {
    use super::export_eps;
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
    fn the_header_has_the_page_bounding_box() {
        let text = export_eps(&Document::default()).unwrap();
        assert!(text.starts_with("%!PS-Adobe-3.0 EPSF-3.0"));
        // 300 x 200 mm in points, rounded up.
        assert!(text.contains("%%BoundingBox: 0 0 851 567"), "{text}");
        assert!(text.ends_with("%%EOF\n"));
    }

    #[test]
    fn a_rectangle_is_a_closed_stroked_path() {
        let mut document = Document::default();
        let layer_id = document.layers[0].id.clone();
        let mut object = rectangle(10.0, 5.0);
        object.layer_id = layer_id;
        document.add_object(object);
        let text = export_eps(&document).unwrap();
        assert!(text.contains("newpath 0 0 moveto"), "{text}");
        assert!(text.contains("28.346457 0 lineto"), "{text}");
        assert!(text.contains("closepath\nstroke\n"), "{text}");
        assert!(text.contains("0.000 0.000 0.000 setrgbcolor"), "{text}");
    }

    #[test]
    fn invisible_objects_are_skipped() {
        let mut document = Document::default();
        let layer_id = document.layers[0].id.clone();
        let mut object = rectangle(10.0, 5.0);
        object.layer_id = layer_id;
        object.visible = false;
        document.add_object(object);
        let text = export_eps(&document).unwrap();
        assert!(!text.contains("moveto"), "{text}");
    }
}
