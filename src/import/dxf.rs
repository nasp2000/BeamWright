use crate::document::Document;
use crate::errors::Result;

/// Minimal DXF importer (Phase 6, P1). Counts supported entities.
#[allow(clippy::unnecessary_wraps)]
pub fn import_dxf(src: &str) -> Result<Document> {
    let mut doc = Document::default();
    // Count LINE entities
    let line_count = src.matches("LINE").count();
    let circle_count = src.matches("CIRCLE").count();
    let arc_count = src.matches("ARC").count();
    let total = line_count + circle_count + arc_count;
    for _ in 0..total {
        doc.objects.push(crate::document::model::DocumentObject {
            id: format!("obj-{}", doc.objects.len()),
            kind: crate::document::model::ObjectKind::Path,
            layer_id: doc.layers[0].id.clone(),
        });
    }
    if doc.layers[0].id.is_empty() {
        doc.layers[0].id = "layer-1".to_string();
    }
    Ok(doc)
}

/// Parse a DXF LINE entity's coordinates from a flat list of (code, value) pairs.
pub fn parse_line_coords(pairs: &[(i32, f64)]) -> Option<((f64, f64), (f64, f64))> {
    let mut x1 = None;
    let mut y1 = None;
    let mut x2 = None;
    let mut y2 = None;
    for (code, val) in pairs {
        match code {
            10 => x1 = Some(*val),
            20 => y1 = Some(*val),
            11 => x2 = Some(*val),
            21 => y2 = Some(*val),
            _ => {}
        }
    }
    match (x1, y1, x2, y2) {
        (Some(a), Some(b), Some(c), Some(d)) => Some(((a, b), (c, d))),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty() {
        let d = import_dxf("").unwrap();
        assert!(d.objects.is_empty());
    }

    #[test]
    fn line() {
        let d = import_dxf("0\nLINE\n").unwrap();
        assert_eq!(d.objects.len(), 1);
    }

    #[test]
    fn multiple() {
        let src = "LINE\nLINE\nCIRCLE\nARC\n";
        let d = import_dxf(src).unwrap();
        assert_eq!(d.objects.len(), 4);
    }

    #[test]
    fn parse_line_full() {
        let pairs = vec![(10, 1.0), (20, 2.0), (11, 3.0), (21, 4.0)];
        let r = parse_line_coords(&pairs).unwrap();
        assert_eq!(r.0, (1.0, 2.0));
        assert_eq!(r.1, (3.0, 4.0));
    }

    #[test]
    fn parse_line_missing() {
        let pairs = vec![(10, 1.0), (20, 2.0)];
        assert!(parse_line_coords(&pairs).is_none());
    }

    #[test]
    fn parse_line_empty() {
        assert!(parse_line_coords(&[]).is_none());
    }
}
