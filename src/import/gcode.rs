use crate::document::Document;
use crate::errors::Result;

/// Minimal G-code importer: counts G0/G1 move lines.
#[allow(clippy::unnecessary_wraps)]
pub fn import_gcode(src: &str) -> Result<Document> {
    let mut doc = Document::default();
    let move_count = src
        .lines()
        .filter(|l| {
            let t = l.trim().to_uppercase();
            t.starts_with("G0")
                || t.starts_with("G1")
                || t.starts_with("G00")
                || t.starts_with("G01")
        })
        .count();
    let layer_id = if doc.layers[0].id.is_empty() {
        doc.layers[0].id = "layer-1".to_string();
        doc.layers[0].id.clone()
    } else {
        doc.layers[0].id.clone()
    };
    for _ in 0..move_count {
        doc.objects.push(crate::document::model::DocumentObject {
            id: format!("obj-{}", doc.objects.len()),
            kind: crate::document::model::ObjectKind::Path,
            layer_id: layer_id.clone(),
            operation_id: None,
        });
    }
    Ok(doc)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty() {
        let d = import_gcode("").unwrap();
        assert!(d.objects.is_empty());
    }

    #[test]
    fn g1_moves() {
        let src = "G21\nG90\nG1 X10 Y0 F1000\nG0 X0 Y0\n";
        let d = import_gcode(src).unwrap();
        assert_eq!(d.objects.len(), 2);
    }

    #[test]
    fn g00_g01() {
        let src = "G00 X1 Y1\nG01 X2 Y2\n";
        let d = import_gcode(src).unwrap();
        assert_eq!(d.objects.len(), 2);
    }
}
