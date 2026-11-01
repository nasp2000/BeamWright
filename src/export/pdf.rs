use std::fmt::Write as _;

use crate::document::Document;
use crate::errors::Result;

use super::paths::{flatten, fmt_num};

/// Millimetres per PDF point (1 pt = 1/72 inch).
const MM_PER_POINT: f64 = 25.4 / 72.0;

/// Export the document as a single-page vector PDF.
///
/// The page is the work area, in points, with the origin at the bottom left as
/// in the document. Every visible object becomes a stroked subpath with its
/// layer colour; curves are flattened. Text and images are not written, matching
/// the other vector exporters. The file is self-contained: no fonts, no
/// compression, no external resources.
#[allow(clippy::cast_precision_loss)]
pub fn export_pdf(document: &Document) -> Result<String> {
    let mut content = String::new();
    let mut layers: Vec<_> = document.layers.iter().collect();
    layers.sort_by_key(|layer| layer.order);
    for layer in layers {
        if !layer.visible {
            continue;
        }
        let (red, green, blue) = layer.rgb();
        let stroke = format!(
            "{:.3} {:.3} {:.3} RG\n",
            f64::from(red) / 255.0,
            f64::from(green) / 255.0,
            f64::from(blue) / 255.0
        );
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
            content.push_str(&stroke);
            content.push_str("0.25 w\n");
            let first = points[0];
            let _ = writeln!(
                content,
                "{} {} m",
                fmt_num(first.0 / MM_PER_POINT),
                fmt_num(first.1 / MM_PER_POINT)
            );
            for point in &points[1..] {
                let _ = writeln!(
                    content,
                    "{} {} l",
                    fmt_num(point.0 / MM_PER_POINT),
                    fmt_num(point.1 / MM_PER_POINT)
                );
            }
            if path.closed {
                content.push_str("h\n");
            }
            content.push_str("S\n");
        }
    }

    let width = document.width_mm / MM_PER_POINT;
    let height = document.height_mm / MM_PER_POINT;
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
        format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {} {}] /Resources << >> /Contents 4 0 R >>",
            fmt_num(width),
            fmt_num(height)
        ),
        format!(
            "<< /Length {} >>\nstream\n{}endstream",
            content.len(),
            content
        ),
    ];

    let mut out = String::from("%PDF-1.4\n");
    let mut offsets = Vec::with_capacity(objects.len());
    for (index, body) in objects.iter().enumerate() {
        offsets.push(out.len());
        let _ = write!(out, "{} 0 obj\n{body}\nendobj\n", index + 1);
    }
    let xref_offset = out.len();
    let _ = writeln!(out, "xref\n0 {}", objects.len() + 1);
    out.push_str("0000000000 65535 f \n");
    for offset in &offsets {
        let _ = writeln!(out, "{offset:010} 00000 n ");
    }
    let _ = writeln!(
        out,
        "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref_offset}\n%%EOF",
        objects.len() + 1
    );
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::export_pdf;
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
    fn an_empty_document_is_a_valid_one_page_pdf() {
        let text = export_pdf(&Document::default()).unwrap();
        assert!(text.starts_with("%PDF-1.4\n"));
        assert!(text.ends_with("%%EOF\n"));
        assert!(text.contains("/Type /Catalog"));
        assert!(text.contains("/Count 1"));
        assert!(
            text.contains("/MediaBox [0 0 850.393701 566.929134]"),
            "{text}"
        );
    }

    #[test]
    fn a_rectangle_is_stroked_and_closed() {
        let mut document = Document::default();
        let layer_id = document.layers[0].id.clone();
        let mut object = rectangle(10.0, 5.0);
        object.layer_id = layer_id;
        document.add_object(object);
        let text = export_pdf(&document).unwrap();
        assert!(text.contains("0 0 m"), "{text}");
        // 10 mm and 5 mm in points.
        assert!(text.contains("28.346457 14.173228 l"), "{text}");
        assert!(text.contains("h\nS\n"), "{text}");
    }

    #[test]
    fn every_xref_offset_points_at_its_object() {
        let mut document = Document::default();
        let layer_id = document.layers[0].id.clone();
        let mut object = rectangle(20.0, 10.0);
        object.layer_id = layer_id;
        document.add_object(object);
        let text = export_pdf(&document).unwrap();
        let xref = text.find("xref\n").unwrap();
        let lines: Vec<&str> = text[xref..].lines().collect();
        assert_eq!(lines[1], "0 5");
        assert_eq!(lines[2], "0000000000 65535 f ");
        for (index, line) in lines[3..7].iter().enumerate() {
            let offset: usize = line[..10].parse().unwrap();
            assert!(
                text[offset..].starts_with(&format!("{} 0 obj", index + 1)),
                "object {} has a wrong offset",
                index + 1
            );
        }
    }

    #[test]
    fn the_startxref_matches_the_table() {
        let text = export_pdf(&Document::default()).unwrap();
        let marker = text.rfind("startxref\n").unwrap();
        let value: usize = text[marker + 10..].lines().next().unwrap().parse().unwrap();
        assert!(text[value..].starts_with("xref\n"));
    }

    #[test]
    fn invisible_objects_are_skipped() {
        let mut document = Document::default();
        let layer_id = document.layers[0].id.clone();
        let mut object = rectangle(10.0, 5.0);
        object.layer_id = layer_id;
        object.visible = false;
        document.add_object(object);
        let text = export_pdf(&document).unwrap();
        assert!(!text.contains(" m\n"), "{text}");
    }
}
