use std::fs;
use std::path::Path;

use crate::document::Document;
use crate::errors::{BeamError, Result};

/// Current supported project format version.
pub const PROJECT_FORMAT_VERSION: u32 = 1;

pub fn to_json(doc: &Document) -> Result<String> {
    validate(doc)?;
    Ok(serde_json::to_string_pretty(doc)?)
}

pub fn from_json(s: &str) -> Result<Document> {
    let doc: Document = serde_json::from_str(s)?;
    validate(&doc)?;
    if doc.format_version != PROJECT_FORMAT_VERSION {
        return Err(BeamError::InvalidInput(format!(
            "unsupported project format version {} (expected {})",
            doc.format_version, PROJECT_FORMAT_VERSION
        )));
    }
    Ok(doc)
}

pub fn validate(doc: &Document) -> Result<()> {
    doc.validate().map_err(BeamError::InvalidInput)
}

/// Save a document to `path` atomically (temp file + rename).
pub fn save_to_file(doc: &Document, path: &Path) -> Result<()> {
    validate(doc)?;
    let json = to_json(doc)?;
    let tmp = path.with_extension("bwp.tmp");
    fs::write(&tmp, json)?;
    // Atomic replace on same filesystem.
    fs::rename(&tmp, path)?;
    Ok(())
}

/// Load a document from `path`.
pub fn load_from_file(path: &Path) -> Result<Document> {
    let s = fs::read_to_string(path)?;
    from_json(&s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::model::Document;
    use crate::document::units::Unit;

    fn sample() -> Document {
        Document {
            name: "Test".to_string(),
            unit: Unit::Millimeters,
            width_mm: 100.0,
            height_mm: 50.0,
            origin: crate::document::model::Origin::BottomLeft,
            layers: vec![crate::layers::model::Layer::default()],
            objects: Vec::new(),
            format_version: PROJECT_FORMAT_VERSION,
        }
    }

    #[test]
    fn round_trip_json() {
        let d = sample();
        let j = to_json(&d).unwrap();
        let back = from_json(&j).unwrap();
        assert_eq!(back.name, d.name);
        assert_eq!(back.width_mm, d.width_mm);
    }

    #[test]
    fn reject_empty_name() {
        let mut d = sample();
        d.name = "   ".to_string();
        assert!(to_json(&d).is_err());
    }

    #[test]
    fn reject_bad_dimensions() {
        let mut d = sample();
        d.width_mm = 0.0;
        assert!(validate(&d).is_err());
        d.width_mm = -1.0;
        assert!(validate(&d).is_err());
        d.width_mm = f64::NAN;
        assert!(validate(&d).is_err());
    }

    #[test]
    fn reject_wrong_version() {
        let mut d = sample();
        d.format_version = 999;
        let j = serde_json::to_string(&d).unwrap();
        assert!(from_json(&j).is_err());
    }

    #[test]
    fn save_load_file_round_trip() {
        let d = sample();
        let dir = std::env::temp_dir();
        let path = dir.join("beamwright_test_project.bwp");
        save_to_file(&d, &path).unwrap();
        let back = load_from_file(&path).unwrap();
        assert_eq!(back.name, d.name);
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn load_missing_file_errors() {
        let path = std::env::temp_dir().join("beamwright_does_not_exist.bwp");
        let _ = fs::remove_file(&path);
        assert!(load_from_file(&path).is_err());
    }
}
