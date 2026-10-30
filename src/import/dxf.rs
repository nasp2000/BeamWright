use crate::document::Document;
use crate::document::model::{Object, Shape};
use crate::errors::{BeamError, Result};
use crate::geometry::{Path, Point2, Transform2, path::Segment};

/// Result of a DXF import, including elements that could not be converted.
#[derive(Debug, Clone, PartialEq)]
pub struct DxfImport {
    pub document: Document,
    pub warnings: Vec<String>,
}

/// Supported entity types.
const SUPPORTED: [&str; 4] = ["LINE", "CIRCLE", "ARC", "LWPOLYLINE"];

/// Import a DXF document into the native model.
///
/// Supported entities: `LINE`, `CIRCLE`, `ARC` and `LWPOLYLINE`. Layer names and
/// entity colours are preserved when present. Other entity types are skipped and
/// reported in [`DxfImport::warnings`]; see `docs/formats.md`.
pub fn import_dxf(src: &str) -> Result<Document> {
    Ok(import_dxf_report(src)?.document)
}

pub fn import_dxf_report(src: &str) -> Result<DxfImport> {
    let mut document = Document::default();
    let mut warnings = Vec::new();
    if src.trim().is_empty() {
        return Ok(DxfImport { document, warnings });
    }
    let pairs = group_pairs(src);
    if pairs.is_empty() {
        return Err(BeamError::InvalidInput(
            "not a DXF document: no group-code pairs".to_string(),
        ));
    }
    let mut skipped: Vec<String> = Vec::new();
    let mut index = 0;
    while index < pairs.len() {
        if pairs[index].0 != 0 {
            index += 1;
            continue;
        }
        let entity_type = pairs[index].1.to_ascii_uppercase();
        let mut end = index + 1;
        while end < pairs.len() && pairs[end].0 != 0 {
            end += 1;
        }
        let fields = &pairs[index + 1..end];
        // Structural markers are not entities.
        if !matches!(
            entity_type.as_str(),
            "SECTION" | "ENDSEC" | "EOF" | "TABLE" | "ENDTAB" | "BLOCK" | "ENDBLK"
        ) {
            if SUPPORTED.contains(&entity_type.as_str()) {
                if let Some(object) = entity_to_object(&entity_type, fields, &mut document) {
                    document.objects.push(object);
                } else {
                    warnings.push(format!(
                        "{entity_type} entity with invalid geometry was skipped"
                    ));
                }
            } else if !skipped.contains(&entity_type) {
                skipped.push(entity_type);
            }
        }
        index = end;
    }
    for entity in skipped {
        warnings.push(format!("unsupported DXF entity skipped: {entity}"));
    }
    Ok(DxfImport { document, warnings })
}

fn entity_to_object(
    entity_type: &str,
    fields: &[(i32, String)],
    document: &mut Document,
) -> Option<Object> {
    let layer_id = layer_for(fields, document);
    let object = match entity_type {
        "LINE" => {
            let from = Point2::new(number(fields, 10)?, number(fields, 20)?);
            let to = Point2::new(number(fields, 11)?, number(fields, 21)?);
            let mut path = Path::new();
            path.push(Segment::Line { from, to });
            Object::new(layer_id, Shape::Path(path))
        }
        "CIRCLE" => {
            let center = Point2::new(number(fields, 10)?, number(fields, 20)?);
            let radius = number(fields, 40)?;
            if radius <= 0.0 {
                return None;
            }
            Object::new(
                layer_id,
                Shape::Ellipse {
                    rx: radius,
                    ry: radius,
                },
            )
            .with_transform(Transform2::translate(center.x, center.y))
        }
        "ARC" => {
            let center = Point2::new(number(fields, 10)?, number(fields, 20)?);
            let radius = number(fields, 40)?;
            if radius <= 0.0 {
                return None;
            }
            let start = number(fields, 50)?.to_radians();
            let end = number(fields, 60)?.to_radians();
            Object::new(layer_id, Shape::Path(arc_path(center, radius, start, end)))
        }
        "LWPOLYLINE" => {
            let closed = fields
                .iter()
                .find(|(code, _)| *code == 70)
                .and_then(|(_, value)| value.trim().parse::<i32>().ok())
                .is_some_and(|flags| flags & 1 == 1);
            let xs = fields
                .iter()
                .filter(|(code, _)| *code == 10)
                .filter_map(|(_, value)| value.trim().parse::<f64>().ok());
            let ys = fields
                .iter()
                .filter(|(code, _)| *code == 20)
                .filter_map(|(_, value)| value.trim().parse::<f64>().ok());
            let vertices: Vec<(f64, f64)> = xs.zip(ys).collect();
            if vertices.len() < 2 {
                return None;
            }
            Object::new(layer_id, Shape::Path(Path::from_points(&vertices, closed)))
        }
        _ => return None,
    };
    Some(object)
}

fn layer_for(fields: &[(i32, String)], document: &mut Document) -> String {
    let name = fields
        .iter()
        .find(|(code, _)| *code == 8)
        .map(|(_, value)| value.trim().to_string())
        .filter(|value| !value.is_empty() && value != "0");
    match name {
        Some(name) => {
            if let Some(layer) = document.layers.iter().find(|layer| layer.name == name) {
                return layer.id.clone();
            }
            let index = fields
                .iter()
                .find(|(code, _)| *code == 62)
                .and_then(|(_, value)| value.trim().parse::<i32>().ok());
            let layer = document.add_layer(name);
            let id = layer.id.clone();
            if let Some(index) = index {
                let (r, g, b) = autocad_index_to_rgb(index);
                if let Some(layer) = document.layer_mut(&id) {
                    layer.color = crate::color::to_hex(r, g, b);
                }
            }
            id
        }
        None => document.layers[0].id.clone(),
    }
}

