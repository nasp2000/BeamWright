use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::geometry::{Path, Point2, Transform2};
use crate::layers::Layer;
use crate::operations::Operation;

pub use super::shape::{Shape, Style};
use super::units::Unit;

/// Current native project schema version.
pub const CURRENT_FORMAT_VERSION: u32 = 2;

/// Coordinate origin convention for the document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Origin {
    #[default]
    BottomLeft,
    TopLeft,
}

/// A drawable object. Geometry is stored in the object's local coordinates and
/// placed on the canvas by `transform`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Object {
    pub id: String,
    pub name: String,
    pub layer_id: String,
    pub transform: Transform2,
    pub shape: Shape,
    pub style: Style,
    pub operation_id: Option<String>,
    /// Objects grouped together share this identifier.
    #[serde(default)]
    pub group_id: Option<String>,
    pub visible: bool,
    pub locked: bool,
}

impl Object {
    pub fn new(layer_id: impl Into<String>, shape: Shape) -> Self {
        let kind = shape.kind_name().to_string();
        Self {
            id: Uuid::new_v4().to_string(),
            name: kind,
            layer_id: layer_id.into(),
            transform: Transform2::identity(),
            shape,
            style: Style::default(),
            operation_id: None,
            group_id: None,
            visible: true,
            locked: false,
        }
    }

    pub fn with_transform(mut self, transform: Transform2) -> Self {
        self.transform = transform;
        self
    }

    pub fn with_style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    pub fn world_path(&self) -> Path {
        self.shape.to_path().transform(&self.transform)
    }

    pub fn bounds(&self) -> Option<(Point2, Point2)> {
        self.shape.transformed_bounds(&self.transform)
    }

    pub fn translate(&mut self, dx: f64, dy: f64) {
        self.transform = Transform2::translate(dx, dy).compose(&self.transform);
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.id.trim().is_empty() {
            return Err("object id must not be empty".to_string());
        }
        if self.layer_id.trim().is_empty() {
            return Err("object layer id must not be empty".to_string());
        }
        if !self.transform.is_finite() {
            return Err(format!("object {} has a non-finite transform", self.id));
        }
        self.shape
            .validate()
            .map_err(|e| format!("object {}: {e}", self.id))?;
        self.style
            .validate()
            .map_err(|e| format!("object {}: {e}", self.id))?;
        if let Some(operation_id) = &self.operation_id {
            if operation_id.trim().is_empty() {
                return Err(format!("object {} has an empty operation id", self.id));
            }
        }
        Ok(())
    }
}

/// A rectangle in document millimetres.
///
/// Used for the material area a job is clipped to and, from the same type, for
/// areas the job must avoid.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MaterialZone {
    pub x_mm: f64,
    pub y_mm: f64,
    pub width_mm: f64,
    pub height_mm: f64,
}

impl MaterialZone {
    pub fn new(x_mm: f64, y_mm: f64, width_mm: f64, height_mm: f64) -> Self {
        Self {
            x_mm,
            y_mm,
            width_mm,
            height_mm,
        }
    }

    pub fn max_x(self) -> f64 {
        self.x_mm + self.width_mm
    }

    pub fn max_y(self) -> f64 {
        self.y_mm + self.height_mm
    }

    pub fn contains(self, point: crate::geometry::Point2) -> bool {
        point.x >= self.x_mm
            && point.x <= self.max_x()
            && point.y >= self.y_mm
            && point.y <= self.max_y()
    }

    pub fn validate(self) -> Result<(), String> {
        if !self.x_mm.is_finite() || !self.y_mm.is_finite() {
            return Err("material area position must be finite".to_string());
        }
        if !self.width_mm.is_finite()
            || !self.height_mm.is_finite()
            || self.width_mm <= 0.0
            || self.height_mm <= 0.0
        {
            return Err("material area must have a positive size".to_string());
        }
        Ok(())
    }
}

