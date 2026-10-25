use crate::document::Document;
use crate::errors::Result;

/// Parse a DXF file content into a Document.
#[allow(clippy::unnecessary_wraps)]
pub fn import_dxf(src: &str) -> Result<Document> {
    let mut doc = Document::default();
    // Parse group-code pairs
    let lines: Vec<&str> = src.lines().collect();
    let mut i = 0;
    let mut pairs: Vec<(i32, String)> = Vec::new();
    while i + 1 < lines.len() {
        if let Ok(code) = lines[i].trim().parse::<i32>() {
            pairs.push((code, lines[i + 1].trim().to_string()));
            i += 2;
        } else {
            i += 1;
        }
    }
    // Walk pairs, find entity starts (group code 0)
    let mut entity_count = 0usize;
    let mut idx = 0;
    // skip to ENTITIES section
    while idx < pairs.len() {
        if pairs[idx].0 == 0 && pairs[idx].1 == "SECTION" {
            // check section name
            if idx + 1 < pairs.len() && pairs[idx + 1].1 == "ENTITIES" {
                idx += 2;
                // parse entities until ENDSEC
                while idx < pairs.len() {
                    if pairs[idx].0 == 0 && pairs[idx].1 == "ENDSEC" {
                        break;
                    }
                    if pairs[idx].0 == 0 {
                        let etype = pairs[idx].1.clone();
                        if matches!(
                            etype.as_str(),
                            "LINE" | "CIRCLE" | "ARC" | "LWPOLYLINE" | "POLYLINE"
                        ) {
                            entity_count += 1;
                        }
                    }
                    idx += 1;
                }
                break;
            }
        }
        idx += 1;
    }
    // fallback: if no ENTITIES section found, count any entity markers
    if entity_count == 0 {
        for p in &pairs {
            if p.0 == 0
                && matches!(
                    p.1.as_str(),
                    "LINE" | "CIRCLE" | "ARC" | "LWPOLYLINE" | "POLYLINE"
                )
            {
                entity_count += 1;
            }
        }
    }
    let layer_id = ensure_layer(&mut doc);
    for _ in 0..entity_count {
        doc.objects.push(crate::document::model::DocumentObject {
            id: format!("obj-{}", doc.objects.len()),
            kind: crate::document::model::ObjectKind::Path,
            layer_id: layer_id.clone(),
            operation_id: None,
        });
    }
    Ok(doc)
}

fn ensure_layer(doc: &mut Document) -> String {
    if doc.layers[0].id.is_empty() {
        doc.layers[0].id = "layer-1".to_string();
    }
    doc.layers[0].id.clone()
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

/// Parse a DXF CIRCLE entity's center and radius from pairs.
pub fn parse_circle(pairs: &[(i32, f64)]) -> Option<((f64, f64), f64)> {
    let mut cx = None;
    let mut cy = None;
    let mut r = None;
    for (code, val) in pairs {
        match code {
            10 => cx = Some(*val),
            20 => cy = Some(*val),
            40 => r = Some(*val),
            _ => {}
        }
    }
    match (cx, cy, r) {
        (Some(a), Some(b), Some(c)) => Some(((a, b), c)),
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
    fn line_section() {
        let src =
            "0\nSECTION\n2\nENTITIES\n0\nLINE\n10\n0\n20\n0\n11\n10\n21\n0\n0\nENDSEC\n0\nEOF\n";
        let d = import_dxf(src).unwrap();
        assert_eq!(d.objects.len(), 1);
    }

    #[test]
    fn multiple_entities() {
        let src = "0\nSECTION\n2\nENTITIES\n0\nLINE\n0\nCIRCLE\n0\nARC\n0\nENDSEC\n0\nEOF\n";
        let d = import_dxf(src).unwrap();
        assert_eq!(d.objects.len(), 3);
    }

    #[test]
    fn fallback_count() {
        let src = "0\nLINE\n0\nCIRCLE\n";
        let d = import_dxf(src).unwrap();
        assert_eq!(d.objects.len(), 2);
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
    fn parse_circle_full() {
        let pairs = vec![(10, 5.0), (20, 6.0), (40, 2.0)];
        let r = parse_circle(&pairs).unwrap();
        assert_eq!(r.0, (5.0, 6.0));
        assert!((r.1 - 2.0).abs() < 1e-9);
    }

    #[test]
    fn parse_circle_missing() {
        let pairs = vec![(10, 5.0), (20, 6.0)];
        assert!(parse_circle(&pairs).is_none());
    }
}
