use crate::document::Document;
use crate::errors::Result;

#[allow(clippy::unnecessary_wraps)]
pub fn export_dxf(doc: &Document) -> Result<String> {
    let mut out = String::new();
    out.push_str("0\nSECTION\n2\nENTITIES\n");
    for _obj in &doc.objects {
        out.push_str("0\nLINE\n");
    }
    out.push_str("0\nENDSEC\n0\nEOF\n");
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::Document;

    #[test]
    fn empty() {
        let d = Document::default();
        let s = export_dxf(&d).unwrap();
        assert!(s.contains("EOF"));
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
        let s = export_dxf(&d).unwrap();
        assert!(s.contains("LINE"));
    }

    #[test]
    fn header_footer() {
        let d = Document::default();
        let s = export_dxf(&d).unwrap();
        assert!(s.starts_with("0\nSECTION"));
        assert!(s.ends_with("EOF\n"));
    }
}
