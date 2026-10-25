use crate::document::Document;
use crate::errors::Result;
use std::fmt::Write as _;

/// Minimal SVG exporter (Phase 6, P0).
#[allow(clippy::unnecessary_wraps)]
pub fn export_svg(doc: &Document) -> Result<String> {
    let mut out = String::new();
    let _ = write!(
        out,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{}" height="{}">"#,
        doc.width_mm, doc.height_mm
    );
    out.push('\n');
    for obj in &doc.objects {
        match obj.kind {
            crate::document::model::ObjectKind::Rectangle => {
                out.push_str("  <rect x=\"0\" y=\"0\" width=\"10\" height=\"5\"/>\n");
            }
            crate::document::model::ObjectKind::Ellipse => {
                out.push_str("  <ellipse cx=\"5\" cy=\"5\" rx=\"2\" ry=\"2\"/>\n");
            }
            crate::document::model::ObjectKind::Line => {
                out.push_str("  <line x1=\"0\" y1=\"0\" x2=\"10\" y2=\"10\"/>\n");
            }
            crate::document::model::ObjectKind::Path => {
                out.push_str("  <path d=\"M0 0 L10 10\"/>\n");
            }
            crate::document::model::ObjectKind::Text => {
                out.push_str("  <text x=\"0\" y=\"10\">text</text>\n");
            }
            crate::document::model::ObjectKind::Image => {
                out.push_str("  <image href=\"\" x=\"0\" y=\"0\" width=\"10\" height=\"10\"/>\n");
            }
            crate::document::model::ObjectKind::Polygon => {
                out.push_str("  <polygon points=\"0,0 10,0 10,10 0,10\"/>\n");
            }
            crate::document::model::ObjectKind::Group => {
                out.push_str("  <g></g>\n");
            }
        }
    }
    out.push_str("</svg>\n");
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::Document;

    #[test]
    fn empty_document() {
        let d = Document::default();
        let s = export_svg(&d).unwrap();
        assert!(s.contains("<svg"));
        assert!(s.contains("</svg>"));
    }

    #[test]
    fn includes_objects() {
        use crate::document::model::{DocumentObject, ObjectKind};
        let mut d = Document::default();
        d.objects.push(DocumentObject {
            id: "o1".to_string(),
            kind: ObjectKind::Rectangle,
            layer_id: "l1".to_string(),
        });
        let s = export_svg(&d).unwrap();
        assert!(s.contains("<rect"));
    }

    #[test]
    fn round_trip_structure() {
        let d = Document::default();
        let s = export_svg(&d).unwrap();
        assert!(s.starts_with("<svg"));
    }
}
