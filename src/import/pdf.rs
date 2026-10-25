use crate::document::Document;
use crate::errors::Result;
use std::path::Path;

/// Minimal PDF importer: creates an empty document (PDF parsing not yet implemented).
#[allow(clippy::unnecessary_wraps)]
pub fn import_pdf(_path: &Path) -> Result<Document> {
    Ok(Document::default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env::temp_dir;

    #[test]
    fn returns_document() {
        let p = temp_dir().join("x.pdf");
        let d = import_pdf(&p).unwrap();
        assert!(d.objects.is_empty());
    }
}
