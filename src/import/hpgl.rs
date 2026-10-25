use crate::document::Document;
use crate::errors::Result;

/// Minimal HPGL importer: counts PU/PD/PA commands.
#[allow(clippy::unnecessary_wraps)]
pub fn import_hpgl(src: &str) -> Result<Document> {
    let mut doc = Document::default();
    let cmd_count = src
        .split(';')
        .filter(|s| {
            let t = s.trim().to_uppercase();
            t.starts_with("PU") || t.starts_with("PD") || t.starts_with("PA")
        })
        .count();
    let layer_id = ensure_layer(&mut doc);
    for _ in 0..cmd_count {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty() {
        let d = import_hpgl("").unwrap();
        assert!(d.objects.is_empty());
    }

    #[test]
    fn pu_pd() {
        let d = import_hpgl("PU0,0;PD10,10;PU;").unwrap();
        assert_eq!(d.objects.len(), 3);
    }

    #[test]
    fn pa() {
        let d = import_hpgl("PA5,5;").unwrap();
        assert_eq!(d.objects.len(), 1);
    }
}
