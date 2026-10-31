use std::path::Path;

use crate::document::shape::Shape;
use crate::document::{Document, Object};
use crate::errors::{BeamError, Result};
use crate::gcode::dialect::GCodeOptions;
use crate::gcode::toolpath::{MoveKind, Toolpath};
use crate::gcode::writer::GCodeWriter;
use crate::geometry::Point2;
use crate::operations::model::OperationParams;
use crate::operations::{Operation, OperationType};

/// Job generation options that are not part of a single operation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct JobOptions {
    pub gcode: GCodeOptions,
    /// Rapid positioning speed, in millimetres per minute.
    pub travel_speed_mm_min: f64,
    /// Reorder operations to shorten rapid moves between them.
    pub assisted_ordering: bool,
}

impl Default for JobOptions {
    fn default() -> Self {
        Self {
            gcode: GCodeOptions::default(),
            travel_speed_mm_min: 3000.0,
            assisted_ordering: true,
        }
    }
}

/// Summary of a generated job.
#[derive(Debug, Clone, PartialEq)]
pub struct JobReport {
    pub lines: usize,
    pub bytes: usize,
    pub seconds: f64,
    pub paths: usize,
    pub warnings: Vec<String>,
}

/// Parameters resolved for one object.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Resolved {
    speed: f64,
    power: f64,
    passes: u32,
    line_interval_mm: f64,
    op_type: OperationType,
}

fn resolve(object: &Object, document: &Document) -> Option<Resolved> {
    let operation: Option<&Operation> = object.operation_id.as_deref().and_then(|id| {
        document
            .operations
            .iter()
            .find(|operation| operation.id == id)
    });
    match operation {
        Some(operation) if !operation.enabled => None,
        Some(operation) => Some(Resolved {
            speed: operation.params.speed,
            power: operation.params.power.clamp(0.0, 100.0) * 10.0,
            passes: operation.params.passes.max(1),
            line_interval_mm: operation.params.line_interval_mm.max(0.01),
            op_type: operation.op_type,
        }),
        None => {
            let params = OperationParams::default();
            Some(Resolved {
                speed: params.speed,
                power: params.power.clamp(0.0, 100.0) * 10.0,
                passes: params.passes.max(1),
                line_interval_mm: params.line_interval_mm.max(0.01),
                op_type: OperationType::Line,
            })
        }
    }
}

/// Build toolpaths from visible document geometry.
pub fn build_toolpaths(document: &Document) -> (Vec<Toolpath>, Vec<String>) {
    let mut toolpaths = Vec::new();
    let mut warnings = Vec::new();
    let mut layers: Vec<_> = document.layers.iter().collect();
    layers.sort_by_key(|layer| layer.order);
    for layer in layers {
        if !layer.visible {
            continue;
        }
        for object in document
            .objects
            .iter()
            .filter(|object| object.layer_id == layer.id && object.visible)
        {
            let Some(resolved) = resolve(object, document) else {
                continue;
            };
            match &object.shape {
                Shape::Image(image) => match raster_toolpath(object, image, &resolved) {
                    Ok(Some(toolpath)) => toolpaths.push(toolpath),
                    Ok(None) => {}
                    Err(error) => {
                        warnings.push(format!("image '{}' was omitted: {error}", object.name));
                    }
                },
                Shape::Text(_) => warnings.push(format!(
                    "{} '{}' has no vector outline and was omitted",
                    object.shape.kind_name(),
                    object.name
                )),
                _ => {
                    let points = object.world_path().flatten(0.2);
                    if points.len() < 2 {
                        warnings.push(format!("'{}' has no usable geometry", object.name));
                        continue;
                    }
                    if matches!(
                        resolved.op_type,
                        OperationType::Fill
                            | OperationType::FillWithOutline
                            | OperationType::FillWithOffset
                    ) {
                        let polygon: Vec<(f64, f64)> =
                            points.iter().map(|point| (point.x, point.y)).collect();
                        let lines = crate::geometry::fill::fill_lines(
                            &polygon,
                            resolved.line_interval_mm,
                            0.0,
                        );
                        if lines.is_empty() {
                            warnings.push(format!(
                                "'{}' is too small to fill at {} mm spacing",
                                object.name, resolved.line_interval_mm
                            ));
                        }
                        let mut toolpath = Toolpath::new(
                            object.name.clone(),
                            resolved.speed,
                            resolved.power,
                            resolved.passes,
                        );
                        for ((x0, y0), (x1, y1)) in lines {
                            toolpath.push(MoveKind::Travel, x0, y0);
                            toolpath.push(MoveKind::Cut, x1, y1);
                        }
                        if resolved.op_type == OperationType::FillWithOutline {
                            append_contour(&mut toolpath, &points);
                        }
                        if !toolpath.is_empty() {
                            toolpaths.push(toolpath.with_operation(object.operation_id.as_deref()));
                        }
                    } else {
                        let mut toolpath = Toolpath::new(
                            object.name.clone(),
                            resolved.speed,
                            resolved.power,
                            resolved.passes,
                        );
                        append_contour(&mut toolpath, &points);
                        toolpaths.push(toolpath.with_operation(object.operation_id.as_deref()));
                    }
                }
            }
        }
    }
    (toolpaths, warnings)
}

