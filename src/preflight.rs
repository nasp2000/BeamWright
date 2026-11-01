//! Preflight checks before writing a job.
//!
//! The report never blocks the editor; it tells a person or an agent what would
//! go wrong in the exported file. Errors mean the job would be empty or
//! invalid; warnings mean it will be written but something deserves a look.

use serde_json::json;

use crate::document::{Document, Shape};
use crate::geometry::Point2;

/// One finding, with a stable code for agents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issue {
    pub code: &'static str,
    pub message: String,
    pub object_id: Option<String>,
}

impl Issue {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            object_id: None,
        }
    }

    fn for_object(code: &'static str, message: impl Into<String>, id: &str) -> Self {
        Self {
            code,
            message: message.into(),
            object_id: Some(id.to_string()),
        }
    }

    fn to_json(&self) -> serde_json::Value {
        json!({
            "code": self.code,
            "message": self.message,
            "object_id": self.object_id,
        })
    }
}

/// Errors and warnings from [`check`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Report {
    pub errors: Vec<Issue>,
    pub warnings: Vec<Issue>,
}

impl Report {
    pub fn is_ok(&self) -> bool {
        self.errors.is_empty()
    }

    pub fn to_json(&self) -> serde_json::Value {
        json!({
            "ok": self.is_ok(),
            "errors": self.errors.iter().map(Issue::to_json).collect::<Vec<_>>(),
            "warnings": self.warnings.iter().map(Issue::to_json).collect::<Vec<_>>(),
        })
    }

    fn error(&mut self, code: &'static str, message: impl Into<String>) {
        self.errors.push(Issue::new(code, message));
    }

    fn error_for(&mut self, code: &'static str, message: impl Into<String>, id: &str) {
        self.errors.push(Issue::for_object(code, message, id));
    }

    fn warning(&mut self, code: &'static str, message: impl Into<String>) {
        self.warnings.push(Issue::new(code, message));
    }

    fn warning_for(&mut self, code: &'static str, message: impl Into<String>, id: &str) {
        self.warnings.push(Issue::for_object(code, message, id));
    }
}

/// Check the document without changing anything.
pub fn check(document: &Document) -> Report {
    let mut report = Report::default();
    if let Err(error) = document.validate() {
        report.error("invalid-document", error);
        return report;
    }
    if document.objects.is_empty() {
        report.warning("empty-document", "the document has no objects");
    }
    check_objects(document, &mut report);
    check_operations(document, &mut report);
    check_job(document, &mut report);
    report
}

fn check_objects(document: &Document, report: &mut Report) {
    let page = (
        Point2::new(0.0, 0.0),
        Point2::new(document.width_mm, document.height_mm),
    );
    for object in &document.objects {
        if !object.visible {
            continue;
        }
        if let Shape::Image(image) = &object.shape {
            if !std::path::Path::new(&image.path).is_file() {
                report.error_for(
                    "missing-image",
                    format!("image '{}' is missing: {}", object.name, image.path),
                    &object.id,
                );
            }
        }
        if matches!(object.shape, Shape::Text(_)) {
            report.warning_for(
                "text-not-outlined",
                format!(
                    "text '{}' is omitted from G-code and PNG until it is converted to outlines",
                    object.name
                ),
                &object.id,
            );
        }
        let Some(bounds) = object.bounds() else {
            report.warning_for(
                "no-geometry",
                format!("'{}' has no bounds", object.name),
                &object.id,
            );
            continue;
        };
        if !within(bounds, page) {
            report.warning_for(
                "outside-work-area",
                format!("'{}' extends outside the work area", object.name),
                &object.id,
            );
        }
        if let Some(stock) = document.stock {
            let material = (
                Point2::new(stock.x_mm, stock.y_mm),
                Point2::new(stock.max_x(), stock.max_y()),
            );
            if !within(bounds, material) {
                report.warning_for(
                    "outside-stock",
                    format!("'{}' extends outside the material area", object.name),
                    &object.id,
                );
            }
        }
        for area in &document.restricted_areas {
            let forbidden = (
                Point2::new(area.x_mm, area.y_mm),
                Point2::new(area.max_x(), area.max_y()),
            );
            if overlaps(bounds, forbidden) {
                report.warning_for(
                    "inside-restricted-area",
                    format!("'{}' reaches a restricted area", object.name),
                    &object.id,
                );
            }
        }
    }
}

fn check_operations(document: &Document, report: &mut Report) {
    for object in &document.objects {
        if let Some(id) = &object.operation_id {
            if document
                .operations
                .iter()
                .all(|operation| &operation.id != id)
            {
                report.error_for(
                    "missing-operation",
                    format!(
                        "'{}' is assigned to an operation that no longer exists",
                        object.name
                    ),
                    &object.id,
                );
            }
        }
    }
    for operation in &document.operations {
        if !operation.enabled {
            continue;
        }
        let assigned = document
            .objects
            .iter()
            .filter(|object| object.operation_id.as_deref() == Some(operation.id.as_str()))
            .count();
        if assigned == 0 {
            report.warning(
                "unused-operation",
                format!("operation '{}' has no objects", operation.name),
            );
        }
    }
    let ordered = crate::gcode::job::ordered_operation_ids(document).len();
    let enabled = document
        .operations
        .iter()
        .filter(|operation| operation.enabled)
        .count();
    if ordered < enabled {
        report.error(
            "operation-cycle",
            "the operation dependencies contain a cycle",
        );
    }
}

