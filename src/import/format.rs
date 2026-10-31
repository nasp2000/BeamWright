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
        Some("png" | "jpg" | "jpeg" | "bmp" | "tif" | "tiff") => "raster",
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

/// Import a file with the default raster pixel limit.
pub fn import_with_report(path: &Path) -> Result<ImportOutcome> {
    import_with_report_limited(path, crate::raster::MAX_PIXELS)
}

/// Import a file, checking `cancel` where the format allows it.
pub fn import_with_report_cancellable(
    path: &Path,
    max_pixels: u64,
    cancel: &dyn Fn() -> bool,
) -> Result<ImportOutcome> {
    if cancel() {
        return Err(BeamError::InvalidInput("cancelled".to_string()));
    }
    let outcome = match detect_format(path) {
        "svg" => {
            let source = std::fs::read_to_string(path)?;
            let report = crate::import::svg::import_svg_report_cancellable(&source, cancel)?;
            ImportOutcome {
                document: report.document,
                warnings: report.warnings,
            }
        }
        "dxf" => {
            let source = std::fs::read_to_string(path)?;
            if cancel() {
                return Err(BeamError::InvalidInput("cancelled".to_string()));
            }
            let report = crate::import::dxf::import_dxf_report(&source)?;
            ImportOutcome {
                document: report.document,
                warnings: report.warnings,
            }
        }
        "bwp" => {
            let (document, note) = crate::document::serialization::load_with_recovery(path)?;
            ImportOutcome {
                document,
                warnings: note.into_iter().collect(),
            }
        }
        "raster" => ImportOutcome {
            document: crate::import::raster::import_raster_with_limit(path, max_pixels)?,
            warnings: Vec::new(),
        },
        other => {
            return Err(BeamError::Unsupported(format!(
                "cannot import '{other}' files; supported formats: .svg, .dxf, raster images and .bwp"
            )));
        }
    };
    if cancel() {
        return Err(BeamError::InvalidInput("cancelled".to_string()));
    }
    Ok(outcome)
}

/// Import a file, choosing the parser from its extension.
///
/// `max_pixels` only affects raster images; it is the limit the preferences
/// configure.
pub fn import_with_report_limited(path: &Path, max_pixels: u64) -> Result<ImportOutcome> {
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
        "raster" => Ok(ImportOutcome {
            document: crate::import::raster::import_raster_with_limit(path, max_pixels)?,
            warnings: Vec::new(),
        }),
        "bwp" => {
            let (document, note) = crate::document::serialization::load_with_recovery(path)?;
            Ok(ImportOutcome {
                document,
                warnings: note.into_iter().collect(),
            })
        }
        other => Err(BeamError::Unsupported(format!(
            "cannot import '{other}' files; supported formats: .svg, .dxf, raster images and .bwp"
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
        assert_eq!(detect_format(Path::new("a.PNG")), "raster");
        assert_eq!(detect_format(Path::new("a.jpeg")), "raster");
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
    fn corrupt_project_recovers_from_its_backup() {
        let path = temp_dir().join("bw_fmt_recovery.bwp");
        let backup = crate::document::serialization::backup_path(&path);
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(&backup);
        let document = Document::default();
        crate::document::serialization::save_to_file(&document, &path).unwrap();
        crate::document::serialization::save_to_file(&document, &path).unwrap();
        std::fs::write(&path, "{broken").unwrap();
        let outcome = import_with_report(&path).unwrap();
        assert_eq!(outcome.warnings.len(), 1);
        assert!(outcome.warnings[0].contains("recovered"));
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(&backup);
    }

    #[test]
    fn imports_can_be_cancelled_before_and_during_parsing() {
        let path = temp_dir().join("bw_fmt_cancel.svg");
        std::fs::write(
            &path,
            "<svg width=\"10\" height=\"10\"><rect width=\"1\" height=\"1\"/></svg>",
        )
        .unwrap();
        assert!(import_with_report_cancellable(&path, 50_000_000, &|| false).is_ok());
        let error = import_with_report_cancellable(&path, 50_000_000, &|| true).unwrap_err();
        assert!(error.to_string().contains("cancelled"));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn the_raster_limit_is_forwarded() {
        let path = temp_dir().join("bw_fmt_limit.png");
        let image = image::RgbaImage::from_pixel(8, 8, image::Rgba([0, 0, 0, 255]));
        image.save(&path).unwrap();
        assert!(import_with_report_limited(&path, 10).is_err());
        assert_eq!(
            import_with_report_limited(&path, 64)
                .unwrap()
                .document
                .objects
                .len(),
            1
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn imports_a_raster_through_the_dispatcher() {
        let path = temp_dir().join("bw_fmt_raster.png");
        let image = image::RgbaImage::from_pixel(3, 2, image::Rgba([10, 20, 30, 255]));
        image.save(&path).unwrap();
        let outcome = import_with_report(&path).unwrap();
        assert_eq!(outcome.document.objects.len(), 1);
        assert_eq!(outcome.warnings.len(), 0);
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