/// A horizontal or vertical guide line, in document millimetres.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Guide {
    pub id: String,
    pub horizontal: bool,
    pub position: f64,
}

impl Guide {
    pub fn new(horizontal: bool, position: f64) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            horizontal,
            position,
        }
    }
}

/// The main document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Document {
    pub name: String,
    pub unit: Unit,
    pub width_mm: f64,
    pub height_mm: f64,
    pub origin: Origin,
    pub layers: Vec<Layer>,
    pub objects: Vec<Object>,
    #[serde(default)]
    pub operations: Vec<Operation>,
    /// Guide lines drawn over the page and used by snapping.
    #[serde(default)]
    pub guides: Vec<Guide>,
    /// Material rectangle; when set, jobs are clipped to it.
    #[serde(default)]
    pub stock: Option<MaterialZone>,
    /// Areas a job must not enter; cuts that reach them are removed.
    #[serde(default)]
    pub restricted_areas: Vec<MaterialZone>,
    pub format_version: u32,
}

/// What [`Document::collect_images`] did.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CollectReport {
    /// Original path and the path it was copied to.
    pub collected: Vec<(String, String)>,
    /// Image paths that no longer exist.
    pub missing: Vec<String>,
}

impl Document {
    /// Copy every image referenced by the project into `destination`.
    ///
    /// Paths are rewritten to the copied files, so moving the project and its
    /// folder keeps it usable. Images that are missing are reported and left
    /// untouched. Returns the mapping and the missing paths.
    pub fn collect_images(
        &mut self,
        destination: &std::path::Path,
    ) -> std::io::Result<CollectReport> {
        use std::collections::HashMap;

        std::fs::create_dir_all(destination)?;
        let mut mapping: HashMap<String, String> = HashMap::new();
        let mut report = CollectReport::default();
        for object in &self.objects {
            let Shape::Image(image) = &object.shape else {
                continue;
            };
            if mapping.contains_key(&image.path) {
                continue;
            }
            let source = std::path::Path::new(&image.path);
            if !source.is_file() {
                report.missing.push(image.path.clone());
                continue;
            }
            let stem = source
                .file_stem()
                .and_then(|value| value.to_str())
                .unwrap_or("image");
            let extension = source
                .extension()
                .and_then(|value| value.to_str())
                .unwrap_or("png");
            let sanitized: String = stem
                .chars()
                .map(|character| {
                    if character.is_ascii_alphanumeric() || character == '-' || character == '_' {
                        character
                    } else {
                        '_'
                    }
                })
                .collect();
            let mut name = format!("{sanitized}.{extension}");
            let mut counter = 2;
            while destination.join(&name).exists() {
                name = format!("{sanitized}-{counter}.{extension}");
                counter += 1;
            }
            let target = destination.join(&name);
            std::fs::copy(source, &target)?;
            let target_text = target.to_string_lossy().to_string();
            mapping.insert(image.path.clone(), target_text.clone());
            report.collected.push((image.path.clone(), target_text));
        }
        for object in &mut self.objects {
            if let Shape::Image(image) = &mut object.shape {
                if let Some(target) = mapping.get(&image.path) {
                    image.path.clone_from(target);
                }
            }
        }
        Ok(report)
    }

    /// Validate the optional material area and every restricted area.
    fn validate_material_limits(&self) -> Result<(), String> {
        if let Some(stock) = self.stock {
            stock
                .validate()
                .map_err(|error| format!("material area: {error}"))?;
        }
        for area in &self.restricted_areas {
            area.validate()
                .map_err(|error| format!("restricted area: {error}"))?;
        }
        Ok(())
    }

    /// Clone this document keeping only the objects whose identifiers are listed.
    ///
    /// Layers and operations are preserved so styles and assignments stay valid.
    pub fn subset(&self, ids: &[String]) -> Document {
        let mut subset = self.clone();
        subset
            .objects
            .retain(|object| ids.iter().any(|id| id == &object.id));
        subset
    }
}

