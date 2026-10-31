use std::fs;
use std::path::Path;

use crate::document::Document;
use crate::document::model::CURRENT_FORMAT_VERSION;
use crate::errors::{BeamError, Result};

/// Current supported project format version.
pub const PROJECT_FORMAT_VERSION: u32 = CURRENT_FORMAT_VERSION;

pub fn to_json(doc: &Document) -> Result<String> {
    validate(doc)?;
    Ok(serde_json::to_string_pretty(doc)?)
}

pub fn from_json(s: &str) -> Result<Document> {
    let doc: Document = serde_json::from_str(s)?;
    if doc.format_version != PROJECT_FORMAT_VERSION {
        return Err(BeamError::InvalidInput(format!(
            "unsupported project format version {} (expected {})",
            doc.format_version, PROJECT_FORMAT_VERSION
        )));
    }
    validate(&doc)?;
    Ok(doc)
}

pub fn validate(doc: &Document) -> Result<()> {
    doc.validate().map_err(BeamError::InvalidInput)
}

/// Backup file used by [`save_to_file`], next to the project.
pub fn backup_path(path: &Path) -> std::path::PathBuf {
    let mut name = path.file_name().map_or_else(
        || std::ffi::OsString::from("project.bwp"),
        std::ffi::OsString::from,
    );
    name.push(".bak");
    path.with_file_name(name)
}

/// Save a document to `path` atomically (temp file + rename).
///
/// The previous file is copied to `<name>.bak` before it is replaced, so an
/// overwritten project can always be recovered.
pub fn save_to_file(doc: &Document, path: &Path) -> Result<()> {
    validate(doc)?;
    let json = to_json(doc)?;
    if path.is_file() {
        fs::copy(path, backup_path(path))?;
    }
    let tmp = path.with_extension("bwp.tmp");
    fs::write(&tmp, json)?;
    // Replace atomically on the same filesystem. On Windows `rename` refuses to
    // overwrite, so fall back to remove-then-rename and clean up on failure.
    match fs::rename(&tmp, path) {
        Ok(()) => Ok(()),
        Err(_) if path.exists() => {
            fs::remove_file(path)?;
            fs::rename(&tmp, path).inspect_err(|_| {
                let _ = fs::remove_file(&tmp);
            })?;
            Ok(())
        }
        Err(error) => {
            let _ = fs::remove_file(&tmp);
            Err(error.into())
        }
    }
}

/// Load a document from `path`.
pub fn load_from_file(path: &Path) -> Result<Document> {
    let s = fs::read_to_string(path)?;
    from_json(&s)
}

/// Load a project, falling back to its backup when the main file is unreadable.
///
/// Returns the document and a human-readable note when recovery was used.
pub fn load_with_recovery(path: &Path) -> Result<(Document, Option<String>)> {
    match load_from_file(path) {
        Ok(document) => Ok((document, None)),
        Err(primary) => {
            let backup = backup_path(path);
            match load_from_file(&backup) {
                Ok(document) => Ok((
                    document,
                    Some(format!(
                        "recovered from {} after: {primary}",
                        backup.display()
                    )),
                )),
                Err(_) => Err(primary),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::model::Document;
    use crate::document::shape::Shape;

    fn sample() -> Document {
        Document::new("Test", 100.0, 50.0)
    }

    #[test]
    fn round_trip_json() {
        let doc = sample();
        let json = to_json(&doc).unwrap();
        let back = from_json(&json).unwrap();
        assert_eq!(back.name, doc.name);
        assert_eq!(back.width_mm, doc.width_mm);
        assert_eq!(back.layers.len(), doc.layers.len());
    }

    #[test]
    fn geometry_survives_round_trip() {
        let mut doc = sample();
        let layer_id = doc.layers[0].id.clone();
        let object = crate::document::model::Object::new(
            layer_id,
            Shape::Rect {
                width: 10.0,
                height: 5.0,
                corner_radius: 0.0,
            },
        );
        let id = object.id.clone();
        doc.add_object(object);
        let back = from_json(&to_json(&doc).unwrap()).unwrap();
        let restored = back.object(&id).unwrap();
        assert_eq!(restored.shape, doc.object(&id).unwrap().shape);
    }

    #[test]
    fn group_membership_survives_round_trip() {
        let mut doc = sample();
        let layer_id = doc.layers[0].id.clone();
        let mut first = crate::document::model::Object::new(
            layer_id.clone(),
            Shape::Rect {
                width: 10.0,
                height: 5.0,
                corner_radius: 0.0,
            },
        );
        first.group_id = Some("group-1".to_string());
        let mut second =
            crate::document::model::Object::new(layer_id, Shape::Ellipse { rx: 2.0, ry: 2.0 });
        second.group_id = Some("group-1".to_string());
        let ids = (first.id.clone(), second.id.clone());
        doc.add_object(first);
        doc.add_object(second);
        let back = from_json(&to_json(&doc).unwrap()).unwrap();
        assert_eq!(
            back.object(&ids.0).unwrap().group_id.as_deref(),
            Some("group-1")
        );
        assert_eq!(
            back.object(&ids.1).unwrap().group_id.as_deref(),
            Some("group-1")
        );
    }

    #[test]
    fn save_keeps_the_previous_version_as_backup() {
        let path = std::env::temp_dir().join("bw_backup_roundtrip.bwp");
        let backup = backup_path(&path);
        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(&backup);
        let mut first = sample();
        first.name = "First".to_string();
        save_to_file(&first, &path).unwrap();
        assert!(!backup.exists());
        let mut second = sample();
        second.name = "Second".to_string();
        save_to_file(&second, &path).unwrap();
        assert!(backup.exists());
        assert_eq!(load_from_file(&backup).unwrap().name, "First");
        assert_eq!(load_from_file(&path).unwrap().name, "Second");
        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(&backup);
    }

    #[test]
    fn recovery_uses_the_backup_when_the_project_is_corrupt() {
        let path = std::env::temp_dir().join("bw_recovery.bwp");
        let backup = backup_path(&path);
        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(&backup);
        let document = sample();
        save_to_file(&document, &path).unwrap();
        // Creating the backup requires a second save.
        save_to_file(&document, &path).unwrap();
        fs::write(&path, "{broken").unwrap();
        let (recovered, note) = load_with_recovery(&path).unwrap();
        assert_eq!(recovered.name, document.name);
        assert!(note.unwrap().contains("recovered"));
        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(&backup);
    }

    #[test]
    fn recovery_reports_the_original_error_without_a_backup() {
        let path = std::env::temp_dir().join("bw_recovery_missing.bwp");
        let backup = backup_path(&path);
        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(&backup);
        fs::write(&path, "{broken").unwrap();
        assert!(load_with_recovery(&path).is_err());
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn reject_empty_name() {
        let mut doc = sample();
        doc.name = "   ".to_string();
        assert!(to_json(&doc).is_err());
    }

    #[test]
    fn reject_wrong_version() {
        let mut doc = sample();
        doc.format_version = 99;
        let json = serde_json::to_string(&doc).unwrap();
        assert!(from_json(&json).is_err());
    }

    #[test]
    fn reject_dangling_layer_reference() {
        let mut doc = sample();
        let object =
            crate::document::model::Object::new("missing", Shape::Ellipse { rx: 1.0, ry: 1.0 });
        doc.objects.push(object);
        assert!(to_json(&doc).is_err());
    }
}
