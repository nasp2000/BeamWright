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
    /// Shift the whole job so its lower-left corner is at (0, 0).
    ///
    /// Use it with a selection to engrave only that zone at the machine origin.
    pub offset_to_origin: bool,
}

impl Default for JobOptions {
    fn default() -> Self {
        Self {
            gcode: GCodeOptions::default(),
            travel_speed_mm_min: 3000.0,
            assisted_ordering: true,
            offset_to_origin: false,
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
#[derive(Debug, Clone, PartialEq)]
#[allow(clippy::struct_excessive_bools)]
struct Resolved {
    speed: f64,
    power: f64,
    passes: u32,
    line_interval_mm: f64,
    op_type: OperationType,
    fill_angle_degrees: f64,
    fill_alternate: bool,
    fill_margin_mm: f64,
    fill_offset_mm: f64,
    overscan_mm: f64,
    cross_hatch: bool,
    air_assist: bool,
    constant_power: bool,
    operation_name: Option<String>,
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
            fill_angle_degrees: operation.params.fill_angle_degrees,
            fill_alternate: operation.params.fill_alternate,
            fill_margin_mm: operation.params.fill_margin_mm.max(0.0),
            overscan_mm: operation.params.overscan_mm.max(0.0),
            cross_hatch: operation.params.cross_hatch,
            air_assist: operation.params.air_assist,
            constant_power: operation.params.constant_power,
            fill_offset_mm: operation.params.fill_offset_mm,
            operation_name: Some(operation.name.clone()),
        }),
        None => {
            let params = OperationParams::default();
            Some(Resolved {
                speed: params.speed,
                power: params.power.clamp(0.0, 100.0) * 10.0,
                passes: params.passes.max(1),
                line_interval_mm: params.line_interval_mm.max(0.01),
                op_type: OperationType::Line,
                fill_angle_degrees: params.fill_angle_degrees,
                fill_alternate: params.fill_alternate,
                fill_margin_mm: params.fill_margin_mm.max(0.0),
                fill_offset_mm: params.fill_offset_mm,
                overscan_mm: params.overscan_mm.max(0.0),
                cross_hatch: params.cross_hatch,
                air_assist: params.air_assist,
                constant_power: params.constant_power,
                operation_name: None,
            })
        }
    }
}

/// Build toolpaths from visible document geometry.
pub fn build_toolpaths(document: &Document) -> (Vec<Toolpath>, Vec<String>) {
    build_toolpaths_cancellable(document, &|| false).unwrap_or_default()
}

fn cancelled() -> BeamError {
    BeamError::InvalidInput("cancelled".to_string())
}

/// Build toolpaths, checking `cancel` between objects and inside long fills.
pub fn build_toolpaths_cancellable(
    document: &Document,
    cancel: &dyn Fn() -> bool,
) -> Result<(Vec<Toolpath>, Vec<String>)> {
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
            if cancel() {
                return Err(cancelled());
            }
            let Some(resolved) = resolve(object, document) else {
                continue;
            };
            match &object.shape {
                Shape::Image(image) => match raster_toolpath(object, image, &resolved, cancel) {
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
                        push_fill_toolpath(
                            &mut toolpaths,
                            object,
                            &resolved,
                            &points,
                            &mut warnings,
                            cancel,
                        )?;
                    } else {
                        let mut toolpath = Toolpath::new(
                            object.name.clone(),
                            resolved.speed,
                            resolved.power,
                            resolved.passes,
                        );
                        append_contour(&mut toolpath, &points);
                        let mut toolpath = toolpath
                            .with_operation(object.operation_id.as_deref())
                            .with_air_assist(resolved.air_assist)
                            .with_constant_power(resolved.constant_power);
                        toolpath.operation_name.clone_from(&resolved.operation_name);
                        toolpaths.push(toolpath);
                    }
                }
            }
        }
    }
    Ok((toolpaths, warnings))
}

