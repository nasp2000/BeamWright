use std::path::Path;

use crate::document::Document;
use crate::errors::{BeamError, Result};

/// Classify a path by the formats the application can import (§13).
pub fn detect_format(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_lowercase)
        .as_deref()
    {
        Some("svg") => "svg",
        Some("dxf") => "dxf",
        Some("bwp") => "bwp",
        _ => "unknown",
    }
}

/// The document produced by an import, with elements that were skipped.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportOutcome {
    pub document: Document,
    pub warnings: Vec<String>,
}

/// Import a file, choosing the parser from its extension.
pub fn import_with_report(path: &Path) -> Result<ImportOutcome> {
    match detect_format(path) {
        "svg" => {
            let source = std::fs::read_to_string(path)?;
            let report = crate::import::svg::import_svg_report(&source)?;
            Ok(ImportOutcome {
                document: report.document,
                warnings: report.warnings,
            })
        }
        "dxf" => {
            let source = std::fs::read_to_string(path)?;
            let report = crate::import::dxf::import_dxf_report(&source)?;
            Ok(ImportOutcome {
                document: report.document,
                warnings: report.warnings,
            })
        }
        "bwp" => Ok(ImportOutcome {
            document: crate::document::serialization::load_from_file(path)?,
            warnings: Vec::new(),
        }),
        other => Err(BeamError::Unsupported(format!(
            "cannot import '{other}' files; supported formats: .svg, .dxf and .bwp"
        ))),
    }
}

/// Import a file and discard the warning list.
pub fn import_by_extension(path: &Path) -> Result<Document> {
    Ok(import_with_report(path)?.document)
}

/// Open a native project or an importable vector file.
pub fn open_document(path: &Path) -> Result<Document> {
    import_by_extension(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env::temp_dir;

    #[test]
    fn detects_formats() {
        assert_eq!(detect_format(Path::new("a.svg")), "svg");
        assert_eq!(detect_format(Path::new("a.DXF")), "dxf");
        assert_eq!(detect_format(Path::new("a.bwp")), "bwp");
        assert_eq!(detect_format(Path::new("a.xyz")), "unknown");
    }

    #[test]
    fn unsupported_extension_is_an_error() {
        let error = import_with_report(Path::new("a.xyz")).unwrap_err();
        assert!(matches!(error, BeamError::Unsupported(_)));
    }

    #[test]
    fn imports_svg_with_report() {
        let path = temp_dir().join("bw_fmt_report.svg");
        std::fs::write(
            &path,
            "<svg width=\"10\" height=\"10\"><rect x=\"0\" y=\"0\" width=\"1\" height=\"1\"/><use href=\"#x\"/></svg>",
        )
        .unwrap();
        let outcome = import_with_report(&path).unwrap();
        assert_eq!(outcome.document.objects.len(), 1);
        assert_eq!(outcome.warnings.len(), 1);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn imports_dxf_with_report() {
        let path = temp_dir().join("bw_fmt_report.dxf");
        std::fs::write(
            &path,
            "0\nSECTION\n2\nENTITIES\n0\nLINE\n10\n0\n20\n0\n11\n10\n21\n0\n0\nENDSEC\n0\nEOF\n",
        )
        .unwrap();
        let outcome = import_with_report(&path).unwrap();
        assert_eq!(outcome.document.objects.len(), 1);
        let _ = std::fs::remove_file(&path);
    }
}
