use std::fmt::Write as _;

use crate::document::Document;
use crate::errors::Result;

use super::paths::flatten;

/// HPGL units per millimetre (40 units/mm is the common plotter resolution).
const UNITS_PER_MM: f64 = 40.0;

/// Export the document as HPGL for a pen plotter.
///
/// Every visible object becomes one `PU`/`PD` movement with the transform baked
/// into the coordinates. Curves are flattened, one pen is selected per layer
/// (cycling through eight) and the plotter is returned to the origin at the end.
/// Coordinates may be negative: the format is position-based, not clipped.
#[allow(clippy::cast_possible_truncation)]
pub fn export_hpgl(document: &Document) -> Result<String> {
    let mut out = String::from("IN;PA;SP1;");
    let mut layers: Vec<_> = document.layers.iter().collect();
    layers.sort_by_key(|layer| layer.order);
    let mut pen = 0;
    for layer in layers {
        if !layer.visible {
            continue;
        }
        pen = pen % 8 + 1;
        let _ = write!(out, "SP{pen};");
        for object in document
            .objects
            .iter()
            .filter(|object| object.layer_id == layer.id && object.visible)
        {
            let path = object.world_path();
            let mut points = flatten(&path, 16);
            if path.closed && points.len() > 1 {
                points.push(points[0]);
            }
            if points.len() < 2 {
                continue;
            }
            let _ = writeln!(out, "PU{};", coord(points[0]));
            out.push_str("PD");
            for point in &points[1..] {
                out.push_str(&coord(*point));
                out.push(',');
            }
            out.pop();
            out.push(';');
        }
    }
    out.push_str("SP0;PU0,0;");
    Ok(out)
}

#[allow(clippy::cast_possible_truncation)]
fn coord(point: (f64, f64)) -> String {
    format!(
        "{},{}",
        (point.0 * UNITS_PER_MM).round() as i64,
        (point.1 * UNITS_PER_MM).round() as i64
    )
}

#[cfg(test)]
mod tests {
    use super::export_hpgl;
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
    fn an_empty_document_initialises_and_parks_the_pen() {
        let text = export_hpgl(&Document::default()).unwrap();
        assert!(text.starts_with("IN;PA;SP1;"));
        assert!(text.ends_with("SP0;PU0,0;"));
    }

    #[test]
    fn a_rectangle_is_closed_in_plotter_units() {
        let mut document = Document::default();
        let layer_id = document.layers[0].id.clone();
        let mut object = rectangle(10.0, 5.0);
        object.layer_id = layer_id;
        document.add_object(object);
        let text = export_hpgl(&document).unwrap();
        assert!(text.contains("PU0,0;"), "{text}");
        // 10 mm is 400 units and 5 mm is 200 units, and the path closes.
        assert!(text.contains("PD400,0,400,200,0,200,0,0;"), "{text}");
    }

    #[test]
    fn invisible_objects_are_skipped() {
        let mut document = Document::default();
        let layer_id = document.layers[0].id.clone();
        let mut object = rectangle(10.0, 5.0);
        object.layer_id = layer_id;
        object.visible = false;
        document.add_object(object);
        let text = export_hpgl(&document).unwrap();
        assert!(!text.contains("PD"), "{text}");
    }
}
