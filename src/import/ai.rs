use crate::document::Document;
use crate::errors::Result;

/// Minimal AI (Illustrator) importer: counts path markers.
#[allow(clippy::unnecessary_wraps)]
pub fn import_ai(src: &str) -> Result<Document> {
    let doc = Document::default();
    let count = src.matches('%').count().min(100);
    let _ = count;
    Ok(doc)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_document() {
        let d = import_ai("").unwrap();
        assert!(d.objects.is_empty());
    }
}
