//! Read-only summaries for people and agents.
//!
//! [`inspect`] describes what is in a document; [`measure`] adds lengths, areas
//! and bounds. Neither changes the document, so they are safe in any session.

use std::collections::BTreeMap;

use serde_json::{Value, json};

use crate::document::{Document, Object};
use crate::geometry::{Path, Point2};

/// Rounded to a micrometre so reports read cleanly and compare well.
fn round(value: f64) -> f64 {
    if value.is_finite() {
        (value * 1000.0).round() / 1000.0
    } else {
        0.0
    }
}

/// Area of a closed path, or zero for an open one.
fn path_area(path: &Path) -> f64 {
    if !path.closed {
        return 0.0;
    }
    let points = path.flatten(0.2);
    if points.len() < 3 {
        return 0.0;
    }
    let polygon: Vec<(f64, f64)> = points.iter().map(|point| (point.x, point.y)).collect();
    crate::geometry::boolean::polygon_area(&polygon).abs()
}

fn bounds_json(bounds: (Point2, Point2)) -> Value {
    let (min, max) = bounds;
    json!({
        "min_x": round(min.x),
        "min_y": round(min.y),
        "max_x": round(max.x),
        "max_y": round(max.y),
        "width": round(max.x - min.x),
        "height": round(max.y - min.y),
    })
}

fn zone_json(zone: &crate::document::MaterialZone) -> Value {
    json!({
        "x_mm": round(zone.x_mm),
        "y_mm": round(zone.y_mm),
        "width_mm": round(zone.width_mm),
        "height_mm": round(zone.height_mm),
    })
}

/// Objects named by `ids`, or every object when it is absent.
///
/// An explicit list may include hidden objects; without one, hidden objects are
/// left out unless `include_hidden` is set.
fn select<'a>(
    document: &'a Document,
    ids: Option<&[String]>,
    include_hidden: bool,
) -> Vec<&'a Object> {
    document
        .objects
        .iter()
        .filter(|object| match ids {
            Some(ids) => ids.iter().any(|id| id == &object.id),
            None => include_hidden || object.visible,
        })
        .collect()
}

/// Describe the document: project, layers, objects, operations and parameters.
pub fn inspect(document: &Document, ids: Option<&[String]>) -> Value {
    let objects: Vec<Value> = select(document, ids, true)
        .into_iter()
        .map(|object| object_json(document, object))
        .collect();
    let layers: Vec<Value> = document
        .layers
        .iter()
        .map(|layer| {
            json!({
                "id": layer.id,
                "name": layer.name,
                "color": layer.color,
                "visible": layer.visible,
                "locked": layer.locked,
                "objects": document
                    .objects
                    .iter()
                    .filter(|object| object.layer_id == layer.id)
                    .count(),
            })
        })
        .collect();
    let operations: Vec<Value> = document
        .operations
        .iter()
        .map(|operation| {
            json!({
                "id": operation.id,
                "name": operation.name,
                "type": operation.op_type.label(),
                "enabled": operation.enabled,
                "objects": document
                    .objects
                    .iter()
                    .filter(|object| object.operation_id.as_deref() == Some(operation.id.as_str()))
                    .count(),
                "depends_on": operation.depends_on,
                "expressions": operation.params.expressions,
            })
        })
        .collect();
    let values = crate::parameters::resolved(&document.parameters);
    let parameters: Vec<Value> = document
        .parameters
        .iter()
        .zip(values)
        .map(|(parameter, (_, value))| match value {
            Ok(value) => json!({
                "name": parameter.name,
                "expression": parameter.expression,
                "unit": parameter.unit,
                "value": round(value),
            }),
            Err(error) => json!({
                "name": parameter.name,
                "expression": parameter.expression,
                "unit": parameter.unit,
                "error": error,
            }),
        })
        .collect();

    json!({
        "project": {
            "name": document.name,
            "width_mm": round(document.width_mm),
            "height_mm": round(document.height_mm),
            "unit": document.unit.label(),
            "origin": match document.origin {
                crate::document::Origin::BottomLeft => "bottom-left",
                crate::document::Origin::TopLeft => "top-left",
            },
            "format_version": document.format_version,
        },
        "counts": {
            "layers": document.layers.len(),
            "objects": document.objects.len(),
            "visible_objects": document.objects.iter().filter(|object| object.visible).count(),
            "locked_objects": document.objects.iter().filter(|object| object.locked).count(),
            "holes": document.objects.iter().filter(|object| object.hole_of.is_some()).count(),
            "operations": document.operations.len(),
            "parameters": document.parameters.len(),
            "guides": document.guides.len(),
        },
        "layers": layers,
        "objects": objects,
        "operations": operations,
        "parameters": parameters,
        "stock": document.stock.as_ref().map(zone_json),
        "restricted_areas": document.restricted_areas.iter().map(zone_json).collect::<Vec<_>>(),
    })
}