/// Build a raster toolpath for one image object.
///
/// Consecutive burned pixels in a row become one cut segment; the image's
/// top-left pixel maps to the object's local top-left corner.
fn raster_toolpath(
    object: &Object,
    image: &crate::objects::ImageObject,
    resolved: &Resolved,
) -> Result<Option<Toolpath>> {
    let path = std::path::Path::new(&image.path);
    if !path.is_file() {
        return Err(BeamError::InvalidInput(format!(
            "file not found: {}",
            image.path
        )));
    }
    let bitmap = crate::raster::process_file(path, &image.settings)?;
    let mut toolpath = Toolpath::new(
        object.name.clone(),
        resolved.speed,
        resolved.power,
        resolved.passes,
    );
    let pixel_mm = 25.4 / image.dpi;
    for row in 0..bitmap.height {
        let mut run_start: Option<usize> = None;
        for column in 0..=bitmap.width {
            let burned = column < bitmap.width && bitmap.get(column, row);
            match (burned, run_start) {
                (true, None) => run_start = Some(column),
                (false, Some(start)) => {
                    let local = TransformBake {
                        transform: object.transform,
                        pixel_mm,
                        height_px: bitmap.height,
                    };
                    let (x0, y0) = local.point(start, row);
                    let (x1, y1) = local.point(column, row);
                    toolpath.push(MoveKind::Travel, x0, y0);
                    toolpath.push(MoveKind::Cut, x1, y1);
                    run_start = None;
                }
                _ => {}
            }
        }
    }
    if toolpath.is_empty() {
        return Ok(None);
    }
    Ok(Some(
        toolpath.with_operation(object.operation_id.as_deref()),
    ))
}

struct TransformBake {
    transform: crate::geometry::Transform2,
    pixel_mm: f64,
    height_px: usize,
}

impl TransformBake {
    fn point(&self, x: usize, y: usize) -> (f64, f64) {
        #[allow(clippy::cast_precision_loss)]
        let local_x = x as f64 * self.pixel_mm;
        #[allow(clippy::cast_precision_loss)]
        let local_y = (self.height_px as f64 - y as f64) * self.pixel_mm;
        let point = self
            .transform
            .apply(crate::geometry::Point2::new(local_x, local_y));
        (point.x, point.y)
    }
}

fn append_contour(toolpath: &mut Toolpath, points: &[Point2]) {
    toolpath.push(MoveKind::Travel, points[0].x, points[0].y);
    for point in &points[1..] {
        toolpath.push(MoveKind::Cut, point.x, point.y);
    }
}

/// Order toolpaths by the position of their operation in the document.
///
/// The sort is stable, so objects inside one operation keep their document
/// order, and toolpaths without an operation come last.
pub fn order_by_operation(toolpaths: &mut [Toolpath], document: &Document) {
    let rank = |toolpath: &Toolpath| {
        toolpath
            .operation_id
            .as_deref()
            .and_then(|id| {
                document
                    .operations
                    .iter()
                    .position(|operation| operation.id == id)
            })
            .unwrap_or(usize::MAX)
    };
    toolpaths.sort_by_key(rank);
}

/// Reorder toolpaths greedily by nearest start point.
///
/// Ordering never crosses an operation boundary: the laser keeps one power and
/// feed for a whole group before moving on to the next operation.
pub fn order_toolpaths(toolpaths: &mut [Toolpath]) {
    let mut start = 0;
    while start < toolpaths.len() {
        let mut end = start + 1;
        while end < toolpaths.len() && toolpaths[end].operation_id == toolpaths[start].operation_id
        {
            end += 1;
        }
        order_group(&mut toolpaths[start..end]);
        start = end;
    }
}