impl Default for Document {
    fn default() -> Self {
        Self {
            name: "Untitled".to_string(),
            unit: Unit::Millimeters,
            width_mm: 300.0,
            height_mm: 200.0,
            origin: Origin::BottomLeft,
            layers: vec![Layer::new("Layer 1").with_color("#000000")],
            objects: Vec::new(),
            operations: Vec::new(),
            guides: Vec::new(),
            stock: None,
            restricted_areas: Vec::new(),
            format_version: CURRENT_FORMAT_VERSION,
        }
    }
}

impl Document {
    pub fn new(name: impl Into<String>, width_mm: f64, height_mm: f64) -> Self {
        Self {
            name: name.into(),
            width_mm,
            height_mm,
            ..Self::default()
        }
    }

    fn validate_guides(&self) -> Result<(), String> {
        let mut guide_ids = std::collections::HashSet::new();
        for guide in &self.guides {
            if !guide.position.is_finite() {
                return Err("guide position must be finite".to_string());
            }
            if !guide_ids.insert(guide.id.as_str()) {
                return Err(format!("duplicate guide id: {}", guide.id));
            }
        }
        Ok(())
    }

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
        self.validate_material_limits()?;
        if self.layers.is_empty() {
            return Err("document must have at least one layer".to_string());
        }

        let mut layer_ids = std::collections::HashSet::new();
        for layer in &self.layers {
            layer.validate()?;
            if !layer_ids.insert(layer.id.as_str()) {
                return Err(format!("duplicate layer id: {}", layer.id));
            }
        }

        let mut operation_ids = std::collections::HashSet::new();
        for operation in &self.operations {
            operation.validate()?;
            if !operation_ids.insert(operation.id.as_str()) {
                return Err(format!("duplicate operation id: {}", operation.id));
            }
        }

        for operation in &self.operations {
            for dependency in &operation.depends_on {
                if !operation_ids.contains(dependency.as_str()) {
                    return Err(format!(
                        "operation {} depends on missing operation {dependency}",
                        operation.id
                    ));
                }
            }
        }
        // Kahn's algorithm: every operation must become ready, otherwise the
        // dependency graph contains a cycle.
        let mut pending: std::collections::HashMap<&str, usize> = self
            .operations
            .iter()
            .map(|operation| (operation.id.as_str(), operation.depends_on.len()))
            .collect();
        let mut ready: std::collections::VecDeque<&str> = self
            .operations
            .iter()
            .filter(|operation| operation.depends_on.is_empty())
            .map(|operation| operation.id.as_str())
            .collect();
        let mut resolved = 0;
        while let Some(id) = ready.pop_front() {
            resolved += 1;
            for operation in &self.operations {
                if operation
                    .depends_on
                    .iter()
                    .any(|dependency| dependency == id)
                {
                    if let Some(count) = pending.get_mut(operation.id.as_str()) {
                        *count -= 1;
                        if *count == 0 {
                            ready.push_back(operation.id.as_str());
                        }
                    }
                }
            }
        }
        if resolved < self.operations.len() {
            return Err("operation dependencies contain a cycle".to_string());
        }

        self.validate_guides()?;