fn object_json(document: &Document, object: &Object) -> Value {
    let path = object.world_path();
    let layer = document
        .layer(&object.layer_id)
        .map(|layer| layer.name.clone());
    let operation = object.operation_id.as_deref().and_then(|id| {
        document
            .operations
            .iter()
            .find(|operation| operation.id == id)
            .map(|operation| operation.name.clone())
    });
    json!({
        "id": object.id,
        "name": object.name,
        "kind": object.shape.kind_name(),
        "layer_id": object.layer_id,
        "layer": layer,
        "operation_id": object.operation_id,
        "operation": operation,
        "hole_of": object.hole_of,
        "visible": object.visible,
        "locked": object.locked,
        "closed": path.closed,
        "length_mm": round(path.length()),
        "area_mm2": round(path_area(&path)),
        "bounds": object.bounds().map(bounds_json),
    })
}

#[derive(Debug, Default, Clone, Copy)]
struct Totals {
    objects: usize,
    length_mm: f64,
    area_mm2: f64,
}

impl Totals {
    fn add(&mut self, length_mm: f64, area_mm2: f64) {
        self.objects += 1;
        self.length_mm += length_mm;
        self.area_mm2 += area_mm2;
    }

    fn to_json(self) -> Value {
        json!({
            "objects": self.objects,
            "length_mm": round(self.length_mm),
            "area_mm2": round(self.area_mm2),
        })
    }
}

/// Lengths, areas and bounds for the document or a subset of its objects.
pub fn measure(document: &Document, ids: Option<&[String]>, include_hidden: bool) -> Value {
    let objects = select(document, ids, include_hidden);
    let mut total = Totals::default();
    let mut holes_area = 0.0;
    let mut closed = 0usize;
    let mut open = 0usize;
    let mut without_geometry = 0usize;
    let mut bounds: Option<(Point2, Point2)> = None;
    let mut by_layer: BTreeMap<String, Totals> = BTreeMap::new();
    let mut by_operation: BTreeMap<String, Totals> = BTreeMap::new();
    for object in &objects {
        let path = object.world_path();
        if path.segments.is_empty() {
            without_geometry += 1;
        }
        let length = path.length();
        let area = path_area(&path);
        if path.closed {
            closed += 1;
        } else {
            open += 1;
        }
        if object.hole_of.is_some() {
            holes_area += area;
        }
        total.add(length, area);
        let layer = document
            .layer(&object.layer_id)
            .map_or_else(|| "(missing layer)".to_string(), |layer| layer.name.clone());
        by_layer.entry(layer).or_default().add(length, area);
        let operation = object
            .operation_id
            .as_deref()
            .and_then(|id| {
                document
                    .operations
                    .iter()
                    .find(|operation| operation.id == id)
            })
            .map_or_else(|| "(none)".to_string(), |operation| operation.name.clone());
        by_operation.entry(operation).or_default().add(length, area);
        if let Some(object_bounds) = object.bounds() {
            bounds = Some(match bounds {
                None => object_bounds,
                Some((min, max)) => (
                    Point2::new(min.x.min(object_bounds.0.x), min.y.min(object_bounds.0.y)),
                    Point2::new(max.x.max(object_bounds.1.x), max.y.max(object_bounds.1.y)),
                ),
            });
        }
    }

    let by_layer: Vec<Value> = by_layer
        .into_iter()
        .map(|(name, totals)| {
            let mut entry = totals.to_json();
            entry["layer"] = json!(name);
            entry
        })
        .collect();
    let by_operation: Vec<Value> = by_operation
        .into_iter()
        .map(|(name, totals)| {
            let mut entry = totals.to_json();
            entry["operation"] = json!(name);
            entry
        })
        .collect();

    let mut result = json!({
        "objects": total.objects,
        "closed_paths": closed,
        "open_paths": open,
        "without_geometry": without_geometry,
        "length_mm": round(total.length_mm),
        "area_mm2": round(total.area_mm2),
        "holes_area_mm2": round(holes_area),
        "net_area_mm2": round((total.area_mm2 - holes_area).max(0.0)),
        "bounds_mm": bounds.map(bounds_json),
        "by_layer": by_layer,
        "by_operation": by_operation,
    });

    if ids.is_none() {
        result["job"] = job_json(document);
    }
    result
}

