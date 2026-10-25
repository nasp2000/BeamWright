use crate::document::Document;
use crate::errors::Result;

#[allow(clippy::unnecessary_wraps)]
pub fn export_hpgl(doc: &Document) -> Result<String> {
    let mut out = String::new();
    out.push_str("IN;");
    out.push_str("SP1;");
    for obj in &doc.objects {
        out.push_str("PU;PD;");
        let _ = obj;
    }
    out.push_str("PU;");
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::Document;

    #[test]
    fn empty() {
        let d = Document::default();
        let s = export_hpgl(&d).unwrap();
        assert!(s.starts_with("IN;"));
        assert!(s.ends_with("PU;"));
    }

    #[test]
    fn with_objects() {
        use crate::document::model::{DocumentObject, ObjectKind};
        let mut d = Document::default();
        d.objects.push(DocumentObject {
            id: "o1".to_string(),
            kind: ObjectKind::Line,
            layer_id: "l1".to_string(),
            operation_id: None,
        });
        let s = export_hpgl(&d).unwrap();
        assert!(s.contains("PD;"));
    }
}