fn order_group(toolpaths: &mut [Toolpath]) {
    let mut position = Point2::origin();
    let mut index = 0;
    while index < toolpaths.len() {
        let mut best = index;
        let mut best_distance = f64::INFINITY;
        for (candidate, toolpath) in toolpaths.iter().enumerate().skip(index) {
            let Some(start) = toolpath.start() else {
                continue;
            };
            let distance = position.distance(start);
            if distance < best_distance {
                best_distance = distance;
                best = candidate;
            }
        }
        toolpaths.swap(index, best);
        if let Some(end) = toolpaths[index].end() {
            position = end;
        }
        index += 1;
    }
}

/// Validate that every move stays inside the document work area.
pub fn validate_bounds(document: &Document, toolpaths: &[Toolpath]) -> Result<()> {
    let tolerance = 1e-6;
    for toolpath in toolpaths {
        for movement in &toolpath.moves {
            if !movement.x.is_finite() || !movement.y.is_finite() {
                return Err(BeamError::InvalidInput(format!(
                    "'{}' contains a non-finite coordinate",
                    toolpath.name
                )));
            }
            if movement.x < -tolerance
                || movement.y < -tolerance
                || movement.x > document.width_mm + tolerance
                || movement.y > document.height_mm + tolerance
            {
                return Err(BeamError::InvalidInput(format!(
                    "'{}' leaves the {:.1} x {:.1} mm work area at ({:.3}, {:.3})",
                    toolpath.name, document.width_mm, document.height_mm, movement.x, movement.y
                )));
            }
        }
    }
    Ok(())
}

/// Estimate job duration in seconds, including rapids.
pub fn estimate_seconds(toolpaths: &[Toolpath], travel_speed_mm_min: f64) -> f64 {
    let travel_speed = if travel_speed_mm_min > 0.0 {
        travel_speed_mm_min
    } else {
        3000.0
    };
    let mut position = Point2::origin();
    let mut seconds = 0.0;
    for toolpath in toolpaths {
        let passes = f64::from(toolpath.passes);
        let approach = toolpath
            .start()
            .map_or(0.0, |start| position.distance(start));
        let cut = if toolpath.speed > 0.0 {
            toolpath.cut_length() / toolpath.speed
        } else {
            0.0
        };
        let travel = (approach + toolpath.travel_length()) / travel_speed;
        seconds += (cut + travel) * passes * 60.0;
        if let Some(end) = toolpath.end() {
            position = end;
        }
    }
    seconds
}

/// Toolpaths in the order they will be executed.
pub fn planned_toolpaths(
    document: &Document,
    options: &JobOptions,
) -> (Vec<Toolpath>, Vec<String>) {
    let (mut toolpaths, warnings) = build_toolpaths(document);
    order_by_operation(&mut toolpaths, document);
    if options.assisted_ordering {
        order_toolpaths(&mut toolpaths);
    }
    (toolpaths, warnings)
}

/// Generate the G-code text and a report without writing a file.
pub fn render_job(document: &Document, options: &JobOptions) -> Result<(String, JobReport)> {
    let (toolpaths, warnings) = planned_toolpaths(document, options);
    validate_bounds(document, &toolpaths)?;
    for toolpath in &toolpaths {
        if toolpath.speed <= 0.0 {
            return Err(BeamError::InvalidInput(format!(
                "'{}' has a non-positive feed rate",
                toolpath.name
            )));
        }
        if !(0.0..=1000.0).contains(&toolpath.power) {
            return Err(BeamError::InvalidInput(format!(
                "'{}' has power outside the supported range",
                toolpath.name
            )));
        }
    }
    let text = GCodeWriter::new(options.gcode).render(&toolpaths);
    let report = JobReport {
        lines: text.lines().count(),
        bytes: text.len(),
        seconds: estimate_seconds(&toolpaths, options.travel_speed_mm_min),
        paths: toolpaths.len(),
        warnings,
    };
    Ok((text, report))
}

