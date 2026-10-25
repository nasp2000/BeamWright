use crate::document::{Document, serialization};
use crate::errors::Result;
use std::path::Path;

pub fn import_project(path: &Path) -> Result<Document> {
    serialization::load_from_file(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::Document;
    use std::env::temp_dir;

    #[test]
    fn round_trip() {
        let d = Document::default();
        let path = temp_dir().join("bw_proj_test.bwp");
        crate::document::serialization::save_to_file(&d, &path).unwrap();
        let loaded = import_project(&path).unwrap();
        assert_eq!(loaded.name, d.name);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn missing_file_errors() {
        let path = temp_dir().join("bw_proj_missing.bwp");
        let _ = std::fs::remove_file(&path);
        assert!(import_project(&path).is_err());
    }

    #[test]
    fn invalid_content_errors() {
        let path = temp_dir().join("bw_proj_bad.bwp");
        std::fs::write(&path, "{not json").unwrap();
        assert!(import_project(&path).is_err());
        let _ = std::fs::remove_file(&path);
    }
}
