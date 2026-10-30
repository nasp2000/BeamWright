use std::path::Path;

use crate::document::Document;
use crate::errors::{BeamError, Result};

/// Format detected from a file extension.
pub fn detect_format(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_lowercase)
        .as_deref()
    {
        Some("svg") => "svg",
        Some("bwp") => "bwp",
        _ => "unknown",
    }
}

/// Import a vector file into a new document.
///
/// Only SVG is supported for import. Other formats return an explicit error
/// instead of silently producing an empty document.
pub fn import_by_extension(path: &Path) -> Result<Document> {
    match detect_format(path) {
        "svg" => {
            let source = std::fs::read_to_string(path)?;
            crate::import::svg::import_svg(&source)
        }
        "bwp" => crate::document::serialization::load_from_file(path),
        other => Err(BeamError::Unsupported(format!(
            "cannot import '{other}' files; supported formats: .svg, .bwp"
        ))),
    }
}

/// Open either a native project or an importable vector file.
pub fn open_document(path: &Path) -> Result<Document> {
    import_by_extension(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env::temp_dir;

    #[test]
    fn detect_svg() {
        assert_eq!(detect_format(Path::new("a.svg")), "svg");
    }

    #[test]
    fn detect_bwp_case_insensitive() {
        assert_eq!(detect_format(Path::new("a.BWP")), "bwp");
    }

    #[test]
    fn detect_unknown() {
        assert_eq!(detect_format(Path::new("a.dxf")), "unknown");
    }

    #[test]
    fn unsupported_format_is_an_error() {
        let error = import_by_extension(Path::new("a.dxf")).unwrap_err();
        assert!(matches!(error, BeamError::Unsupported(_)));
    }

    #[test]
    fn import_svg_by_extension() {
        let path = temp_dir().join("beamwright_format_test.svg");
        std::fs::write(
            &path,
            r#"<svg width="10" height="10"><rect x="0" y="0" width="1" height="1"/></svg>"#,
        )
        .unwrap();
        let document = import_by_extension(&path).unwrap();
        assert_eq!(document.objects.len(), 1);
        let _ = std::fs::remove_file(&path);
    }
}
