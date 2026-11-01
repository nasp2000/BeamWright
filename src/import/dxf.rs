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
const SUPPORTED: [&str; 6] = ["LINE", "CIRCLE", "ARC", "LWPOLYLINE", "ELLIPSE", "SPLINE"];

/// Import a DXF document into the native model.
///
/// Supported entities: `LINE`, `CIRCLE`, `ARC`, `LWPOLYLINE`, `ELLIPSE` and
/// `SPLINE` (sampled as a B-spline). Layer names and entity colours are preserved
/// when present. Other entity types are skipped and reported in
/// [`DxfImport::warnings`]; see `docs/formats.md`.
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
                if let Some(object) =
                    entity_to_object(&entity_type, fields, &mut document, &mut warnings)
                {
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

#[allow(clippy::too_many_lines)]
fn entity_to_object(
    entity_type: &str,
    fields: &[(i32, String)],
    document: &mut Document,
    warnings: &mut Vec<String>,
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
        "ELLIPSE" => {
            let centre = Point2::new(number(fields, 10)?, number(fields, 20)?);
            let major = Point2::new(number(fields, 11)?, number(fields, 21)?);
            let ratio = number(fields, 40).unwrap_or(1.0).abs();
            if major.x.hypot(major.y) <= 1e-9 || ratio <= 1e-9 {
                return None;
            }
            let start = number(fields, 41).unwrap_or(0.0);
            let mut end = number(fields, 42).unwrap_or(std::f64::consts::TAU);
            if (end - start).abs() < 1e-9 {
                end = start + std::f64::consts::TAU;
            }
            let sweep = end - start;
            let closed = (sweep.abs() - std::f64::consts::TAU).abs() < 1e-6;
            let minor = Point2::new(-major.y * ratio, major.x * ratio);
            let steps = ((sweep.abs() / std::f64::consts::TAU) * 64.0)
                .ceil()
                .max(8.0);
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let steps = steps as usize;
            let mut points = Vec::with_capacity(steps + 1);
            for step in 0..=steps {
                #[allow(clippy::cast_precision_loss)]
                let t = start + sweep * step as f64 / steps as f64;
                points.push((
                    centre.x + major.x * t.cos() + minor.x * t.sin(),
                    centre.y + major.y * t.cos() + minor.y * t.sin(),
                ));
            }
            if closed && points.len() > 1 {
                points.pop();
            }
            Object::new(layer_id, Shape::Path(Path::from_points(&points, closed)))
        }
        "SPLINE" => {
            let xs: Vec<f64> = fields
                .iter()
                .filter(|(code, _)| *code == 10)
                .filter_map(|(_, value)| value.trim().parse().ok())
                .collect();
            let ys: Vec<f64> = fields
                .iter()
                .filter(|(code, _)| *code == 20)
                .filter_map(|(_, value)| value.trim().parse().ok())
                .collect();
            let control: Vec<Point2> = xs
                .iter()
                .zip(&ys)
                .map(|(x, y)| Point2::new(*x, *y))
                .collect();
            if control.len() < 2 {
                return None;
            }
            let degree = fields
                .iter()
                .find(|(code, _)| *code == 71)
                .and_then(|(_, value)| value.trim().parse::<usize>().ok())
                .unwrap_or(3)
                .clamp(1, control.len() - 1);
            let knots: Vec<f64> = fields
                .iter()
                .filter(|(code, _)| *code == 40)
                .filter_map(|(_, value)| value.trim().parse().ok())
                .collect();
            if fields.iter().any(|(code, value)| {
                *code == 41
                    && value
                        .trim()
                        .parse::<f64>()
                        .is_ok_and(|weight| (weight - 1.0).abs() > 1e-9)
            }) {
                warnings.push(
                    "SPLINE control-point weights are ignored; the curve is not rational"
                        .to_string(),
                );
            }
            let points = sample_spline(&control, degree, &knots, 64);
            Object::new(layer_id, Shape::Path(Path::from_points(&points, false)))
        }
        _ => return None,
    };
    Some(object)
}

/// Sample a B-spline through `control`, clamped to its end points.
fn sample_spline(
    control: &[Point2],
    degree: usize,
    knots: &[f64],
    steps: usize,
) -> Vec<(f64, f64)> {
    let count = control.len();
    let degree = degree.clamp(1, count - 1);
    let knots = normalized_knots(count, degree, knots);
    #[allow(clippy::cast_precision_loss)]
    let start = knots[degree];
    let end = knots[count];
    let mut points = Vec::with_capacity(steps + 1);
    for step in 0..=steps {
        #[allow(clippy::cast_precision_loss)]
        let u = start + (end - start) * step as f64 / steps as f64;
        points.push(de_boor(control, degree, &knots, u.min(end - 1e-12)));
    }
    points[0] = (control[0].x, control[0].y);
    points[steps] = (control[count - 1].x, control[count - 1].y);
    points
}

