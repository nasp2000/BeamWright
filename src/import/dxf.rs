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
}
