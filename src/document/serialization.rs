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

/// Save a document to `path` atomically (temp file + rename).
pub fn save_to_file(doc: &Document, path: &Path) -> Result<()> {
    validate(doc)?;
    let json = to_json(doc)?;
    let tmp = path.with_extension("bwp.tmp");
    fs::write(&tmp, json)?;
    // Atomic replace on the same filesystem.
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