/// Approximate the standard seven-colour palette for entity colours.
fn autocad_index_to_rgb(index: i32) -> (u8, u8, u8) {
    match index.abs() {
        1 => (255, 0, 0),
        2 => (255, 255, 0),
        3 => (0, 255, 0),
        4 => (0, 255, 255),
        5 => (0, 0, 255),
        6 => (255, 0, 255),
        _ => (0, 0, 0),
    }
}

fn arc_path(center: Point2, radius: f64, start: f64, end: f64) -> Path {
    let sweep = if end > start {
        end - start
    } else {
        end - start + std::f64::consts::TAU
    };
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let steps = ((sweep.abs() / std::f64::consts::TAU) * 64.0)
        .ceil()
        .max(8.0) as usize;
    let mut path = Path::new();
    let mut previous = Point2::new(
        center.x + radius * start.cos(),
        center.y + radius * start.sin(),
    );
    for step in 1..=steps {
        #[allow(clippy::cast_precision_loss)]
        let angle = start + sweep * (step as f64 / steps as f64);
        let point = Point2::new(
            center.x + radius * angle.cos(),
            center.y + radius * angle.sin(),
        );
        path.push(Segment::Line {
            from: previous,
            to: point,
        });
        previous = point;
    }
    path
}

fn group_pairs(src: &str) -> Vec<(i32, String)> {
    let lines: Vec<&str> = src.lines().collect();
    let mut pairs = Vec::new();
    let mut index = 0;
    while index + 1 < lines.len() {
        if let Ok(code) = lines[index].trim().parse::<i32>() {
            pairs.push((code, lines[index + 1].trim().to_string()));
            index += 2;
        } else {
            index += 1;
        }
    }
    pairs
}

fn number(fields: &[(i32, String)], code: i32) -> Option<f64> {
    fields
        .iter()
        .find(|(field_code, _)| *field_code == code)
        .and_then(|(_, value)| value.trim().parse::<f64>().ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dxf(body: &str) -> String {
        format!("0\nSECTION\n2\nENTITIES\n{body}0\nENDSEC\n0\nEOF\n")
    }

    #[test]
    fn empty_is_empty_document() {
        let document = import_dxf("").unwrap();
        assert_eq!(document.objects.len(), 0);
        assert_eq!(document.layers.len(), 1);
    }

    #[test]
    fn rejects_garbage() {
        assert!(import_dxf("not a dxf").is_err());
    }

    #[test]
    fn line_entity_and_layer() {
        let report =
            import_dxf_report(&dxf("0\nLINE\n8\nCut\n10\n1\n20\n2\n11\n11\n21\n12\n")).unwrap();
        let document = report.document;
        assert_eq!(document.objects.len(), 1);
        assert_eq!(document.layers.len(), 2);
        assert_eq!(document.layers[1].name, "Cut");
        match &document.objects[0].shape {
            Shape::Path(path) => match &path.segments[0] {
                Segment::Line { from, to } => {
                    assert_eq!((from.x, from.y), (1.0, 2.0));
                    assert_eq!((to.x, to.y), (11.0, 12.0));
                }
                Segment::Cubic(_) => panic!("unexpected cubic segment"),
            },
            other => panic!("unexpected shape: {other:?}"),
        }
        assert_eq!(report.warnings.len(), 0);
    }

    #[test]
    fn circle_entity() {
        let document = import_dxf(&dxf("0\nCIRCLE\n10\n5\n20\n5\n40\n2\n")).unwrap();
        assert_eq!(
            document.objects[0].shape,
            Shape::Ellipse { rx: 2.0, ry: 2.0 }
        );
        assert_eq!(document.objects[0].transform.tx, 5.0);
    }

    #[test]
    fn lwpolyline_closed() {
        let document = import_dxf(&dxf(
            "0\nLWPOLYLINE\n90\n3\n70\n1\n10\n0\n20\n0\n10\n10\n20\n0\n10\n10\n20\n10\n",
        ))
        .unwrap();
        match &document.objects[0].shape {
            Shape::Path(path) => {
                assert!(path.closed);
                assert_eq!(path.segment_count(), 3);
            }
            other => panic!("unexpected shape: {other:?}"),
        }
    }

    #[test]
    fn arc_has_endpoints() {
        let document = import_dxf(&dxf("0\nARC\n10\n0\n20\n0\n40\n10\n50\n0\n60\n90\n")).unwrap();
        match &document.objects[0].shape {
            Shape::Path(path) => {
                let first = path.segments.first().unwrap();
                let last = path.segments.last().unwrap();
                let (Segment::Line { from, .. }, Segment::Line { to, .. }) = (first, last) else {
                    panic!("expected line segments");
                };
                assert!((from.x - 10.0).abs() < 1e-6);
                assert!(to.y > 9.9);
            }
            other => panic!("unexpected shape: {other:?}"),
        }
    }

    #[test]
    fn unsupported_entity_is_reported_once() {
        let report =
            import_dxf_report(&dxf("0\nMTEXT\n10\n0\n20\n0\n0\nMTEXT\n10\n1\n20\n1\n")).unwrap();
        assert_eq!(report.document.objects.len(), 0);
        assert_eq!(report.warnings.len(), 1);
        assert!(report.warnings[0].contains("MTEXT"));
    }

    #[test]
    fn invalid_geometry_is_reported() {
        let report = import_dxf_report(&dxf("0\nCIRCLE\n10\n0\n20\n0\n40\n-1\n")).unwrap();
        assert_eq!(report.document.objects.len(), 0);
        assert_eq!(report.warnings.len(), 1);
    }
}