/// Cut and travel distance of the actual toolpaths, passes included.
fn job_json(document: &Document) -> Value {
    let (toolpaths, warnings) = crate::gcode::build_toolpaths(document);
    let mut cut_mm = 0.0;
    let mut travel_mm = 0.0;
    let mut passes = 0u32;
    for toolpath in &toolpaths {
        passes += toolpath.passes;
        let repeat = f64::from(toolpath.passes);
        for pair in toolpath.moves.windows(2) {
            let (from, to) = (&pair[0], &pair[1]);
            let distance = (to.x - from.x).hypot(to.y - from.y);
            match to.kind {
                crate::gcode::MoveKind::Cut => cut_mm += distance * repeat,
                crate::gcode::MoveKind::Travel => travel_mm += distance * repeat,
            }
        }
    }
    json!({
        "paths": toolpaths.len(),
        "passes": passes,
        "cut_mm": round(cut_mm),
        "travel_mm": round(travel_mm),
        "warnings": warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::{inspect, measure};
    use crate::document::model::Object;
    use crate::document::{Document, Shape};
    use crate::geometry::Transform2;
    use crate::operations::{Operation, OperationParams, OperationType};

    fn document() -> Document {
        let mut document = Document::default();
        document.operations.push(Operation {
            id: "cut".to_string(),
            name: "Cut".to_string(),
            op_type: OperationType::OutlineCutting,
            params: OperationParams::default(),
            enabled: true,
            depends_on: Vec::new(),
        });
        let layer_id = document.layers[0].id.clone();
        let mut rectangle = Object::new(
            layer_id.clone(),
            Shape::Rect {
                width: 20.0,
                height: 10.0,
                corner_radius: 0.0,
            },
        );
        rectangle.operation_id = Some("cut".to_string());
        document.add_object(rectangle);
        let outer = document.objects[0].id.clone();
        let mut hole = Object::new(
            layer_id,
            Shape::Rect {
                width: 4.0,
                height: 4.0,
                corner_radius: 0.0,
            },
        );
        hole.transform = Transform2::translate(2.0, 2.0);
        hole.hole_of = Some(outer);
        hole.operation_id = Some("cut".to_string());
        document.add_object(hole);
        document
    }

    #[test]
    fn inspect_lists_objects_with_bounds_and_areas() {
        let document = document();
        let report = inspect(&document, None);
        assert_eq!(report["counts"]["objects"], 2);
        assert_eq!(report["counts"]["holes"], 1);
        let object = &report["objects"][0];
        assert_eq!(object["kind"], "Rectangle");
        assert_eq!(object["operation"], "Cut");
        assert!((object["area_mm2"].as_f64().unwrap() - 200.0).abs() < 1e-6);
        assert!((object["length_mm"].as_f64().unwrap() - 60.0).abs() < 1e-6);
        assert_eq!(object["closed"], true);
    }

    #[test]
    fn inspect_can_limit_the_objects() {
        let document = document();
        let ids = vec![document.objects[1].id.clone()];
        let report = inspect(&document, Some(&ids));
        assert_eq!(report["objects"].as_array().unwrap().len(), 1);
        assert_eq!(
            report["objects"][0]["hole_of"],
            document.objects[0].id.clone()
        );
        assert_eq!(report["counts"]["objects"], 2);
    }

    #[test]
    fn measure_counts_lengths_areas_and_holes() {
        let document = document();
        let report = measure(&document, None, false);
        assert_eq!(report["objects"], 2);
        assert_eq!(report["closed_paths"], 2);
        assert!((report["length_mm"].as_f64().unwrap() - 76.0).abs() < 1e-6);
        assert!((report["area_mm2"].as_f64().unwrap() - 216.0).abs() < 1e-6);
        assert!((report["holes_area_mm2"].as_f64().unwrap() - 16.0).abs() < 1e-6);
        assert!((report["net_area_mm2"].as_f64().unwrap() - 200.0).abs() < 1e-6);
        let bounds = &report["bounds_mm"];
        assert!((bounds["width"].as_f64().unwrap() - 20.0).abs() < 1e-6);
        assert!((bounds["height"].as_f64().unwrap() - 10.0).abs() < 1e-6);
        assert_eq!(report["by_layer"][0]["layer"], "Layer 1");
        assert_eq!(report["by_operation"][0]["operation"], "Cut");
        assert!(report["job"]["cut_mm"].as_f64().unwrap() > 0.0);
    }

    #[test]
    fn measure_ignores_hidden_objects_unless_asked() {
        let mut document = document();
        document.objects[1].visible = false;
        let report = measure(&document, None, false);
        assert_eq!(report["objects"], 1);
        let report = measure(&document, None, true);
        assert_eq!(report["objects"], 2);
        let ids = vec![document.objects[1].id.clone()];
        let report = measure(&document, Some(&ids), false);
        assert_eq!(report["objects"], 1);
        assert_eq!(report["by_operation"][0]["operation"], "Cut");
    }

    #[test]
    fn an_empty_document_measures_zero() {
        let report = measure(&Document::default(), None, false);
        assert_eq!(report["objects"], 0);
        assert_eq!(report["length_mm"], 0.0);
        assert!(report["bounds_mm"].is_null());
        assert_eq!(report["job"]["paths"], 0);
    }

    #[test]
    fn objects_without_geometry_are_counted() {
        let mut document = document();
        let layer_id = document.layers[0].id.clone();
        document.add_object(Object::new(
            layer_id,
            Shape::Text(crate::objects::TextObject::default()),
        ));
        let report = measure(&document, None, false);
        assert_eq!(report["without_geometry"], 1);
    }
}