        let mut object_ids = std::collections::HashSet::new();
        for object in &self.objects {
            object.validate()?;
            if !object_ids.insert(object.id.as_str()) {
                return Err(format!("duplicate object id: {}", object.id));
            }
            if !layer_ids.contains(object.layer_id.as_str()) {
                return Err(format!(
                    "object {} references missing layer {}",
                    object.id, object.layer_id
                ));
            }
            if let Some(operation_id) = &object.operation_id {
                if !operation_ids.contains(operation_id.as_str()) {
                    return Err(format!(
                        "object {} references missing operation {}",
                        object.id, operation_id
                    ));
                }
            }
        }
        Ok(())
    }

    pub fn add_guide(&mut self, horizontal: bool, position: f64) -> &Guide {
        self.guides.push(Guide::new(horizontal, position));
        self.guides.last().expect("guide was just pushed")
    }

    pub fn remove_guide(&mut self, id: &str) -> bool {
        let before = self.guides.len();
        self.guides.retain(|guide| guide.id != id);
        self.guides.len() != before
    }

    pub fn add_layer(&mut self, name: impl Into<String>) -> &Layer {
        let order = self.layers.iter().map(|l| l.order).max().unwrap_or(0) + 1;
        let mut layer = Layer::new(name);
        layer.order = order;
        self.layers.push(layer);
        self.layers.last().expect("layer was just pushed")
    }

    pub fn layer(&self, id: &str) -> Option<&Layer> {
        self.layers.iter().find(|l| l.id == id)
    }

    pub fn layer_mut(&mut self, id: &str) -> Option<&mut Layer> {
        self.layers.iter_mut().find(|l| l.id == id)
    }

    pub fn add_object(&mut self, object: Object) -> &Object {
        self.objects.push(object);
        self.objects.last().expect("object was just pushed")
    }

    pub fn object(&self, id: &str) -> Option<&Object> {
        self.objects.iter().find(|o| o.id == id)
    }

    pub fn object_mut(&mut self, id: &str) -> Option<&mut Object> {
        self.objects.iter_mut().find(|o| o.id == id)
    }

    pub fn remove_object(&mut self, id: &str) -> Option<Object> {
        let index = self.objects.iter().position(|o| o.id == id)?;
        Some(self.objects.remove(index))
    }

    pub fn remove_layer(&mut self, id: &str) -> Result<(), String> {
        if self.layers.len() <= 1 {
            return Err("the document must keep at least one layer".to_string());
        }
        if self.objects.iter().any(|o| o.layer_id == id) {
            return Err("cannot remove a layer that still contains objects".to_string());
        }
        let index = self
            .layers
            .iter()
            .position(|l| l.id == id)
            .ok_or_else(|| format!("unknown layer: {id}"))?;
        self.layers.remove(index);
        Ok(())
    }

    pub fn objects_on_layer(&self, layer_id: &str) -> Vec<&Object> {
        self.objects
            .iter()
            .filter(|o| o.layer_id == layer_id)
            .collect()
    }

    pub fn layer_of(&self, object_id: &str) -> Option<&str> {
        self.object(object_id).map(|o| o.layer_id.as_str())
    }

    pub fn assign_operation(&mut self, object_id: &str, operation_id: Option<String>) {
        if let Some(object) = self.object_mut(object_id) {
            object.operation_id = operation_id;
        }
    }

    /// Bounding box of every visible object, in document coordinates.
    pub fn bounds(&self) -> Option<(Point2, Point2)> {
        let mut bounds: Option<(Point2, Point2)> = None;
        for object in &self.objects {
            if !object.visible {
                continue;
            }
            let Some((min, max)) = object.bounds() else {
                continue;
            };
            bounds = Some(match bounds {
                None => (min, max),
                Some((bmin, bmax)) => (
                    Point2::new(bmin.x.min(min.x), bmin.y.min(min.y)),
                    Point2::new(bmax.x.max(max.x), bmax.y.max(max.y)),
                ),
            });
        }
        bounds
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::shape::Shape;

    fn rect_object(layer: &str) -> Object {
        Object::new(
            layer,
            Shape::Rect {
                width: 10.0,
                height: 5.0,
                corner_radius: 0.0,
            },
        )
    }

    #[test]
    fn images_are_collected_into_a_folder() {
        let root = std::env::temp_dir().join("bw_collect_images");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("a")).unwrap();
        std::fs::create_dir_all(root.join("b")).unwrap();
        std::fs::write(root.join("a/photo.png"), b"one").unwrap();
        std::fs::write(root.join("b/photo.png"), b"two").unwrap();
        let mut document = Document::default();
        let layer = document.layers[0].id.clone();
        for path in [
            root.join("a/photo.png"),
            root.join("a/photo.png"),
            root.join("b/photo.png"),
            root.join("missing.png"),
        ] {
            let image = crate::objects::ImageObject::new(path.to_string_lossy(), 4, 4, 96.0);
            document.add_object(Object::new(layer.clone(), Shape::Image(image)));
        }
        let destination = root.join("assets");
        let report = document.collect_images(&destination).unwrap();
        assert_eq!(report.collected.len(), 2);
        assert_eq!(report.missing.len(), 1);
        assert!(destination.join("photo.png").is_file());
        assert!(destination.join("photo-2.png").is_file());
        for object in &document.objects {
            if let Shape::Image(image) = &object.shape {
                if !image.path.ends_with("missing.png") {
                    assert!(image.path.contains("assets"));
                }
            }
        }
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn the_material_area_round_trips_and_validates() {
        let mut document = Document::default();
        assert!(document.stock.is_none());
        document.stock = Some(MaterialZone::new(10.0, 20.0, 100.0, 80.0));
        assert!(document.validate().is_ok());
        let json = serde_json::to_string(&document).unwrap();
        let loaded: Document = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.stock, document.stock);

        // Projects written before the material area existed load without one.
        let old = r#"{"name":"Old","unit":"Millimeters","width_mm":300,"height_mm":200,
            "origin":"BottomLeft","layers":[],"objects":[],"format_version":2}"#;
        let loaded: Document = serde_json::from_str(old).unwrap();
        assert!(loaded.stock.is_none());

        for zone in [
            MaterialZone::new(0.0, 0.0, 0.0, 10.0),
            MaterialZone::new(0.0, 0.0, 10.0, -1.0),
            MaterialZone::new(f64::NAN, 0.0, 10.0, 10.0),
        ] {
            document.stock = Some(zone);
            assert!(document.validate().is_err(), "{zone:?} was accepted");
        }
    }

    #[test]
    fn restricted_areas_round_trip_and_validate() {
        let mut document = Document {
            restricted_areas: vec![MaterialZone::new(1.0, 2.0, 3.0, 4.0)],
            ..Document::default()
        };
        assert!(document.validate().is_ok());
        let json = serde_json::to_string(&document).unwrap();
        let loaded: Document = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.restricted_areas, document.restricted_areas);

        document.restricted_areas[0].width_mm = 0.0;
        assert!(document.validate().is_err());
    }

    #[test]
    fn a_material_area_contains_its_edges() {
        let zone = MaterialZone::new(0.0, 0.0, 10.0, 5.0);
        assert!(zone.contains(crate::geometry::Point2::new(0.0, 0.0)));
        assert!(zone.contains(crate::geometry::Point2::new(10.0, 5.0)));
        assert!(!zone.contains(crate::geometry::Point2::new(10.1, 2.0)));
        assert_eq!(zone.max_x(), 10.0);
        assert_eq!(zone.max_y(), 5.0);
    }

    #[test]
    fn guides_round_trip_and_validate() {
        let mut document = Document::default();
        document.add_guide(true, 25.0);
        document.add_guide(false, 10.0);
        assert!(document.validate().is_ok());
        let json = serde_json::to_string(&document).unwrap();
        let loaded: Document = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.guides, document.guides);

        document.guides[0].position = f64::NAN;
        assert!(document.validate().is_err());
        document.guides[0].position = 1.0;
        document.guides[1].id = document.guides[0].id.clone();
        assert!(document.validate().is_err());
        document.guides[1].id = "other-guide".to_string();
        let id = document.guides[0].id.clone();
        assert!(document.remove_guide(&id));
        assert_eq!(document.guides.len(), 1);
    }

    #[test]
    fn operation_dependencies_are_validated() {
        let mut document = Document::default();
        let first = Operation::new("First", crate::operations::OperationType::Line);
        let mut second = Operation::new("Second", crate::operations::OperationType::Fill);
        let first_id = first.id.clone();
        let second_id = second.id.clone();
        second.depends_on = vec![first_id];
        document.operations.push(first);
        document.operations.push(second);
        assert!(document.validate().is_ok());

        document.operations[1].depends_on = vec!["missing".to_string()];
        assert!(
            document
                .validate()
                .unwrap_err()
                .contains("missing operation")
        );

        document.operations[1].depends_on = vec![second_id];
        assert!(document.validate().is_err());

        document.operations[0].depends_on = vec![document.operations[1].id.clone()];
        document.operations[1].depends_on = vec![document.operations[0].id.clone()];
        let error = document.validate().unwrap_err();
        assert!(error.contains("cycle"), "{error}");
    }

    #[test]
    fn default_document_valid() {
        let doc = Document::default();
        assert!(doc.validate().is_ok());
        assert_eq!(doc.layers.len(), 1);
        assert_eq!(doc.format_version, CURRENT_FORMAT_VERSION);
    }

    #[test]
    fn reject_empty_name() {
        let doc = Document {
            name: "  ".to_string(),
            ..Document::default()
        };
        assert!(doc.validate().is_err());
    }

    #[test]
    fn reject_bad_dimensions() {
        for width in [0.0_f64, -5.0, f64::NAN] {
            let doc = Document {
                width_mm: width,
                ..Document::default()
            };
            assert!(doc.validate().is_err());
        }
    }

    #[test]
    fn object_layer_must_exist() {
        let mut doc = Document::default();
        let layer_id = doc.layers[0].id.clone();
        doc.objects.push(rect_object(&layer_id));
        assert!(doc.validate().is_ok());
        doc.objects[0].layer_id = "missing".to_string();
        assert!(doc.validate().is_err());
    }

    #[test]
    fn duplicate_object_ids_rejected() {
        let mut doc = Document::default();
        let layer_id = doc.layers[0].id.clone();
        let first = rect_object(&layer_id);
        let mut second = first.clone();
        second.id = first.id.clone();
        doc.objects.push(first);
        doc.objects.push(second);
        assert!(doc.validate().is_err());
    }

    #[test]
    fn add_and_remove_object() {
        let mut doc = Document::default();
        let layer_id = doc.layers[0].id.clone();
        let object = rect_object(&layer_id);
        let id = object.id.clone();
        doc.add_object(object);
        assert!(doc.object(&id).is_some());
        assert_eq!(doc.objects_on_layer(&layer_id).len(), 1);
        assert!(doc.remove_object(&id).is_some());
        assert!(doc.object(&id).is_none());
    }

    #[test]
    fn translate_moves_bounds() {
        let mut doc = Document::default();
        let layer_id = doc.layers[0].id.clone();
        let object = rect_object(&layer_id);
        let id = object.id.clone();
        doc.add_object(object);
        doc.object_mut(&id).unwrap().translate(3.0, 4.0);
        let (min, _) = doc.bounds().unwrap();
        assert!((min.x - 3.0).abs() < 1e-9);
        assert!((min.y - 4.0).abs() < 1e-9);
    }

    #[test]
    fn remove_layer_with_objects_fails() {
        let mut doc = Document::default();
        let layer_id = doc.layers[0].id.clone();
        doc.add_object(rect_object(&layer_id));
        assert!(doc.remove_layer(&layer_id).is_err());
    }

    #[test]
    fn operation_reference_must_exist() {
        let mut doc = Document::default();
        let layer_id = doc.layers[0].id.clone();
        let mut object = rect_object(&layer_id);
        object.operation_id = Some("missing".to_string());
        doc.objects.push(object);
        assert!(doc.validate().is_err());
    }
}