/// A valid, non-decreasing knot vector, clamped at both ends.
fn normalized_knots(count: usize, degree: usize, knots: &[f64]) -> Vec<f64> {
    if knots.len() == count + degree + 1 && knots.windows(2).all(|pair| pair[0] <= pair[1]) {
        return knots.to_vec();
    }
    let spans = count - degree;
    let mut result = vec![0.0; degree + 1];
    for span in 1..spans {
        #[allow(clippy::cast_precision_loss)]
        result.push(span as f64);
    }
    #[allow(clippy::cast_precision_loss)]
    result.extend(std::iter::repeat_n(spans as f64, degree + 1));
    result
}

/// One point on a B-spline, evaluated with de Boor's algorithm.
fn de_boor(control: &[Point2], degree: usize, knots: &[f64], u: f64) -> (f64, f64) {
    let count = control.len();
    let mut span = degree;
    while span < count - 1 && u >= knots[span + 1] {
        span += 1;
    }
    let mut points: Vec<(f64, f64)> = (0..=degree)
        .map(|offset| {
            (
                control[span - degree + offset].x,
                control[span - degree + offset].y,
            )
        })
        .collect();
    for level in 1..=degree {
        for index in (level..=degree).rev() {
            let left = knots[span + index - degree];
            let right = knots[span + 1 + index - level];
            let denominator = right - left;
            let alpha = if denominator.abs() < 1e-12 {
                0.0
            } else {
                (u - left) / denominator
            };
            points[index] = (
                points[index - 1].0 * (1.0 - alpha) + points[index].0 * alpha,
                points[index - 1].1 * (1.0 - alpha) + points[index].1 * alpha,
            );
        }
    }
    points[degree]
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
    fn ellipse_entity_becomes_a_sampled_path() {
        let report = import_dxf_report(&dxf("0
ELLIPSE
10
0
20
0
11
10
21
0
40
0.5
"))
        .unwrap();
        assert_eq!(report.document.objects.len(), 1);
        assert_eq!(report.warnings.len(), 0);
        let Shape::Path(path) = &report.document.objects[0].shape else {
            panic!("expected a path");
        };
        assert!(path.closed);
        let points = path.to_points();
        // The major axis is 10 mm and the minor axis 5 mm.
        let max_x = points.iter().map(|(x, _)| *x).fold(f64::MIN, f64::max);
        let max_y = points.iter().map(|(_, y)| *y).fold(f64::MIN, f64::max);
        assert!((max_x - 10.0).abs() < 0.1, "{max_x}");
        assert!((max_y - 5.0).abs() < 0.1, "{max_y}");
    }

    #[test]
    fn spline_entity_is_sampled_between_its_end_points() {
        let report = import_dxf_report(&dxf("0
SPLINE
71
3
10
0
20
0
10
10
20
0
10
10
20
10
10
0
20
10
"))
        .unwrap();
        assert_eq!(report.document.objects.len(), 1);
        assert_eq!(report.warnings.len(), 0);
        let Shape::Path(path) = &report.document.objects[0].shape else {
            panic!("expected a path");
        };
        let points = path.to_points();
        assert!((points[0].0).abs() < 1e-9 && (points[0].1).abs() < 1e-9);
        let last = points[points.len() - 1];
        assert!((last.0).abs() < 1e-9 && (last.1 - 10.0).abs() < 1e-9);
        // The curve stays inside the control polygon.
        for (x, y) in &points {
            assert!((0.0..=10.0).contains(x), "{x}");
            assert!((0.0..=10.0).contains(y), "{y}");
        }
    }

    #[test]
    fn spline_weights_are_reported_as_ignored() {
        let report = import_dxf_report(&dxf("0
SPLINE
71
3
10
0
20
0
10
10
20
0
10
10
20
10
10
0
20
10
41
2
"))
        .unwrap();
        assert!(
            report
                .warnings
                .iter()
                .any(|warning| warning.contains("weights")),
            "{:?}",
            report.warnings
        );
    }

    #[test]
    fn a_single_cubic_spline_matches_its_bezier() {
        // Four control points with a clamped cubic knot vector form a Bézier.
        let control = [
            Point2::new(0.0, 0.0),
            Point2::new(0.0, 10.0),
            Point2::new(10.0, 10.0),
            Point2::new(10.0, 0.0),
        ];
        let points = sample_spline(&control, 3, &[], 8);
        // At t = 0.5 the curve is the midpoint of the middle control points.
        assert!(points[4].0 > 4.9 && points[4].0 < 5.1, "{:?}", points[4]);
        assert!(points[4].1 > 7.0, "{:?}", points[4]);
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