/// Write the job atomically to `path`.
pub fn export_job(document: &Document, path: &Path, options: &JobOptions) -> Result<JobReport> {
    let (text, report) = render_job(document, options)?;
    let temporary = path.with_extension("nc.tmp");
    std::fs::write(&temporary, text)?;
    std::fs::rename(&temporary, path)?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::Object;

    fn document_with_rect() -> Document {
        let mut document = Document::default();
        let layer_id = document.layers[0].id.clone();
        document.add_object(Object::new(
            layer_id,
            Shape::Rect {
                width: 10.0,
                height: 5.0,
                corner_radius: 0.0,
            },
        ));
        document
    }

    #[test]
    fn empty_document_has_no_cuts() {
        let (text, report) = render_job(&Document::default(), &JobOptions::default()).unwrap();
        assert!(text.contains("G21"));
        assert!(text.contains("M5"));
        assert!(!text.contains("G1 X"));
        assert_eq!(report.paths, 0);
        assert_eq!(report.seconds, 0.0);
    }

    #[test]
    fn rectangle_becomes_a_closed_contour() {
        let document = document_with_rect();
        let (text, report) = render_job(&document, &JobOptions::default()).unwrap();
        assert_eq!(report.paths, 1);
        assert!(text.contains("G1 X"));
        assert_eq!(text.matches("G1 X").count(), 4);
        assert!(report.lines > 5);
    }

    #[test]
    fn operation_parameters_are_applied() {
        let mut document = document_with_rect();
        let operation = Operation {
            id: "op-1".to_string(),
            name: "Cut".to_string(),
            op_type: OperationType::OutlineCutting,
            params: OperationParams {
                speed: 800.0,
                power: 35.0,
                passes: 2,
                line_interval_mm: 0.1,
            },
            enabled: true,
        };
        document.operations.push(operation);
        document.objects[0].operation_id = Some("op-1".to_string());
        let (text, _) = render_job(&document, &JobOptions::default()).unwrap();
        assert!(text.contains("F800 S350"));
        assert_eq!(text.matches("G1 X").count(), 8);
    }

    #[test]
    fn disabled_operation_is_skipped() {
        let mut document = document_with_rect();
        document.operations.push(Operation {
            id: "op-1".to_string(),
            name: "Cut".to_string(),
            op_type: OperationType::OutlineCutting,
            params: OperationParams::default(),
            enabled: false,
        });
        document.objects[0].operation_id = Some("op-1".to_string());
        let (toolpaths, warnings) = build_toolpaths(&document);
        assert_eq!(toolpaths.len(), 0);
        assert_eq!(warnings.len(), 0);
    }

    #[test]
    fn out_of_bounds_geometry_is_rejected() {
        let mut document = Document {
            width_mm: 10.0,
            height_mm: 10.0,
            ..Document::default()
        };
        let layer_id = document.layers[0].id.clone();
        let object = Object::new(
            layer_id,
            Shape::Rect {
                width: 20.0,
                height: 5.0,
                corner_radius: 0.0,
            },
        );
        document.add_object(object);
        let error = render_job(&document, &JobOptions::default()).unwrap_err();
        assert!(matches!(error, BeamError::InvalidInput(_)));
    }

    #[test]
    fn fill_operation_produces_several_lines() {
        let mut document = document_with_rect();
        document.operations.push(Operation {
            id: "fill".to_string(),
            name: "Fill".to_string(),
            op_type: OperationType::Fill,
            params: OperationParams {
                speed: 2000.0,
                power: 10.0,
                passes: 1,
                line_interval_mm: 1.0,
            },
            enabled: true,
        });
        document.objects[0].operation_id = Some("fill".to_string());
        let (toolpaths, _) = build_toolpaths(&document);
        assert_eq!(toolpaths.len(), 1);
        assert!(toolpaths[0].moves.len() > 4);
    }

    #[test]
    fn nearest_neighbour_ordering_shortens_travel() {
        let mut first = Toolpath::new("a", 1000.0, 100.0, 1);
        first.push(MoveKind::Travel, 90.0, 0.0);
        first.push(MoveKind::Cut, 100.0, 0.0);
        let mut second = Toolpath::new("b", 1000.0, 100.0, 1);
        second.push(MoveKind::Travel, 10.0, 0.0);
        second.push(MoveKind::Cut, 20.0, 0.0);
        let mut toolpaths = vec![first, second];
        order_toolpaths(&mut toolpaths);
        assert_eq!(toolpaths[0].name, "b");
    }

    fn two_operation_document() -> Document {
        let mut document = Document::default();
        let layer_id = document.layers[0].id.clone();
        let mut first = Object::new(
            layer_id.clone(),
            Shape::Rect {
                width: 10.0,
                height: 5.0,
                corner_radius: 0.0,
            },
        );
        first.name = "engrave".to_string();
        first.operation_id = Some("engrave".to_string());
        let mut second = Object::new(
            layer_id,
            Shape::Rect {
                width: 10.0,
                height: 5.0,
                corner_radius: 0.0,
            },
        );
        second.name = "cut".to_string();
        second.operation_id = Some("cut".to_string());
        second.transform = crate::geometry::Transform2::translate(20.0, 0.0);
        document.add_object(first);
        document.add_object(second);
        // The engraving object comes first in the document, but the cut
        // operation is listed first, so the cut must run first.
        document.operations.push(Operation {
            id: "cut".to_string(),
            name: "Cut".to_string(),
            op_type: OperationType::OutlineCutting,
            params: OperationParams {
                speed: 500.0,
                power: 80.0,
                passes: 1,
                line_interval_mm: 0.1,
            },
            enabled: true,
        });
        document.operations.push(Operation {
            id: "engrave".to_string(),
            name: "Engrave".to_string(),
            op_type: OperationType::LineEngraving,
            params: OperationParams {
                speed: 3000.0,
                power: 15.0,
                passes: 1,
                line_interval_mm: 0.1,
            },
            enabled: true,
        });
        document
    }

    #[test]
    fn toolpaths_follow_the_operation_order() {
        let document = two_operation_document();
        let (mut toolpaths, _) = build_toolpaths(&document);
        assert_eq!(toolpaths[0].name, "engrave");
        order_by_operation(&mut toolpaths, &document);
        assert_eq!(toolpaths[0].name, "cut");
        assert_eq!(toolpaths[0].operation_id.as_deref(), Some("cut"));
        assert_eq!(toolpaths[1].name, "engrave");
    }

    #[test]
    fn assisted_ordering_keeps_operations_together() {
        let mut a1 = Toolpath::new("a1", 1000.0, 100.0, 1).with_operation(Some("a"));
        a1.push(MoveKind::Travel, 90.0, 0.0);
        a1.push(MoveKind::Cut, 100.0, 0.0);
        let mut b1 = Toolpath::new("b1", 1000.0, 100.0, 1).with_operation(Some("b"));
        b1.push(MoveKind::Travel, 10.0, 0.0);
        b1.push(MoveKind::Cut, 20.0, 0.0);
        let mut a2 = Toolpath::new("a2", 1000.0, 100.0, 1).with_operation(Some("a"));
        a2.push(MoveKind::Travel, 30.0, 0.0);
        a2.push(MoveKind::Cut, 40.0, 0.0);
        // Groups arrive contiguous (after `order_by_operation`); the nearest
        // neighbour pass may reorder inside a group but never across one.
        let mut toolpaths = vec![a1, a2, b1];
        order_toolpaths(&mut toolpaths);
        let order: Vec<&str> = toolpaths.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(order, vec!["a2", "a1", "b1"]);
    }

    #[test]
    fn unassigned_toolpaths_run_last() {
        let mut document = two_operation_document();
        let layer_id = document.layers[0].id.clone();
        let mut loose = Object::new(
            layer_id,
            Shape::Rect {
                width: 4.0,
                height: 4.0,
                corner_radius: 0.0,
            },
        );
        loose.name = "loose".to_string();
        loose.transform = crate::geometry::Transform2::translate(40.0, 0.0);
        document.add_object(loose);
        let (mut toolpaths, _) = build_toolpaths(&document);
        order_by_operation(&mut toolpaths, &document);
        assert_eq!(toolpaths.last().unwrap().name, "loose");
        assert!(toolpaths.last().unwrap().operation_id.is_none());
    }

    #[test]
    fn gcode_emits_operations_in_panel_order() {
        let document = two_operation_document();
        let options = JobOptions {
            assisted_ordering: false,
            ..JobOptions::default()
        };
        let (text, report) = render_job(&document, &options).unwrap();
        assert_eq!(report.paths, 2);
        let cut = text.find("F500 S800").expect("cut feed");
        let engrave = text.find("F3000 S150").expect("engrave feed");
        assert!(cut < engrave, "the cut operation must come first");
    }

    #[test]
    fn estimate_counts_passes_and_speed() {
        let mut toolpath = Toolpath::new("a", 600.0, 100.0, 1);
        toolpath.push(MoveKind::Travel, 0.0, 0.0);
        toolpath.push(MoveKind::Cut, 10.0, 0.0);
        let one = estimate_seconds(std::slice::from_ref(&toolpath), 6000.0);
        // 10 mm at 600 mm/min = 1 s; the path starts at the origin.
        assert!((one - 1.0).abs() < 1e-9);
        toolpath.passes = 2;
        let two = estimate_seconds(&[toolpath], 6000.0);
        assert!((two - 2.0).abs() < 1e-9);
        // A path starting 10 mm away adds a 0.1 s rapid.
        let mut offset = Toolpath::new("b", 600.0, 100.0, 1);
        offset.push(MoveKind::Travel, 10.0, 0.0);
        offset.push(MoveKind::Cut, 20.0, 0.0);
        let with_approach = estimate_seconds(&[offset], 6000.0);
        assert!((with_approach - 1.1).abs() < 1e-9);
    }

    #[test]
    fn text_is_reported_as_omitted() {
        let mut document = Document::default();
        let layer_id = document.layers[0].id.clone();
        document.add_object(Object::new(
            layer_id,
            Shape::Text(crate::objects::TextObject {
                content: "hi".to_string(),
                font: "sans".to_string(),
                size: 4.0,
                x: 0.0,
                y: 0.0,
            }),
        ));
        let (toolpaths, warnings) = build_toolpaths(&document);
        assert_eq!(toolpaths.len(), 0);
        assert_eq!(warnings.len(), 1);
    }

    fn image_document(path: &std::path::Path) -> Document {
        let mut document = Document::default();
        let layer_id = document.layers[0].id.clone();
        let image = crate::objects::ImageObject::new(
            path.to_string_lossy(),
            4,
            2,
            crate::import::raster::DEFAULT_DPI,
        );
        document.add_object(Object::new(layer_id, Shape::Image(image)));
        document.operations.push(Operation {
            id: "engrave".to_string(),
            name: "Image".to_string(),
            op_type: OperationType::ImageProcessing,
            params: OperationParams {
                speed: 3000.0,
                power: 20.0,
                passes: 1,
                line_interval_mm: 0.1,
            },
            enabled: true,
        });
        document.objects[0].operation_id = Some("engrave".to_string());
        document
    }

    #[test]
    fn raster_image_generates_engraving_runs() {
        let path = std::env::temp_dir().join("beamwright_job_raster.png");
        let mut image = image::GrayImage::new(4, 2);
        for (x, _y, pixel) in image.enumerate_pixels_mut() {
            *pixel = if x < 2 {
                image::Luma([0])
            } else {
                image::Luma([255])
            };
        }
        image.save(&path).unwrap();
        let document = image_document(&path);
        let (toolpaths, warnings) = build_toolpaths(&document);
        assert_eq!(warnings.len(), 0);
        assert_eq!(toolpaths.len(), 1);
        // Two rows, each with one run: travel + cut per row.
        assert_eq!(toolpaths[0].moves.len(), 4);
        assert_eq!(toolpaths[0].speed, 3000.0);
        assert!((toolpaths[0].power - 200.0).abs() < 1e-9);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn missing_image_file_is_reported() {
        let document = image_document(std::path::Path::new("/tmp/beamwright_no_such.png"));
        let (toolpaths, warnings) = build_toolpaths(&document);
        assert_eq!(toolpaths.len(), 0);
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("not found"));
    }

    #[test]
    fn blank_image_produces_no_toolpath() {
        let path = std::env::temp_dir().join("beamwright_job_blank.png");
        let image = image::GrayImage::from_pixel(4, 2, image::Luma([255]));
        image.save(&path).unwrap();
        let document = image_document(&path);
        let (toolpaths, warnings) = build_toolpaths(&document);
        assert_eq!(toolpaths.len(), 0);
        assert_eq!(warnings.len(), 0);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn export_writes_file_atomically() {
        let document = document_with_rect();
        let path = std::env::temp_dir().join("beamwright_job.nc");
        let _ = std::fs::remove_file(&path);
        let report = export_job(&document, &path, &JobOptions::default()).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert_eq!(report.lines, text.lines().count());
        assert!(!path.with_extension("nc.tmp").exists());
        let _ = std::fs::remove_file(&path);
    }
}