/// Build a raster toolpath for one image object.
///
/// Consecutive burned pixels in a row become one cut segment; the image's
/// top-left pixel maps to the object's local top-left corner.
fn raster_toolpath(
    object: &Object,
    image: &crate::objects::ImageObject,
    resolved: &Resolved,
    cancel: &dyn Fn() -> bool,
) -> Result<Option<Toolpath>> {
    let path = std::path::Path::new(&image.path);
    if !path.is_file() {
        return Err(BeamError::InvalidInput(format!(
            "file not found: {}",
            image.path
        )));
    }
    let bitmap = crate::raster::process_object(path, image)?;
    let mut toolpath = Toolpath::new(
        object.name.clone(),
        resolved.speed,
        resolved.power,
        resolved.passes,
    );
    let width_mm = image.width_mm();
    let height_mm = image.height_mm();
    #[allow(clippy::cast_precision_loss)]
    let pixel_x = width_mm / bitmap.width as f64;
    #[allow(clippy::cast_precision_loss)]
    let pixel_y = height_mm / bitmap.height as f64;
    for row in 0..bitmap.height {
        if row % 64 == 0 && cancel() {
            return Err(cancelled());
        }
        let mut run_start: Option<usize> = None;
        for column in 0..=bitmap.width {
            let burned = column < bitmap.width && bitmap.get(column, row);
            match (burned, run_start) {
                (true, None) => run_start = Some(column),
                (false, Some(start)) => {
                    let local = TransformBake {
                        transform: object.transform,
                        pixel_x,
                        pixel_y,
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
        toolpath
            .with_operation(object.operation_id.as_deref())
            .with_operation_name(resolved.operation_name.as_deref()),
    ))
}

struct TransformBake {
    transform: crate::geometry::Transform2,
    pixel_x: f64,
    pixel_y: f64,
    height_px: usize,
}

impl TransformBake {
    fn point(&self, x: usize, y: usize) -> (f64, f64) {
        #[allow(clippy::cast_precision_loss)]
        let local_x = x as f64 * self.pixel_x;
        #[allow(clippy::cast_precision_loss)]
        let local_y = (self.height_px as f64 - y as f64) * self.pixel_y;
        let point = self
            .transform
            .apply(crate::geometry::Point2::new(local_x, local_y));
        (point.x, point.y)
    }
}

/// Build the fill toolpath for one object.
///
/// Handles linear, outlined and offset fills, including the margin, angle,
/// pattern offset and alternating direction of a linear fill.
#[allow(clippy::too_many_lines)]
fn push_fill_toolpath(
    toolpaths: &mut Vec<Toolpath>,
    object: &Object,
    resolved: &Resolved,
    points: &[Point2],
    warnings: &mut Vec<String>,
    cancel: &dyn Fn() -> bool,
) -> Result<()> {
    let mut polygon: Vec<(f64, f64)> = points.iter().map(|point| (point.x, point.y)).collect();
    // Flattened closed paths repeat the first point; offsets would treat it as
    // an extra, degenerate corner.
    if polygon.len() > 2 {
        let first = polygon[0];
        let last = polygon[polygon.len() - 1];
        if (first.0 - last.0).abs() <= 1e-9 && (first.1 - last.1).abs() <= 1e-9 {
            polygon.pop();
        }
    }
    if cancel() {
        return Err(cancelled());
    }
    if resolved.op_type == OperationType::FillWithOffset {
        let mut toolpath = Toolpath::new(
            object.name.clone(),
            resolved.speed,
            resolved.power,
            resolved.passes,
        );
        let loops = append_offset_fill(&mut toolpath, &polygon, resolved.line_interval_mm);
        if loops == 0 {
            warnings.push(format!(
                "'{}' is too small to offset-fill at {} mm spacing",
                object.name, resolved.line_interval_mm
            ));
        } else {
            toolpaths.push(
                toolpath
                    .with_operation(object.operation_id.as_deref())
                    .with_air_assist(resolved.air_assist)
                    .with_constant_power(resolved.constant_power),
            );
        }
        return Ok(());
    }
    let mut fill_polygon = polygon;
    if crate::geometry::boolean::polygon_area(&fill_polygon) < 0.0 {
        fill_polygon.reverse();
    }
    if resolved.fill_margin_mm > 0.0 {
        let inset =
            crate::geometry::offset::offset_polygon_miter(&fill_polygon, -resolved.fill_margin_mm);
        if inset.len() < 3
            || crate::geometry::offset::offset_collapsed(
                &fill_polygon,
                &inset,
                -resolved.fill_margin_mm,
            )
        {
            warnings.push(format!(
                "'{}' is too small to fill with a {} mm margin",
                object.name, resolved.fill_margin_mm
            ));
            return Ok(());
        }
        fill_polygon = inset;
    }
    let options = crate::geometry::fill::FillOptions {
        spacing: resolved.line_interval_mm,
        angle: resolved.fill_angle_degrees.to_radians(),
        offset: resolved.fill_offset_mm,
        alternate: resolved.fill_alternate,
    };
    let mut lines = crate::geometry::fill::fill_lines_with(&fill_polygon, options);
    if resolved.cross_hatch {
        lines.extend(crate::geometry::fill::fill_lines_with(
            &fill_polygon,
            crate::geometry::fill::FillOptions {
                angle: options.angle + std::f64::consts::FRAC_PI_2,
                ..options
            },
        ));
    }
    let overscan = resolved.overscan_mm;
    let lines: Vec<((f64, f64), (f64, f64))> = lines
        .into_iter()
        .map(|((x0, y0), (x1, y1))| {
            if overscan <= 0.0 {
                return ((x0, y0), (x1, y1));
            }
            let dx = x1 - x0;
            let dy = y1 - y0;
            let length = dx.hypot(dy);
            if length <= 1e-9 {
                return ((x0, y0), (x1, y1));
            }
            let ux = dx / length;
            let uy = dy / length;
            (
                (x0 - ux * overscan, y0 - uy * overscan),
                (x1 + ux * overscan, y1 + uy * overscan),
            )
        })
        .collect();
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
    for (index, ((x0, y0), (x1, y1))) in lines.into_iter().enumerate() {
        if index % 512 == 0 && cancel() {
            return Err(cancelled());
        }
        toolpath.push(MoveKind::Travel, x0, y0);
        toolpath.push(MoveKind::Cut, x1, y1);
    }
    if resolved.op_type == OperationType::FillWithOutline {
        append_contour(&mut toolpath, points);
    }
    if !toolpath.is_empty() {
        let mut toolpath = toolpath
            .with_operation(object.operation_id.as_deref())
            .with_air_assist(resolved.air_assist)
            .with_constant_power(resolved.constant_power);
        toolpath.operation_name.clone_from(&resolved.operation_name);
        toolpaths.push(toolpath);
    }
    Ok(())
}

/// Add concentric inward offsets of `polygon` to `toolpath`.
///
/// Each inset is one line interval, which turns an offset fill into closed loops
/// instead of the parallel lines of a linear fill. Returns how many loops were
/// added; zero means the shape is too small for the requested spacing.
fn append_offset_fill(toolpath: &mut Toolpath, polygon: &[(f64, f64)], interval: f64) -> usize {
    use crate::geometry::boolean::polygon_area;
    use crate::geometry::offset::{offset_collapsed, offset_polygon_miter};

    if polygon.len() < 3 || interval <= 0.0 {
        return 0;
    }
    let mut contour = polygon.to_vec();
    if polygon_area(&contour) < 0.0 {
        contour.reverse();
    }
    let mut loops = 0;
    for step in 1..=1000 {
        let inset = interval * f64::from(step);
        let offset = offset_polygon_miter(&contour, -inset);
        if offset.len() < 3
            || offset_collapsed(&contour, &offset, -inset)
            || polygon_area(&offset) <= 1e-9
        {
            break;
        }
        let points: Vec<Point2> = offset.iter().map(|(x, y)| Point2::new(*x, *y)).collect();
        append_contour(toolpath, &points);
        loops += 1;
    }
    loops
}

fn append_contour(toolpath: &mut Toolpath, points: &[Point2]) {
    toolpath.push(MoveKind::Travel, points[0].x, points[0].y);
    for point in &points[1..] {
        toolpath.push(MoveKind::Cut, point.x, point.y);
    }
}

/// Operation identifiers in execution order.
///
/// The panel order breaks ties, but an operation never runs before one it
/// depends on. Dependencies on operations that no longer exist are ignored here;
/// [`Document::validate`] rejects them before a job is written.
pub fn ordered_operation_ids(document: &Document) -> Vec<String> {
    let mut emitted: Vec<String> = Vec::new();
    let mut remaining: Vec<usize> = (0..document.operations.len()).collect();
    while !remaining.is_empty() {
        let mut progressed = false;
        for (position, index) in remaining.iter().enumerate() {
            let operation = &document.operations[*index];
            let ready = operation.depends_on.iter().all(|dependency| {
                !document
                    .operations
                    .iter()
                    .any(|candidate| candidate.id == *dependency)
                    || emitted.iter().any(|id| id == dependency)
            });
            if ready {
                emitted.push(operation.id.clone());
                remaining.remove(position);
                progressed = true;
                break;
            }
        }
        if !progressed {
            // A cycle should have been rejected by validation; keep the panel
            // order for whatever remains instead of looping.
            for index in remaining {
                emitted.push(document.operations[index].id.clone());
            }
            break;
        }
    }
    emitted
}

/// Order toolpaths by the position of their operation in execution order.
///
/// The sort is stable, so objects inside one operation keep their document
/// order, and toolpaths without an operation come last.
pub fn order_by_operation(toolpaths: &mut [Toolpath], document: &Document) {
    let order = ordered_operation_ids(document);
    let rank = |toolpath: &Toolpath| {
        toolpath
            .operation_id
            .as_deref()
            .and_then(|id| order.iter().position(|ordered| ordered == id))
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
pub fn validate_bounds(_document: &Document, toolpaths: &[Toolpath]) -> Result<()> {
    for toolpath in toolpaths {
        for movement in &toolpath.moves {
            if !movement.x.is_finite() || !movement.y.is_finite() {
                return Err(BeamError::InvalidInput(format!(
                    "'{}' contains a non-finite coordinate",
                    toolpath.name
                )));
            }
        }
    }
    Ok(())
}

/// Report geometry that extends beyond the work area.
///
/// Working outside the page is allowed (the job is still generated); the report
/// tells the operator so they can reposition or resize the work area.
pub fn bounds_warnings(document: &Document, toolpaths: &[Toolpath]) -> Vec<String> {
    let tolerance = 1e-6;
    let mut warnings = Vec::new();
    for toolpath in toolpaths {
        let mut outside = false;
        for movement in &toolpath.moves {
            if movement.x < -tolerance
                || movement.y < -tolerance
                || movement.x > document.width_mm + tolerance
                || movement.y > document.height_mm + tolerance
            {
                outside = true;
                break;
            }
        }
        if outside {
            warnings.push(format!(
                "'{}' extends outside the {:.1} x {:.1} mm work area; the job is generated as placed",
                toolpath.name, document.width_mm, document.height_mm
            ));
        }
    }
    warnings
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

/// Move every toolpath so the job starts at (0, 0).
pub fn shift_to_origin(toolpaths: &mut [Toolpath]) {
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    for toolpath in toolpaths.iter() {
        for movement in &toolpath.moves {
            min_x = min_x.min(movement.x);
            min_y = min_y.min(movement.y);
        }
    }
    if !min_x.is_finite() || !min_y.is_finite() {
        return;
    }
    for toolpath in toolpaths.iter_mut() {
        for movement in &mut toolpath.moves {
            movement.x -= min_x;
            movement.y -= min_y;
        }
    }
}

/// Toolpaths in the order they will be executed.
pub fn planned_toolpaths(
    document: &Document,
    options: &JobOptions,
) -> (Vec<Toolpath>, Vec<String>) {
    planned_toolpaths_cancellable(document, options, &|| false).unwrap_or_default()
}

/// Toolpaths in execution order, checking `cancel` while they are built.
pub fn planned_toolpaths_cancellable(
    document: &Document,
    options: &JobOptions,
    cancel: &dyn Fn() -> bool,
) -> Result<(Vec<Toolpath>, Vec<String>)> {
    let (mut toolpaths, warnings) = build_toolpaths_cancellable(document, cancel)?;
    order_by_operation(&mut toolpaths, document);
    if options.assisted_ordering {
        if cancel() {
            return Err(cancelled());
        }
        order_toolpaths(&mut toolpaths);
    }
    Ok((toolpaths, warnings))
}

/// Generate the G-code text and a report without writing a file.
pub fn render_job(document: &Document, options: &JobOptions) -> Result<(String, JobReport)> {
    render_job_cancellable(document, options, &|| false)
}

/// Generate the G-code text, checking `cancel` between phases and paths.
pub fn render_job_cancellable(
    document: &Document,
    options: &JobOptions,
    cancel: &dyn Fn() -> bool,
) -> Result<(String, JobReport)> {
    document.validate().map_err(BeamError::InvalidInput)?;
    let (mut toolpaths, mut warnings) = planned_toolpaths_cancellable(document, options, cancel)?;
    if cancel() {
        return Err(cancelled());
    }
    if options.offset_to_origin {
        shift_to_origin(&mut toolpaths);
    }
    validate_bounds(document, &toolpaths)?;
    for (index, toolpath) in toolpaths.iter().enumerate() {
        if index % 64 == 0 && cancel() {
            return Err(cancelled());
        }
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
    if cancel() {
        return Err(cancelled());
    }
    warnings.extend(bounds_warnings(document, &toolpaths));
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
    export_job_cancellable(document, path, options, &|| false)
}

/// Write the job atomically, checking `cancel` before touching the file.
pub fn export_job_cancellable(
    document: &Document,
    path: &Path,
    options: &JobOptions,
    cancel: &dyn Fn() -> bool,
) -> Result<JobReport> {
    let (text, report) = render_job_cancellable(document, options, cancel)?;
    if cancel() {
        return Err(cancelled());
    }
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
    fn dependencies_override_the_panel_order() {
        let mut document = two_operation_document();
        document.operations.swap(0, 1);
        if let Some(engrave) = document
            .operations
            .iter_mut()
            .find(|operation| operation.id == "engrave")
        {
            engrave.depends_on = vec!["cut".to_string()];
        }
        assert_eq!(
            ordered_operation_ids(&document),
            vec!["cut".to_string(), "engrave".to_string()]
        );
        let options = JobOptions {
            assisted_ordering: false,
            ..JobOptions::default()
        };
        let (text, _) = render_job(&document, &options).unwrap();
        let cut = text.find("F500 S800").expect("cut feed");
        let engrave = text.find("F3000 S150").expect("engrave feed");
        assert!(cut < engrave);
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
                ..OperationParams::default()
            },
            enabled: true,
            depends_on: Vec::new(),
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
            depends_on: Vec::new(),
        });
        document.objects[0].operation_id = Some("op-1".to_string());
        let (toolpaths, warnings) = build_toolpaths(&document);
        assert_eq!(toolpaths.len(), 0);
        assert_eq!(warnings.len(), 0);
    }

    #[test]
    fn out_of_bounds_geometry_is_generated_with_a_warning() {
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
        let (_, report) = render_job(&document, &JobOptions::default()).unwrap();
        assert!(
            report
                .warnings
                .iter()
                .any(|warning| warning.contains("outside")),
            "{:?}",
            report.warnings
        );
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
                ..OperationParams::default()
            },
            enabled: true,
            depends_on: Vec::new(),
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
                ..OperationParams::default()
            },
            enabled: true,
            depends_on: Vec::new(),
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
                ..OperationParams::default()
            },
            enabled: true,
            depends_on: Vec::new(),
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
        assert!(text.contains("(Operation 1: Cut - cut"));
        assert!(text.contains("(Operation 2: Engrave - engrave"));
    }

    fn fill_document(params: OperationParams) -> Document {
        let mut document = Document::default();
        let layer_id = document.layers[0].id.clone();
        document.operations.push(Operation {
            id: "fill".to_string(),
            name: "Fill".to_string(),
            op_type: OperationType::Fill,
            params,
            enabled: true,
            depends_on: Vec::new(),
        });
        let mut object = Object::new(
            layer_id,
            Shape::Rect {
                width: 10.0,
                height: 4.0,
                corner_radius: 0.0,
            },
        );
        object.operation_id = Some("fill".to_string());
        document.add_object(object);
        document
    }

    #[test]
    fn fill_angle_rotates_the_lines() {
        let document = fill_document(OperationParams {
            line_interval_mm: 1.0,
            fill_angle_degrees: 90.0,
            ..OperationParams::default()
        });
        let (toolpaths, warnings) = build_toolpaths(&document);
        assert_eq!(toolpaths.len(), 1);
        assert_eq!(warnings.len(), 0);
        let mut previous_travel_x = None;
        let mut cuts = 0;
        for movement in &toolpaths[0].moves {
            match movement.kind {
                MoveKind::Travel => previous_travel_x = Some(movement.x),
                MoveKind::Cut => {
                    cuts += 1;
                    let travel_x = previous_travel_x.expect("cut after travel");
                    assert!((movement.x - travel_x).abs() < 1e-6);
                }
            }
        }
        assert!(cuts > 0);
    }

    #[test]
    fn fill_margin_insets_the_pattern() {
        let document = fill_document(OperationParams {
            line_interval_mm: 1.0,
            fill_margin_mm: 1.0,
            ..OperationParams::default()
        });
        let (toolpaths, _) = build_toolpaths(&document);
        for movement in &toolpaths[0].moves {
            assert!(movement.x >= 1.0 - 1e-6 && movement.x <= 9.0 + 1e-6);
            assert!(movement.y >= 1.0 - 1e-6 && movement.y <= 3.0 + 1e-6);
        }
    }

    #[test]
    fn fill_margin_that_does_not_fit_is_reported() {
        let document = fill_document(OperationParams {
            line_interval_mm: 1.0,
            fill_margin_mm: 5.0,
            ..OperationParams::default()
        });
        let (toolpaths, warnings) = build_toolpaths(&document);
        assert_eq!(toolpaths.len(), 0);
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("margin"));
    }

    #[test]
    fn alternating_fill_reverses_every_other_line() {
        let document = fill_document(OperationParams {
            line_interval_mm: 1.0,
            fill_alternate: true,
            ..OperationParams::default()
        });
        let (toolpaths, _) = build_toolpaths(&document);
        let mut starts = Vec::new();
        let mut previous_cut: Option<(f64, f64)> = None;
        for movement in &toolpaths[0].moves {
            if movement.kind == MoveKind::Cut {
                if let Some(previous) = previous_cut {
                    if (movement.y - previous.1).abs() > 0.5 {
                        starts.push(movement.x);
                    }
                }
                previous_cut = Some((movement.x, movement.y));
            }
        }
        assert!(starts.len() >= 2);
        // Consecutive rows start on opposite sides of the shape.
        assert!((starts[0] - starts[1]).abs() > 5.0);
    }

    #[test]
    fn offset_fill_creates_concentric_loops() {
        let mut document = Document::default();
        let layer_id = document.layers[0].id.clone();
        let operation = Operation {
            id: "offset".to_string(),
            name: "Offset".to_string(),
            op_type: OperationType::FillWithOffset,
            params: OperationParams {
                speed: 1000.0,
                power: 50.0,
                passes: 1,
                line_interval_mm: 1.0,
                ..OperationParams::default()
            },
            enabled: true,
            depends_on: Vec::new(),
        };
        document.operations.push(operation);
        let mut object = Object::new(
            layer_id,
            Shape::Rect {
                width: 10.0,
                height: 10.0,
                corner_radius: 0.0,
            },
        );
        object.operation_id = Some("offset".to_string());
        document.add_object(object);
        let (toolpaths, warnings) = build_toolpaths(&document);
        assert_eq!(toolpaths.len(), 1);
        assert_eq!(warnings.len(), 0);
        let travels = toolpaths[0]
            .moves
            .iter()
            .filter(|movement| movement.kind == MoveKind::Travel)
            .count();
        // A 10 x 10 square shrinks by 1 mm per side: 8, 6, 4 and 2 mm loops.
        assert_eq!(travels, 4);
        assert!(toolpaths[0].cut_length() > 0.0);
    }

    #[test]
    fn offset_fill_reports_a_shape_that_is_too_small() {
        let mut document = Document::default();
        let layer_id = document.layers[0].id.clone();
        document.operations.push(Operation {
            id: "offset".to_string(),
            name: "Offset".to_string(),
            op_type: OperationType::FillWithOffset,
            params: OperationParams {
                speed: 1000.0,
                power: 50.0,
                passes: 1,
                line_interval_mm: 2.0,
                ..OperationParams::default()
            },
            enabled: true,
            depends_on: Vec::new(),
        });
        let mut object = Object::new(
            layer_id,
            Shape::Rect {
                width: 2.0,
                height: 2.0,
                corner_radius: 0.0,
            },
        );
        object.operation_id = Some("offset".to_string());
        document.add_object(object);
        let (toolpaths, warnings) = build_toolpaths(&document);
        assert_eq!(toolpaths.len(), 0);
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("offset-fill"));
    }

    #[test]
    fn job_generation_can_be_cancelled() {
        let document = document_with_rect();
        assert!(render_job_cancellable(&document, &JobOptions::default(), &|| false).is_ok());
        let error =
            render_job_cancellable(&document, &JobOptions::default(), &|| true).unwrap_err();
        assert!(error.to_string().contains("cancelled"));

        let path = std::env::temp_dir().join("bw_cancelled_job.nc");
        let _ = std::fs::remove_file(&path);
        let error =
            export_job_cancellable(&document, &path, &JobOptions::default(), &|| true).unwrap_err();
        assert!(error.to_string().contains("cancelled"));
        assert!(!path.exists());
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
                ..crate::objects::TextObject::default()
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
                ..OperationParams::default()
            },
            enabled: true,
            depends_on: Vec::new(),
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
    fn rotated_image_swaps_its_physical_size() {
        let path = std::env::temp_dir().join("beamwright_job_rotated.png");
        let mut picture = image::GrayImage::new(4, 2);
        for (x, _y, pixel) in picture.enumerate_pixels_mut() {
            *pixel = if x < 2 {
                image::Luma([0])
            } else {
                image::Luma([255])
            };
        }
        picture.save(&path).unwrap();
        let mut document = image_document(&path);
        if let Shape::Image(image) = &mut document.objects[0].shape {
            image.settings.rotation_degrees = 90.0;
        }
        let (toolpaths, warnings) = build_toolpaths(&document);
        assert_eq!(warnings.len(), 0);
        let Shape::Image(image) = &document.objects[0].shape else {
            panic!("expected image");
        };
        assert!(image.width_mm() < image.height_mm());
        let width = image.width_mm();
        let height = image.height_mm();
        for movement in &toolpaths[0].moves {
            assert!(movement.x >= -1e-9 && movement.x <= width + 1e-9);
            assert!(movement.y >= -1e-9 && movement.y <= height + 1e-9);
        }
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
