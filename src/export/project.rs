use crate::document::Document;
use crate::errors::Result;
use std::path::Path;

pub fn export_project(doc: &Document, path: &Path) -> Result<()> {
    crate::document::serialization::save_to_file(doc, path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::Document;
    use std::env::temp_dir;

    #[test]
    fn round_trip() {
        let d = Document::default();
        let path = temp_dir().join("bw_exp_proj.bwp");
        export_project(&d, &path).unwrap();
        assert!(path.exists());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn invalid_path_errors() {
        let d = Document::default();
        let path = temp_dir().join("bw_exp_dir");
        std::fs::create_dir_all(&path).unwrap();
        // writing a file where a directory exists should error
        let file = path.join("x.bwp");
        std::fs::create_dir_all(&file).unwrap();
        assert!(export_project(&d, &file).is_err());
        let _ = std::fs::remove_dir_all(&path);
    }

    #[test]
    fn overwrite_ok() {
        let d = Document::default();
        let path = temp_dir().join("bw_exp_overwrite.bwp");
        export_project(&d, &path).unwrap();
        export_project(&d, &path).unwrap();
        assert!(path.exists());
        let _ = std::fs::remove_file(&path);
    }
}