fn check_job(document: &Document, report: &mut Report) {
    if document.objects.is_empty() {
        return;
    }
    let (toolpaths, warnings) = crate::gcode::build_toolpaths(document);
    for warning in warnings {
        report.warning("job", warning);
    }
    if toolpaths.is_empty() {
        report.error("no-toolpaths", "the job has no toolpaths to write");
    }
}

fn within(bounds: (Point2, Point2), area: (Point2, Point2)) -> bool {
    bounds.0.x >= area.0.x - 1e-9
        && bounds.0.y >= area.0.y - 1e-9
        && bounds.1.x <= area.1.x + 1e-9
        && bounds.1.y <= area.1.y + 1e-9
}

fn overlaps(left: (Point2, Point2), right: (Point2, Point2)) -> bool {
    left.0.x <= right.1.x + 1e-9
        && left.1.x >= right.0.x - 1e-9
        && left.0.y <= right.1.y + 1e-9
        && left.1.y >= right.0.y - 1e-9
}

#[cfg(test)]
mod tests {
    use super::check;
    use crate::document::model::Object;
    use crate::document::{Document, MaterialZone, Shape};
    use crate::geometry::Transform2;
    use crate::operations::{Operation, OperationParams, OperationType};

    fn rectangle_document() -> Document {
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
        let mut object = Object::new(
            layer_id,
            Shape::Rect {
                width: 20.0,
                height: 10.0,
                corner_radius: 0.0,
            },
        );
        object.operation_id = Some("cut".to_string());
        document.add_object(object);
        document
    }

    #[test]
    fn a_clean_document_has_no_errors() {
        let report = check(&rectangle_document());
        assert!(report.is_ok(), "{:?}", report.errors);
        assert_eq!(report.warnings.len(), 0, "{:?}", report.warnings);
    }

    #[test]
    fn an_empty_document_warns() {
        let report = check(&Document::default());
        assert!(report.is_ok());
        assert!(
            report
                .warnings
                .iter()
                .any(|issue| issue.code == "empty-document")
        );
    }

    #[test]
    fn a_missing_image_is_an_error() {
        let mut document = rectangle_document();
        document.objects.clear();
        let layer_id = document.layers[0].id.clone();
        let object = Object::new(
            layer_id,
            Shape::Image(crate::objects::ImageObject::new(
                "/tmp/beamwright_missing_preflight.png",
                10,
                10,
                96.0,
            )),
        );
        document.add_object(object);
        let report = check(&document);
        assert!(
            report
                .errors
                .iter()
                .any(|issue| issue.code == "missing-image")
        );
    }

    #[test]
    fn text_warns_about_outlines() {
        let mut document = rectangle_document();
        let layer_id = document.layers[0].id.clone();
        let text = crate::objects::TextObject {
            content: "Hello".to_string(),
            ..crate::objects::TextObject::default()
        };
        document.add_object(Object::new(layer_id, Shape::Text(text)));
        let report = check(&document);
        assert!(
            report
                .warnings
                .iter()
                .any(|issue| issue.code == "text-not-outlined")
        );
    }

    #[test]
    fn objects_outside_the_area_and_stock_warn() {
        let mut document = rectangle_document();
        document.objects[0].transform = Transform2::translate(290.0, 0.0);
        let report = check(&document);
        assert!(
            report
                .warnings
                .iter()
                .any(|issue| issue.code == "outside-work-area")
        );

        document.stock = Some(MaterialZone::new(0.0, 0.0, 50.0, 50.0));
        let report = check(&document);
        assert!(
            report
                .warnings
                .iter()
                .any(|issue| issue.code == "outside-stock")
        );
    }

    #[test]
    fn a_restricted_area_warns() {
        let mut document = rectangle_document();
        document.restricted_areas = vec![MaterialZone::new(5.0, 2.0, 3.0, 3.0)];
        let report = check(&document);
        assert!(
            report
                .warnings
                .iter()
                .any(|issue| issue.code == "inside-restricted-area")
        );
    }

    #[test]
    fn an_operation_cycle_is_an_error() {
        let mut document = rectangle_document();
        document.operations.push(Operation {
            id: "second".to_string(),
            name: "Second".to_string(),
            op_type: OperationType::OutlineCutting,
            params: OperationParams::default(),
            enabled: true,
            depends_on: vec!["cut".to_string()],
        });
        document.operations[0].depends_on = vec!["second".to_string()];
        let report = check(&document);
        assert!(
            report
                .errors
                .iter()
                .any(|issue| issue.code == "invalid-document"),
            "{:?}",
            report.errors
        );
    }
}
