use serde::{Deserialize, Serialize};

use super::units::Unit;

/// Coordinate origin convention for the document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Origin {
    BottomLeft,
    TopLeft,
}

use crate::layers::model::Layer;

/// A vector object kind (PLAN.md §6). Geometry is stored as a placeholder
/// in this skeleton; concrete geometry lands in `geometry/` and `objects/`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ObjectKind {
    Line,
    Rectangle,
    Ellipse,
    Polygon,
    Path,
    Text,
    Image,
    Group,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentObject {
    pub id: String,
    pub kind: ObjectKind,
    pub layer_id: String,
    pub operation_id: Option<String>,
}

/// The main document (PLAN.md §4.1).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub name: String,
    pub unit: Unit,
    pub width_mm: f64,
    pub height_mm: f64,
    pub origin: Origin,
    pub layers: Vec<Layer>,
    pub objects: Vec<DocumentObject>,
    pub format_version: u32,
}

impl Default for Document {
    fn default() -> Self {
        Self {
            name: "Untitled".to_string(),
            unit: Unit::Millimeters,
            width_mm: 300.0,
            height_mm: 200.0,
            origin: Origin::BottomLeft,
            layers: vec![Layer::default()],
            objects: Vec::new(),
            format_version: 1,
        }
    }
}

impl Document {
    pub fn validate(&self) -> Result<(), String> {
        if self.name.trim().is_empty() {
            return Err("project name must not be empty".to_string());
        }
        if !self.width_mm.is_finite() || self.width_mm <= 0.0 {
            return Err("width must be positive and finite".to_string());
        }
        if !self.height_mm.is_finite() || self.height_mm <= 0.0 {
            return Err("height must be positive and finite".to_string());
        }
        if self.format_version == 0 {
            return Err("format version must be >= 1".to_string());
        }
        for l in &self.layers {
            if l.name.trim().is_empty() {
                return Err("layer name must not be empty".to_string());
            }
        }
        Ok(())
    }
}

impl Document {
    pub fn assign_operation(&mut self, object_id: &str, operation_id: Option<String>) {
        if let Some(o) = self.objects.iter_mut().find(|o| o.id == object_id) {
            o.operation_id = operation_id;
        }
    }
    pub fn objects_on_layer(&self, layer_id: &str) -> Vec<&DocumentObject> {
        self.objects
            .iter()
            .filter(|o| o.layer_id == layer_id)
            .collect()
    }
    pub fn layer_of(&self, object_id: &str) -> Option<&str> {
        self.objects
            .iter()
            .find(|o| o.id == object_id)
            .map(|o| o.layer_id.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_document_valid() {
        let d = Document::default();
        assert!(d.validate().is_ok());
        assert_eq!(d.layers.len(), 1);
    }

    #[test]
    fn reject_empty_name() {
        let d = Document {
            name: "  ".to_string(),
            ..Document::default()
        };
        assert!(d.validate().is_err());
    }

    #[test]
    fn reject_bad_dimensions() {
        for w in [0.0_f64, -5.0, f64::NAN] {
            let d = Document {
                width_mm: w,
                ..Document::default()
            };
            assert!(d.validate().is_err());
        }
    }

    #[test]
    fn reject_empty_layer_name() {
        let layer = Layer {
            name: String::new(),
            ..Layer::default()
        };
        let d = Document {
            layers: vec![layer],
            ..Document::default()
        };
        assert!(d.validate().is_err());
    }

    #[test]
    fn reject_zero_format_version() {
        let d = Document {
            format_version: 0,
            ..Document::default()
        };
        assert!(d.validate().is_err());
    }
}

#[cfg(test)]
mod assoc_tests {
    use super::*;
    use crate::document::model::DocumentObject;

    #[test]
    fn assign_operation() {
        let mut d = Document::default();
        d.layers[0].id = "l1".to_string();
        d.objects.push(DocumentObject {
            id: "o1".to_string(),
            kind: ObjectKind::Line,
            layer_id: "l1".to_string(),
            operation_id: None,
        });
        d.assign_operation("o1", Some("op-1".to_string()));
        assert_eq!(d.objects[0].operation_id, Some("op-1".to_string()));
    }

    #[test]
    fn objects_on_layer() {
        let mut d = Document::default();
        d.layers[0].id = "l1".to_string();
        d.objects.push(DocumentObject {
            id: "o1".to_string(),
            kind: ObjectKind::Line,
            layer_id: "l1".to_string(),
            operation_id: None,
        });
        d.objects.push(DocumentObject {
            id: "o2".to_string(),
            kind: ObjectKind::Line,
            layer_id: "l2".to_string(),
            operation_id: None,
        });
        assert_eq!(d.objects_on_layer("l1").len(), 1);
        assert_eq!(d.objects_on_layer("l2").len(), 1);
    }

    #[test]
    fn layer_of() {
        let mut d = Document::default();
        d.layers[0].id = "l1".to_string();
        d.objects.push(DocumentObject {
            id: "o1".to_string(),
            kind: ObjectKind::Line,
            layer_id: "l1".to_string(),
            operation_id: None,
        });
        assert_eq!(d.layer_of("o1"), Some("l1"));
        assert!(d.layer_of("nope").is_none());
    }
}
