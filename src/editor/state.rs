use crate::document::serialization;
use crate::document::shape::Shape;
use crate::document::{Document, Object};
use crate::errors::Result;
use crate::geometry::{Path, Point2, Transform2, path::Segment};
use crate::history::History;

use super::canvas::CanvasState;
use super::hit_test::distance_point_to_segment;
use super::selection::Selection;
use super::tools::Tool;

/// Grid duplicate settings.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GridOptions {
    pub columns: usize,
    pub rows: usize,
    /// Spacing between columns and rows, in millimetres.
    pub dx: f64,
    pub dy: f64,
    /// Offset of the whole grid from the original, in millimetres.
    pub margin_x: f64,
    pub margin_y: f64,
    /// Fill column by column instead of row by row.
    pub column_major: bool,
}

impl Default for GridOptions {
    fn default() -> Self {
        Self {
            columns: 2,
            rows: 2,
            dx: 10.0,
            dy: 10.0,
            margin_x: 0.0,
            margin_y: 0.0,
            column_major: false,
        }
    }
}

/// How node handles are drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HandleStyle {
    #[default]
    Square,
    Circle,
}

impl HandleStyle {
    pub fn label(self) -> &'static str {
        match self {
            HandleStyle::Square => "Square",
            HandleStyle::Circle => "Circle",
        }
    }

    pub fn from_label(label: &str) -> Self {
        match label {
            "circle" => HandleStyle::Circle,
            _ => HandleStyle::Square,
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            HandleStyle::Square => "square",
            HandleStyle::Circle => "circle",
        }
    }
}

/// What transformations rotate and scale around.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TransformCenter {
    /// The centre of the selection's bounds.
    #[default]
    Selection,
    /// The centre of the work area.
    Page,
    /// Each object's own centre.
    Object,
}

/// Copy distribution modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepeatMode {
    Linear,
    Circular,
}

/// Parameters for [`EditorState::repeat_selection`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RepeatOptions {
    pub mode: RepeatMode,
    /// Number of copies to create.
    pub copies: usize,
    /// Linear step per copy, in millimetres.
    pub dx: f64,
    pub dy: f64,
    /// Circular orbit radius, in millimetres.
    pub radius: f64,
    /// Rotate each linear copy by this angle, or circular copies along the orbit.
    pub rotation_degrees: f64,
    pub rotate_copies: bool,
}

impl Default for RepeatOptions {
    fn default() -> Self {
        Self {
            mode: RepeatMode::Linear,
            copies: 3,
            dx: 10.0,
            dy: 0.0,
            radius: 20.0,
            rotation_degrees: 0.0,
            rotate_copies: true,
        }
    }
}

/// One straight segment in world space.
type LineSegment = ((f64, f64), (f64, f64));

/// An object cut by the trimmer, ready to be replaced by its kept pieces.
struct Trimmed {
    index: usize,
    pieces: Vec<Vec<(f64, f64)>>,
    closed: bool,
}

/// One contour produced by an outline: points plus whether it is closed.
type OutlineContour = (Vec<(f64, f64)>, bool);

/// An object replaced by its stroke outline.
struct Outlined {
    index: usize,
    contours: Vec<OutlineContour>,
}

/// One crossing between the outlines of two objects.
#[derive(Debug, Clone, PartialEq)]
pub struct PathIntersection {
    pub first: String,
    pub second: String,
    pub point: Point2,
}

/// Testable editor state: document, view and interaction state without egui.
// Snapping targets are independent toggles; one enum would not model a user
// enabling several of them at once.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone)]
pub struct EditorState {
    pub document: Document,
    pub history: History,
    pub canvas: CanvasState,
    pub tool: Tool,
    pub selection: Selection,
    pub active_layer_id: String,
    pub snap_enabled: bool,
    pub snap_step_mm: f64,
    /// Snap drawing to path nodes (segment endpoints).
    pub snap_nodes: bool,
    /// Snap drawing to the ends of open paths.
    pub snap_endpoints: bool,
    /// Snap drawing to object bounding-box centres.
    pub snap_centers: bool,
    /// Snap drawing to the midpoint of every segment.
    pub snap_midpoints: bool,
    /// Snap drawing to crossings between other objects' outlines.
    pub snap_intersections: bool,
    /// Snap drawing to the corners and edge midpoints of object bounds.
    pub snap_limits: bool,
    /// Pull radius in screen pixels; converted to millimetres by the zoom.
    pub snap_tolerance_px: f64,
    /// Draw millimetre rulers along the top and left edges.
    pub show_rulers: bool,
    /// Draw selection outlines and handles around selected objects.
    pub show_selection: bool,
    /// Colour used for selection outlines and handles.
    pub selection_color: String,
    /// Shape used for node handles.
    pub handle_style: HandleStyle,
    /// Zoom factor applied by the zoom commands.
    pub zoom_step: f64,
    /// Clicking one member of a group selects the whole group.
    pub select_whole_group: bool,
    /// Zoom factor applied by the mouse wheel.
    pub scroll_zoom_step: f64,
    /// Draw the document guides.
    pub guides_visible: bool,
    /// Snap drawing to visible guides.
    pub snap_guides: bool,
    /// Centre used by rotate, scale and flip.
    pub transform_center: TransformCenter,
    /// Path object whose nodes are being edited.
    pub node_object: Option<String>,
    /// Indices of the selected nodes in that path.
    pub node_selection: Vec<usize>,
}

impl EditorState {
    pub fn new(document: Document) -> Self {
        let active_layer_id = document
            .layers
            .iter()
            .max_by_key(|layer| layer.order)
            .map(|layer| layer.id.clone())
            .unwrap_or_default();
        Self {
            document,
            history: History::default(),
            canvas: CanvasState::default(),
            tool: Tool::default(),
            selection: Selection::default(),
            active_layer_id,
            snap_enabled: false,
            snap_step_mm: 1.0,
            snap_nodes: false,
            snap_endpoints: false,
            snap_centers: false,
            snap_midpoints: false,
            snap_intersections: false,
            snap_limits: false,
            snap_tolerance_px: 8.0,
            show_rulers: true,
            show_selection: true,
            selection_color: crate::theme::CANVAS_SELECTION.to_string(),
            handle_style: HandleStyle::Square,
            zoom_step: 1.25,
            select_whole_group: true,
            scroll_zoom_step: 1.15,
            guides_visible: true,
            snap_guides: true,
            transform_center: TransformCenter::Selection,
            node_object: None,
            node_selection: Vec::new(),
        }
    }

    pub fn record(&mut self) {
        self.record_action("Edit");
    }

    /// Record the state before a change together with a user-facing action
    /// name, so undo and redo can say what they will do.
    pub fn record_action(&mut self, action: &str) {
        self.history.record(&self.document, action);
    }

    pub fn set_document(&mut self, document: Document) {
        self.document = document;
        self.selection.clear();
        self.history.clear();
        self.active_layer_id = self
            .document
            .layers
            .iter()
            .max_by_key(|layer| layer.order)
            .map(|layer| layer.id.clone())
            .unwrap_or_default();
    }

    /// Apply the enabled snapping modes to a drawing position.
    ///
    /// Object snapping (nodes, open-path ends, object centres) wins over the
    /// grid, because it is the more specific target. The pull radius is
    /// `snap_tolerance_px` screen pixels converted with the current zoom.
    pub fn snap(&self, point: Point2) -> Point2 {
        if let Some(candidate) = self.object_snap(point) {
            return candidate;
        }
        if !self.snap_enabled || self.snap_step_mm <= 0.0 {
            return point;
        }
        Point2::new(
            (point.x / self.snap_step_mm).round() * self.snap_step_mm,
            (point.y / self.snap_step_mm).round() * self.snap_step_mm,
        )
    }

    /// Nearest enabled snap target within the pull radius, if any.
    pub fn object_snap(&self, point: Point2) -> Option<Point2> {
        if !(self.snap_nodes
            || self.snap_endpoints
            || self.snap_centers
            || self.snap_midpoints
            || self.snap_intersections
            || self.snap_limits
            || self.snap_guides)
        {
            return None;
        }
        let zoom = if self.canvas.zoom > 0.0 {
            self.canvas.zoom
        } else {
            1.0
        };
        let tolerance = (self.snap_tolerance_px / zoom).max(0.001);
        let mut best: Option<(f64, Point2)> = None;
        let mut consider = |candidate: Point2| {
            let distance = point.distance(candidate);
            if distance <= tolerance && best.is_none_or(|(current, _)| distance < current) {
                best = Some((distance, candidate));
            }
        };
        for object in &self.document.objects {
            if !object.visible {
                continue;
            }
            for candidate in self.object_snap_candidates(object) {
                consider(candidate);
            }
        }
        if self.snap_guides && self.guides_visible {
            for guide in &self.document.guides {
                consider(if guide.horizontal {
                    Point2::new(point.x, guide.position)
                } else {
                    Point2::new(guide.position, point.y)
                });
            }
        }
        if self.snap_intersections {
            for candidate in self.intersections_near(point, tolerance) {
                consider(candidate);
            }
        }
        best.map(|(_, candidate)| candidate)
    }

    /// Nodes, endpoints, midpoints, centres and bounds that can be snapped to.
    fn object_snap_candidates(&self, object: &crate::document::Object) -> Vec<Point2> {
        let mut candidates = Vec::new();
        let path = object.world_path();
        if path.segments.is_empty() {
            return candidates;
        }
        if self.snap_nodes {
            for segment in &path.segments {
                match *segment {
                    Segment::Line { from, to } => {
                        candidates.push(from);
                        candidates.push(to);
                    }
                    Segment::Cubic(c) => {
                        candidates.push(c.p0);
                        candidates.push(c.p3);
                    }
                }
            }
        }
        if self.snap_endpoints && !path.closed {
            if let Some(start) = path.start_point() {
                candidates.push(start);
            }
            if let Some(end) = path.last_point() {
                candidates.push(end);
            }
        }
        if self.snap_midpoints {
            for segment in &path.segments {
                candidates.push(match *segment {
                    Segment::Line { from, to } => {
                        Point2::new(f64::midpoint(from.x, to.x), f64::midpoint(from.y, to.y))
                    }
                    Segment::Cubic(cubic) => cubic.eval(0.5),
                });
            }
        }
        if (self.snap_centers || self.snap_limits) && object.bounds().is_some() {
            if let Some((min, max)) = object.bounds() {
                if self.snap_centers {
                    candidates.push(Point2::new(
                        f64::midpoint(min.x, max.x),
                        f64::midpoint(min.y, max.y),
                    ));
                }
                if self.snap_limits {
                    candidates.extend([
                        Point2::new(min.x, min.y),
                        Point2::new(max.x, min.y),
                        Point2::new(max.x, max.y),
                        Point2::new(min.x, max.y),
                        Point2::new(f64::midpoint(min.x, max.x), min.y),
                        Point2::new(max.x, f64::midpoint(min.y, max.y)),
                        Point2::new(f64::midpoint(min.x, max.x), max.y),
                        Point2::new(min.x, f64::midpoint(min.y, max.y)),
                    ]);
                }
            }
        }
        candidates
    }

    /// Crossings between different objects' segments near `point`.
    ///
    /// Only segments that pass close to the pointer can contain a crossing near
    /// it, so the search stays local instead of testing every pair.
    fn intersections_near(&self, point: Point2, tolerance: f64) -> Vec<Point2> {
        use crate::geometry::intersection::{Segment as Line, segment_crossing_point};

        let mut segments: Vec<Line> = Vec::new();
        let near = |line: &Line| {
            crate::geometry::intersection::point_segment_distance(
                point.x, point.y, line.x1, line.y1, line.x2, line.y2,
            ) <= tolerance * 4.0
        };
        for object in &self.document.objects {
            if !object.visible {
                continue;
            }
            for segment in object.world_path().segments {
                // Curves are approximated by their two halves, which is enough
                // to find a crossing near the pointer.
                let lines: Vec<Line> = match segment {
                    Segment::Line { from, to } => vec![Line {
                        x1: from.x,
                        y1: from.y,
                        x2: to.x,
                        y2: to.y,
                    }],
                    Segment::Cubic(cubic) => {
                        let middle = cubic.eval(0.5);
                        vec![
                            Line {
                                x1: cubic.p0.x,
                                y1: cubic.p0.y,
                                x2: middle.x,
                                y2: middle.y,
                            },
                            Line {
                                x1: middle.x,
                                y1: middle.y,
                                x2: cubic.p3.x,
                                y2: cubic.p3.y,
                            },
                        ]
                    }
                };
                segments.extend(lines.into_iter().filter(near));
            }
        }
        let mut points = Vec::new();
        for (index, first) in segments.iter().enumerate() {
            for second in segments.iter().skip(index + 1) {
                if let Some((x, y)) = segment_crossing_point(first, second) {
                    let candidate = Point2::new(x, y);
                    if candidate.distance(point) <= tolerance
                        && points
                            .iter()
                            .all(|existing: &Point2| existing.distance(candidate) > 1e-6)
                    {
                        points.push(candidate);
                    }
                }
            }
        }
        points
    }

    pub fn add_rectangle(&mut self, start: Point2, end: Point2) -> Option<String> {
        let start = self.snap(start);
        let end = self.snap(end);
        let min_x = start.x.min(end.x);
        let min_y = start.y.min(end.y);
        let width = (end.x - start.x).abs();
        let height = (end.y - start.y).abs();
        if width < 0.01 || height < 0.01 {
            return None;
        }
        let shape = Shape::Rect {
            width,
            height,
            corner_radius: 0.0,
        };
        self.add_shape(shape, Transform2::translate(min_x, min_y))
    }

    pub fn add_ellipse(&mut self, start: Point2, end: Point2) -> Option<String> {
        let start = self.snap(start);
        let end = self.snap(end);
        let rx = (end.x - start.x).abs() / 2.0;
        let ry = (end.y - start.y).abs() / 2.0;
        if rx < 0.005 || ry < 0.005 {
            return None;
        }
        let center = Point2::new(f64::midpoint(start.x, end.x), f64::midpoint(start.y, end.y));
        self.add_shape(
            Shape::Ellipse { rx, ry },
            Transform2::translate(center.x, center.y),
        )
    }

    /// Create an open path from a freehand stroke, dropping duplicate points.
    pub fn add_polyline(&mut self, points: &[Point2], closed: bool) -> Option<String> {
        let mut simplified: Vec<(f64, f64)> = Vec::new();
        for point in points {
            let duplicate = simplified
                .last()
                .is_some_and(|(x, y)| (x - point.x).hypot(y - point.y) < 0.05);
            if !duplicate {
                simplified.push((point.x, point.y));
            }
        }
        if simplified.len() < 2 {
            return None;
        }
        let path = Path::from_points(&simplified, closed);
        self.add_shape(Shape::Path(path), Transform2::identity())
    }

    /// Create a regular polygon centred on `center` with the first vertex at
    /// `edge`, closing the outline.
    pub fn add_polygon(&mut self, center: Point2, edge: Point2, sides: usize) -> Option<String> {
        if !(3..=128).contains(&sides) {
            return None;
        }
        let radius = center.distance(edge);
        if radius < 0.01 {
            return None;
        }
        let start_angle = (edge.y - center.y).atan2(edge.x - center.x);
        let points: Vec<(f64, f64)> = (0..sides)
            .map(|index| {
                #[allow(clippy::cast_precision_loss)]
                let angle = start_angle + std::f64::consts::TAU * index as f64 / sides as f64;
                (
                    center.x + radius * angle.cos(),
                    center.y + radius * angle.sin(),
                )
            })
            .collect();
        let path = Path::from_points(&points, true);
        self.add_shape(Shape::Path(path), Transform2::identity())
    }

    /// Create a star centred on `center` with one point towards `edge`.
    ///
    /// `points` counts the outer tips (3..=64) and `inner_ratio` is the inner
    /// radius as a fraction of the outer radius (0.05..=1).
    pub fn add_star(
        &mut self,
        center: Point2,
        edge: Point2,
        points: usize,
        inner_ratio: f64,
    ) -> Option<String> {
        if !(3..=64).contains(&points) || !(0.05..=1.0).contains(&inner_ratio) {
            return None;
        }
        let radius = center.distance(edge);
        if radius < 0.01 {
            return None;
        }
        let inner = radius * inner_ratio;
        let start_angle = (edge.y - center.y).atan2(edge.x - center.x);
        #[allow(clippy::cast_precision_loss)]
        let steps = points * 2;
        let vertices: Vec<(f64, f64)> = (0..steps)
            .map(|index| {
                #[allow(clippy::cast_precision_loss)]
                let angle = start_angle + std::f64::consts::TAU * index as f64 / steps as f64;
                let distance = if index % 2 == 0 { radius } else { inner };
                (
                    center.x + distance * angle.cos(),
                    center.y + distance * angle.sin(),
                )
            })
            .collect();
        self.add_shape(
            Shape::Path(Path::from_points(&vertices, true)),
            Transform2::identity(),
        )
    }

    /// Replace a text object with vector outlines produced by the bundled font.
    ///
    /// Each closed contour becomes its own path object (holes included) in the
    /// document, so the result no longer depends on any font being installed.
    /// Returns the new object identifiers. The conversion is undoable.
    pub fn convert_text_to_outlines(
        &mut self,
        object_id: &str,
    ) -> std::result::Result<Vec<String>, String> {
        self.convert_text_to_outlines_with_font(object_id, None)
    }

    /// Convert text to outlines with the font file chosen for the object.
    ///
    /// `None` uses the bundled font, which is also the fallback when the chosen
    /// font cannot be read.
    pub fn convert_text_to_outlines_with_font(
        &mut self,
        object_id: &str,
        font_data: Option<&[u8]>,
    ) -> std::result::Result<Vec<String>, String> {
        let index = self
            .document
            .objects
            .iter()
            .position(|object| object.id == object_id)
            .ok_or_else(|| format!("unknown object: {object_id}"))?;
        self.ensure_editable(index)?;
        let object = self.document.objects[index].clone();
        let Shape::Text(text) = &object.shape else {
            return Err(format!("'{}' is not text", object.name));
        };
        let paths = match font_data {
            Some(data) => crate::objects::text_to_paths_with_font(text, data)?,
            None => crate::objects::text_to_paths(text)?,
        };
        if paths.is_empty() {
            return Err("the text has no printable characters".to_string());
        }
        self.record_action("Text to outlines");
        let mut style = object.style.clone();
        if style.fill.is_none() && style.stroke.is_none() {
            style.fill = Some("#000000".to_string());
        }
        self.document.objects.remove(index);
        self.selection.clear();
        let mut ids = Vec::with_capacity(paths.len());
        for (position, path) in paths.into_iter().enumerate() {
            let path = path.transform(&object.transform);
            let mut outline =
                Object::new(object.layer_id.clone(), Shape::Path(path)).with_style(style.clone());
            outline.name = format!("{} outline {}", object.name, position + 1);
            outline.operation_id.clone_from(&object.operation_id);
            let id = outline.id.clone();
            ids.push(id.clone());
            self.selection.add(id);
            self.document.objects.insert(index + position, outline);
        }
        Ok(ids)
    }

    /// Lay a text's first line along another object's outline.
    ///
    /// The text object is replaced by its outline contours, which follow the
    /// path's arc length and rotate with its tangent. The path object is kept.
    /// Returns the new object identifiers.
    pub fn fit_text_to_path(
        &mut self,
        text_id: &str,
        path_id: &str,
    ) -> std::result::Result<Vec<String>, String> {
        self.fit_text_to_path_with_font(text_id, path_id, None)
    }

    /// Fit text to a path with the font file chosen for the object.
    pub fn fit_text_to_path_with_font(
        &mut self,
        text_id: &str,
        path_id: &str,
        font_data: Option<&[u8]>,
    ) -> std::result::Result<Vec<String>, String> {
        let text_index = self
            .document
            .objects
            .iter()
            .position(|object| object.id == text_id)
            .ok_or_else(|| format!("unknown object: {text_id}"))?;
        let path_index = self
            .document
            .objects
            .iter()
            .position(|object| object.id == path_id)
            .ok_or_else(|| format!("unknown object: {path_id}"))?;
        if text_index == path_index {
            return Err("the text and the path must be different objects".to_string());
        }
        self.ensure_editable(text_index)?;
        let text_object = self.document.objects[text_index].clone();
        let Shape::Text(text) = &text_object.shape else {
            return Err(format!("'{}' is not text", text_object.name));
        };
        let path = self.document.objects[path_index].world_path();
        if path.is_empty() {
            return Err("the target object has no path".to_string());
        }
        let paths = match font_data {
            Some(data) => crate::objects::text_along_path_with_font(text, &path, data)?,
            None => crate::objects::text_along_path(text, &path)?,
        };
        if paths.is_empty() {
            return Err("the text could not be placed on the path".to_string());
        }
        self.record_action("Fit text to path");
        let mut style = text_object.style.clone();
        if style.fill.is_none() && style.stroke.is_none() {
            style.fill = Some("#000000".to_string());
        }
        self.document.objects.remove(text_index);
        self.selection.clear();
        let mut ids = Vec::with_capacity(paths.len());
        for (position, outline_path) in paths.into_iter().enumerate() {
            let mut outline = Object::new(text_object.layer_id.clone(), Shape::Path(outline_path))
                .with_style(style.clone());
            outline.name = format!("{} path {}", text_object.name, position + 1);
            outline.operation_id.clone_from(&text_object.operation_id);
            let id = outline.id.clone();
            ids.push(id.clone());
            self.selection.add(id);
            self.document.objects.insert(text_index + position, outline);
        }
        Ok(ids)
    }

    /// Point an image object at another file.
    ///
    /// The new pixel size replaces the stored one; a crop that no longer fits is
    /// cleared instead of leaving an invalid object.
    pub fn relink_image(
        &mut self,
        object_id: &str,
        path: &std::path::Path,
        width_px: u32,
        height_px: u32,
    ) -> std::result::Result<(), String> {
        let index = self
            .document
            .objects
            .iter()
            .position(|object| object.id == object_id)
            .ok_or_else(|| format!("unknown object: {object_id}"))?;
        self.ensure_editable(index)?;
        let object = &self.document.objects[index];
        if !matches!(object.shape, Shape::Image(_)) {
            return Err(format!("'{}' is not an image", object.name));
        }
        if width_px == 0 || height_px == 0 {
            return Err("the image has no pixels".to_string());
        }
        self.record_action("Relink image");
        let object = &mut self.document.objects[index];
        let Shape::Image(image) = &mut object.shape else {
            return Ok(());
        };
        image.path = path.to_string_lossy().to_string();
        image.width_px = width_px;
        image.height_px = height_px;
        if let Some(crop) = image.crop {
            if crop.x >= width_px
                || crop.y >= height_px
                || crop.width > width_px - crop.x
                || crop.height > height_px - crop.y
            {
                image.crop = None;
            }
        }
        Ok(())
    }

    /// Create a path from editable nodes (the pen tool's output).
    pub fn add_bezier_nodes(
        &mut self,
        nodes: &[crate::geometry::path::Node],
        closed: bool,
    ) -> Option<String> {
        if nodes.len() < 2 {
            return None;
        }
        self.add_shape(
            Shape::Path(Path::from_nodes(nodes, closed)),
            Transform2::identity(),
        )
    }

    /// Duplicate a layer with all of its objects.
    ///
    /// The copy gets a new layer id, fresh object ids and the next layer order,
    /// so it stays independent from the original.
    pub fn duplicate_layer(&mut self, layer_id: &str) -> Option<String> {
        let layer = self.document.layer(layer_id)?.clone();
        let objects: Vec<Object> = self
            .document
            .objects
            .iter()
            .filter(|object| object.layer_id == layer_id)
            .cloned()
            .collect();
        self.record_action("Duplicate layer");
        let mut copy = layer;
        copy.id = uuid::Uuid::new_v4().to_string();
        copy.name = format!("{} copy", copy.name);
        copy.order = self
            .document
            .layers
            .iter()
            .map(|layer| layer.order)
            .max()
            .unwrap_or(0)
            + 1;
        let new_id = copy.id.clone();
        self.document.layers.push(copy);
        for mut object in objects {
            object.id = uuid::Uuid::new_v4().to_string();
            object.layer_id.clone_from(&new_id);
            object.group_id = None;
            self.document.add_object(object);
        }
        Some(new_id)
    }

    /// Select every object of a layer.
    pub fn select_layer_objects(&mut self, layer_id: &str) -> usize {
        let ids: Vec<String> = self
            .document
            .objects
            .iter()
            .filter(|object| object.layer_id == layer_id)
            .map(|object| object.id.clone())
            .collect();
        self.selection.clear();
        for id in &ids {
            self.selection.add(id.clone());
        }
        ids.len()
    }

    /// Select every object whose stroke or fill is `color`.
    pub fn select_by_color(&mut self, color: &str) -> usize {
        let Some(normalized) = crate::color::normalize_hex_color(color) else {
            return 0;
        };
        let ids: Vec<String> = self
            .document
            .objects
            .iter()
            .filter(|object| {
                [&object.style.stroke, &object.style.fill]
                    .into_iter()
                    .flatten()
                    .filter_map(|value| crate::color::normalize_hex_color(value))
                    .any(|value| value == normalized)
            })
            .map(|object| object.id.clone())
            .collect();
        self.selection.clear();
        for id in &ids {
            self.selection.add(id.clone());
        }
        ids.len()
    }

    /// Group every object of a layer into one group.
    pub fn group_layer_objects(&mut self, layer_id: &str) -> usize {
        let count = self.select_layer_objects(layer_id);
        if count < 2 || !self.group_selection() {
            return 0;
        }
        count
    }

    /// Add a horizontal or vertical guide at a document position.
    pub fn add_guide(&mut self, horizontal: bool, position: f64) -> Option<String> {
        if !position.is_finite() {
            return None;
        }
        self.record_action("Add guide");
        Some(self.document.add_guide(horizontal, position).id.clone())
    }

    /// Remove a guide by id.
    pub fn remove_guide(&mut self, id: &str) -> bool {
        if self.document.guides.iter().all(|guide| guide.id != id) {
            return false;
        }
        self.record_action("Remove guide");
        self.document.remove_guide(id)
    }

    /// Insert a text object whose anchor is `at`, selected and ready to edit.
    pub fn add_text(&mut self, at: Point2) -> Option<String> {
        let text = crate::objects::TextObject {
            x: at.x,
            y: at.y,
            ..crate::objects::TextObject::default()
        };
        self.add_shape(Shape::Text(text), Transform2::identity())
    }

    pub fn add_line(&mut self, start: Point2, end: Point2) -> Option<String> {
        let start = self.snap(start);
        let end = self.snap(end);
        if start.distance(end) < 0.01 {
            return None;
        }
        let path = Path {
            segments: vec![Segment::Line {
                from: start,
                to: end,
            }],
            closed: false,
        };
        self.add_shape(Shape::Path(path), Transform2::identity())
    }

    fn add_shape(&mut self, shape: Shape, transform: Transform2) -> Option<String> {
        if self.active_layer_id.is_empty() {
            self.active_layer_id = self.document.layers.first()?.id.clone();
        }
        let layer = self.document.layer(&self.active_layer_id)?;
        if layer.locked || !layer.visible {
            return None;
        }
        self.record_action(&format!("Draw {}", shape.kind_name()));
        let object = Object::new(self.active_layer_id.clone(), shape).with_transform(transform);
        let id = object.id.clone();
        self.document.add_object(object);
        self.selection.clear();
        self.selection.add(id.clone());
        Some(id)
    }

    /// World-space outline of a closed object, flattened to a polygon.
    fn closed_world_contour(object: &Object) -> Option<Vec<(f64, f64)>> {
        let path = object.world_path();
        if !path.is_closed() {
            return None;
        }
        let mut points: Vec<(f64, f64)> = path
            .flatten(0.2)
            .into_iter()
            .map(|point| (point.x, point.y))
            .collect();
        if points.len() > 1 {
            let first = points[0];
            let last = points[points.len() - 1];
            if (first.0 - last.0).abs() <= 1e-9 && (first.1 - last.1).abs() <= 1e-9 {
                points.pop();
            }
        }
        if points.len() < 3 { None } else { Some(points) }
    }

    /// Selected object indices, in document order.
    pub(crate) fn selected_indices(&self) -> Vec<usize> {
        self.document
            .objects
            .iter()
            .enumerate()
            .filter(|(_, object)| self.selection.contains(&object.id))
            .map(|(index, _)| index)
            .collect()
    }

    pub(crate) fn ensure_editable(&self, index: usize) -> std::result::Result<(), String> {
        let object = &self.document.objects[index];
        if object.locked {
            return Err(format!("'{}' is locked", object.name));
        }
        if self.layer_locked(&object.layer_id) {
            return Err(format!("the layer of '{}' is locked", object.name));
        }
        Ok(())
    }

    /// Combine exactly two closed shapes with a boolean operation.
    ///
    /// The earlier object in document order is the subject, so `Difference`
    /// subtracts the later (upper) object from the earlier one. One path object
    /// is created per resulting contour, keeping the subject's style, layer and
    /// operation; the two originals are removed.
    pub fn boolean_selection(
        &mut self,
        op: crate::geometry::BooleanOp,
    ) -> std::result::Result<usize, String> {
        use crate::geometry::boolean::{boolean_contours, polygon_area};

        let selected = self.selected_indices();
        if selected.len() != 2 {
            return Err("Select exactly two closed shapes".to_string());
        }
        let (first_index, second_index) = (selected[0], selected[1]);
        self.ensure_editable(first_index)?;
        self.ensure_editable(second_index)?;
        let subject =
            Self::closed_world_contour(&self.document.objects[first_index]).ok_or_else(|| {
                format!(
                    "'{}' is not a closed shape",
                    self.document.objects[first_index].name
                )
            })?;
        let clip =
            Self::closed_world_contour(&self.document.objects[second_index]).ok_or_else(|| {
                format!(
                    "'{}' is not a closed shape",
                    self.document.objects[second_index].name
                )
            })?;
        let mut contours = boolean_contours(op, &subject, &clip);
        if contours.is_empty() {
            return Err(format!("{} leaves no area", op.label()));
        }
        for contour in &mut contours {
            if polygon_area(contour) < 0.0 {
                contour.reverse();
            }
        }
        contours.sort_by(|left, right| {
            polygon_area(right)
                .partial_cmp(&polygon_area(left))
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        self.record_action("Boolean");
        let template = self.document.objects[first_index].clone();
        self.document.objects.remove(second_index);
        self.document.objects.remove(first_index);
        let insert_at = first_index.min(self.document.objects.len());
        self.selection.clear();
        for (index, contour) in contours.iter().enumerate() {
            let name = if contours.len() == 1 {
                format!("{} {}", op.label(), template.name)
            } else {
                format!("{} {} {}", op.label(), template.name, index + 1)
            };
            let mut object = Object::new(
                template.layer_id.clone(),
                Shape::Path(Path::from_points(contour, true)),
            )
            .with_style(template.style.clone());
            object.name = name;
            object.operation_id.clone_from(&template.operation_id);
            self.selection.add(object.id.clone());
            self.document.objects.insert(insert_at + index, object);
        }
        Ok(contours.len())
    }

    /// Outline of an object flattened to straight segments, if it has geometry.
    fn outline_segments(object: &Object) -> Vec<crate::geometry::Segment> {
        let points = object.world_path().flatten(0.2);
        points
            .windows(2)
            .filter_map(|pair| {
                let (from, to) = (pair[0], pair[1]);
                if from.distance(to) <= 1e-9 {
                    None
                } else {
                    Some(crate::geometry::Segment {
                        x1: from.x,
                        y1: from.y,
                        x2: to.x,
                        y2: to.y,
                    })
                }
            })
            .collect()
    }

    /// Geometry signature used to find duplicates: closed flag plus every
    /// flattened point, in order.
    fn outline_signature(object: &Object) -> Option<(bool, Vec<(f64, f64)>)> {
        let path = object.world_path();
        let points: Vec<(f64, f64)> = path
            .flatten(0.1)
            .into_iter()
            .map(|point| (point.x, point.y))
            .collect();
        if points.len() < 2 {
            return None;
        }
        Some((path.closed, points))
    }

    fn same_signature(
        first: &(bool, Vec<(f64, f64)>),
        second: &(bool, Vec<(f64, f64)>),
        tolerance: f64,
    ) -> bool {
        first.0 == second.0
            && first.1.len() == second.1.len()
            && first
                .1
                .iter()
                .zip(&second.1)
                .all(|(a, b)| (a.0 - b.0).abs() <= tolerance && (a.1 - b.1).abs() <= tolerance)
    }

    /// Groups of objects whose outlines are identical.
    ///
    /// Outlines are compared by their flattened world points in order, so two
    /// copies of the same shape are detected while a shape drawn with a
    /// different start point or direction is not. Objects without vector
    /// geometry are ignored.
    pub fn duplicate_groups(&self) -> Vec<Vec<String>> {
        type Signature = (bool, Vec<(f64, f64)>);

        let signatures: Vec<Option<Signature>> = self
            .document
            .objects
            .iter()
            .map(Self::outline_signature)
            .collect();
        let mut assigned = vec![false; signatures.len()];
        let mut groups = Vec::new();
        for index in 0..signatures.len() {
            if assigned[index] {
                continue;
            }
            let Some(signature) = &signatures[index] else {
                continue;
            };
            let mut group = vec![self.document.objects[index].id.clone()];
            for (other, other_signature) in signatures.iter().enumerate().skip(index + 1) {
                if assigned[other] {
                    continue;
                }
                let Some(other_signature) = other_signature else {
                    continue;
                };
                if Self::same_signature(signature, other_signature, 1e-6) {
                    assigned[other] = true;
                    group.push(self.document.objects[other].id.clone());
                }
            }
            if group.len() > 1 {
                assigned[index] = true;
                groups.push(group);
            }
        }
        groups
    }

    /// Remove duplicate outlines, keeping the first object of each group.
    ///
    /// Locked objects are never removed. Returns how many objects were removed.
    pub fn remove_duplicates(&mut self) -> usize {
        let mut remove = Vec::new();
        for group in self.duplicate_groups() {
            let mut kept = false;
            for id in group {
                let Some(object) = self.document.object(&id) else {
                    continue;
                };
                if object.locked {
                    continue;
                }
                if kept {
                    remove.push(id);
                } else {
                    kept = true;
                }
            }
        }
        if remove.is_empty() {
            return 0;
        }
        self.record_action("Remove duplicates");
        let count = remove.len();
        self.selection.clear();
        for id in remove {
            self.selection.remove(&id);
            self.document.remove_object(&id);
        }
        count
    }

    /// Every intersection between the outlines of the relevant objects.
    ///
    /// The selected objects are examined when at least two are selected;
    /// otherwise the whole document is. Intersections closer than 0.001 mm
    /// between the same pair of objects are reported once.
    pub fn intersections(&self) -> Vec<PathIntersection> {
        use crate::geometry::intersection::segment_crossing_point;

        let indices = if self.selection.len() >= 2 {
            self.selected_indices()
        } else {
            (0..self.document.objects.len()).collect()
        };
        let segments: Vec<(usize, Vec<crate::geometry::Segment>)> = indices
            .iter()
            .map(|index| {
                (
                    *index,
                    Self::outline_segments(&self.document.objects[*index]),
                )
            })
            .filter(|(_, segments)| !segments.is_empty())
            .collect();
        let mut found = Vec::new();
        for (position, (first_index, first)) in segments.iter().enumerate() {
            for (second_index, second) in segments.iter().skip(position + 1) {
                let mut points: Vec<Point2> = Vec::new();
                for a in first {
                    for b in second {
                        let Some((x, y)) = segment_crossing_point(a, b) else {
                            continue;
                        };
                        let point = Point2::new(x, y);
                        if points
                            .iter()
                            .all(|existing| existing.distance(point) > 1e-3)
                        {
                            points.push(point);
                        }
                    }
                }
                for point in points {
                    found.push(PathIntersection {
                        first: self.document.objects[*first_index].id.clone(),
                        second: self.document.objects[*second_index].id.clone(),
                        point,
                    });
                }
            }
        }
        found
    }

    /// The contours that reproduce an object's stroke, or an error naming why
    /// it cannot be outlined.
    fn stroke_contours(object: &Object) -> std::result::Result<Vec<OutlineContour>, String> {
        use crate::geometry::boolean::polygon_area;
        use crate::geometry::offset::{offset_collapsed, offset_polygon_miter, offset_polyline};

        let width = object.style.stroke_width_mm;
        if !width.is_finite() || width <= 0.0 {
            return Err(format!("'{}' has no stroke width", object.name));
        }
        let path = object.world_path();
        if path.is_empty() {
            return Err(format!("'{}' has no vector outline", object.name));
        }
        let half = width / 2.0;
        let mut contours = Vec::new();
        if path.closed {
            let mut contour: Vec<(f64, f64)> = path
                .flatten(0.2)
                .into_iter()
                .map(|point| (point.x, point.y))
                .collect();
            if contour.len() > 1 {
                let first = contour[0];
                let last = contour[contour.len() - 1];
                if (first.0 - last.0).abs() <= 1e-9 && (first.1 - last.1).abs() <= 1e-9 {
                    contour.pop();
                }
            }
            if polygon_area(&contour) < 0.0 {
                contour.reverse();
            }
            let outer = offset_polygon_miter(&contour, half);
            if outer.len() >= 3 {
                contours.push((outer, true));
            }
            let inner = offset_polygon_miter(&contour, -half);
            if inner.len() >= 3
                && !offset_collapsed(&contour, &inner, -half)
                && polygon_area(&inner) > 1e-9
            {
                contours.push((inner, true));
            }
        } else {
            let points: Vec<(f64, f64)> = path
                .flatten(0.2)
                .into_iter()
                .map(|point| (point.x, point.y))
                .collect();
            if points.len() >= 2 {
                contours.push((offset_polyline(&points, half), false));
                contours.push((offset_polyline(&points, -half), false));
            }
        }
        if contours.is_empty() {
            return Err(format!("'{}' is too thin to outline", object.name));
        }
        Ok(contours)
    }

    /// Cut objects where they cross the last selected closed shape and drop the
    /// parts inside it.
    ///
    /// The last selected object is the trimmer and is left unchanged. Every other
    /// selected object is split at the crossings; kept pieces become separate
    /// paths, and an object that lies entirely inside is removed. Returns how
    /// many objects were changed.
    pub fn trim_selection(&mut self) -> std::result::Result<usize, String> {
        use crate::geometry::boolean::{point_in_polygon, split_polyline};

        let indices = self.selected_indices();
        if indices.len() < 2 {
            return Err("Select the shapes to trim and a closed trimmer".to_string());
        }
        let trimmer_index = indices[indices.len() - 1];
        let trimmer = Self::closed_world_contour(&self.document.objects[trimmer_index])
            .ok_or_else(|| {
                format!(
                    "'{}' is not a closed trimmer",
                    self.document.objects[trimmer_index].name
                )
            })?;
        let mut plans: Vec<Trimmed> = Vec::new();
        let mut changed = 0;
        for index in indices[..indices.len() - 1].iter().copied() {
            self.ensure_editable(index)?;
            let object = &self.document.objects[index];
            let path = object.world_path();
            if path.is_empty() {
                continue;
            }
            let points: Vec<(f64, f64)> = path
                .flatten(0.2)
                .into_iter()
                .map(|point| (point.x, point.y))
                .collect();
            let pieces = split_polyline(&points, path.closed, &trimmer);
            let uncut = pieces.len() == 1;
            let mut kept = Vec::new();
            let mut removed = false;
            for piece in pieces {
                if piece.len() < 2 {
                    continue;
                }
                let middle = (piece.len() - 1) / 2;
                let mid = (
                    f64::midpoint(piece[middle].0, piece[middle + 1].0),
                    f64::midpoint(piece[middle].1, piece[middle + 1].1),
                );
                if point_in_polygon(mid.0, mid.1, &trimmer) {
                    removed = true;
                } else {
                    kept.push(piece);
                }
            }
            if removed {
                changed += 1;
            }
            plans.push(Trimmed {
                index,
                pieces: kept,
                closed: path.closed && uncut,
            });
        }
        if changed == 0 {
            return Err("Nothing crosses the trimmer".to_string());
        }

        self.record_action("Trim");
        self.selection.clear();
        let mut objects = Vec::with_capacity(self.document.objects.len());
        let mut plan = plans.into_iter().peekable();
        for (index, object) in self.document.objects.iter().enumerate() {
            let Some(entry) = plan.next_if(|entry| entry.index == index) else {
                objects.push(object.clone());
                continue;
            };
            for (position, points) in entry.pieces.iter().enumerate() {
                let mut piece = object.clone();
                piece.id = uuid::Uuid::new_v4().to_string();
                piece.shape = Shape::Path(Path::from_points(points, entry.closed));
                piece.transform = Transform2::identity();
                piece.name = format!("{} part {}", object.name, position + 1);
                self.selection.add(piece.id.clone());
                objects.push(piece);
            }
        }
        self.document.objects = objects;
        Ok(changed)
    }

    /// Remove segments that two selected outlines share exactly.
    ///
    /// Only segments with the same two endpoints (in either direction) are
    /// compared; partially collinear overlaps are left alone and reported as
    /// unsupported. Remaining runs become open paths. Returns how many objects
    /// were changed.
    pub fn delete_overlapping_segments(&mut self) -> std::result::Result<usize, String> {
        fn same(a: (f64, f64), b: (f64, f64)) -> bool {
            (a.0 - b.0).abs() <= 1e-6 && (a.1 - b.1).abs() <= 1e-6
        }

        let indices = self.selected_indices();
        if indices.len() < 2 {
            return Err("Select at least two paths".to_string());
        }
        let mut segments: Vec<Vec<LineSegment>> = Vec::new();
        for index in &indices {
            self.ensure_editable(*index)?;
            let points: Vec<(f64, f64)> = self.document.objects[*index]
                .world_path()
                .flatten(0.2)
                .into_iter()
                .map(|point| (point.x, point.y))
                .collect();
            segments.push(points.windows(2).map(|pair| (pair[0], pair[1])).collect());
        }
        let mut remove: Vec<Vec<bool>> = segments
            .iter()
            .map(|list| vec![false; list.len()])
            .collect();
        let mut found = false;
        for first in 0..segments.len() {
            for second in first + 1..segments.len() {
                for (first_index, a) in segments[first].iter().enumerate() {
                    for (second_index, b) in segments[second].iter().enumerate() {
                        let equal = (same(a.0, b.0) && same(a.1, b.1))
                            || (same(a.0, b.1) && same(a.1, b.0));
                        if equal {
                            remove[first][first_index] = true;
                            remove[second][second_index] = true;
                            found = true;
                        }
                    }
                }
            }
        }
        if !found {
            return Err("No overlapping segments found".to_string());
        }

        self.record_action("Delete overlapping segments");
        self.selection.clear();
        let mut objects = Vec::with_capacity(self.document.objects.len());
        let mut changed = 0;
        for (index, object) in self.document.objects.iter().enumerate() {
            let Some(position) = indices.iter().position(|selected| *selected == index) else {
                objects.push(object.clone());
                continue;
            };
            let list = &segments[position];
            let flags = &remove[position];
            let mut runs: Vec<Vec<(f64, f64)>> = Vec::new();
            let mut current: Vec<(f64, f64)> = Vec::new();
            for (segment, gone) in list.iter().zip(flags) {
                if *gone {
                    if current.len() >= 2 {
                        runs.push(std::mem::take(&mut current));
                    } else {
                        current.clear();
                    }
                } else {
                    if current.is_empty() {
                        current.push(segment.0);
                    }
                    current.push(segment.1);
                }
            }
            if current.len() >= 2 {
                runs.push(current);
            }
            changed += 1;
            for (run_index, run) in runs.iter().enumerate() {
                let mut piece = object.clone();
                piece.id = uuid::Uuid::new_v4().to_string();
                piece.shape = Shape::Path(Path::from_points(run, false));
                piece.transform = Transform2::identity();
                piece.name = format!("{} part {}", object.name, run_index + 1);
                self.selection.add(piece.id.clone());
                objects.push(piece);
            }
        }
        self.document.objects = objects;
        Ok(changed)
    }

    /// Replace each selected stroke by the outlines of its width.
    ///
    /// A closed shape becomes an outer and an inner contour, so cutting both
    /// reproduces the painted stroke; an open path becomes two offset polylines.
    /// The new paths keep the original style and remove the source object.
    /// Returns the number of contours created.
    pub fn outline_selection(&mut self) -> std::result::Result<usize, String> {
        let targets = self.selected_indices();
        if targets.is_empty() {
            return Err("Select at least one object".to_string());
        }
        let mut outlined: Vec<Outlined> = Vec::new();
        for index in targets {
            self.ensure_editable(index)?;
            let contours = Self::stroke_contours(&self.document.objects[index])?;
            outlined.push(Outlined { index, contours });
        }
        let created: usize = outlined.iter().map(|entry| entry.contours.len()).sum();
        self.record_action("Outline stroke");
        self.selection.clear();
        let mut templates: Vec<(usize, Object, Vec<OutlineContour>)> = outlined
            .into_iter()
            .map(|entry| {
                (
                    entry.index,
                    self.document.objects[entry.index].clone(),
                    entry.contours,
                )
            })
            .collect();
        let mut removed: Vec<usize> = templates.iter().map(|(index, _, _)| *index).collect();
        removed.sort_unstable();
        removed.reverse();
        for index in removed {
            self.document.objects.remove(index);
        }
        for (index, template, contours) in templates.drain(..) {
            let insert_at = index.min(self.document.objects.len());
            for (position, (points, closed)) in contours.into_iter().enumerate() {
                let mut object = Object::new(
                    template.layer_id.clone(),
                    Shape::Path(Path::from_points(&points, closed)),
                )
                .with_style(template.style.clone());
                object.name = format!("{} outline {}", template.name, position + 1);
                object.operation_id.clone_from(&template.operation_id);
                self.selection.add(object.id.clone());
                self.document.objects.insert(insert_at + position, object);
            }
        }
        Ok(created)
    }

    /// Join the selected open paths into one path, welding the nearest ends.
    ///
    /// Paths are appended in nearest-neighbour order and reversed when that
    /// makes the connection shorter. A straight segment closes the gap between
    /// two ends; the result keeps the first path's style, layer and operation.
    pub fn join_paths(&mut self) -> std::result::Result<String, String> {
        let indices = self.selected_indices();
        let mut paths: Vec<Path> = Vec::new();
        for index in &indices {
            self.ensure_editable(*index)?;
            let object = &self.document.objects[*index];
            match &object.shape {
                Shape::Path(path) if !path.closed => paths.push(object.world_path()),
                Shape::Path(_) => return Err(format!("'{}' is already closed", object.name)),
                _ => return Err(format!("'{}' is not a path", object.name)),
            }
        }
        if paths.len() < 2 {
            return Err("Select at least two open paths".to_string());
        }
        let first_index = indices[0];
        let mut joined = paths.remove(0);
        while !paths.is_empty() {
            let Some(end) = joined.last_point() else {
                break;
            };
            let mut best = 0;
            let mut best_reversed = false;
            let mut best_distance = f64::INFINITY;
            for (candidate, path) in paths.iter().enumerate() {
                if let Some(start) = path.start_point() {
                    let distance = end.distance(start);
                    if distance < best_distance {
                        best_distance = distance;
                        best = candidate;
                        best_reversed = false;
                    }
                }
                if let Some(finish) = path.last_point() {
                    let distance = end.distance(finish);
                    if distance < best_distance {
                        best_distance = distance;
                        best = candidate;
                        best_reversed = true;
                    }
                }
            }
            let mut path = paths.remove(best);
            if best_reversed {
                path.reverse();
            }
            if let Some(start) = path.start_point() {
                if end.distance(start) > 1e-9 {
                    joined.push(Segment::Line {
                        from: end,
                        to: start,
                    });
                }
            }
            joined.append(&path);
        }

        self.record_action("Join paths");
        let template = self.document.objects[first_index].clone();
        let mut insert_at = indices[0];
        for index in indices.iter().rev() {
            self.document.objects.remove(*index);
            insert_at = *index;
        }
        let mut object = Object::new(template.layer_id.clone(), Shape::Path(joined))
            .with_style(template.style.clone());
        object.name = "Joined Path".to_string();
        object.operation_id.clone_from(&template.operation_id);
        let id = object.id.clone();
        self.document.objects.insert(insert_at, object);
        self.selection.clear();
        self.selection.add(id.clone());
        Ok(id)
    }

    /// Mark every selected open path as closed.
    pub fn close_paths(&mut self) -> std::result::Result<usize, String> {
        let targets = self.selected_indices();
        if targets.is_empty() {
            return Err("Select at least one open path".to_string());
        }
        let mut closable = Vec::new();
        for index in targets {
            self.ensure_editable(index)?;
            if let Shape::Path(path) = &self.document.objects[index].shape {
                if !path.closed && path.segments.len() >= 2 {
                    closable.push(index);
                }
            }
        }
        if closable.is_empty() {
            return Err("No open paths selected".to_string());
        }
        self.record_action("Close paths");
        let count = closable.len();
        for index in closable {
            if let Shape::Path(path) = &mut self.document.objects[index].shape {
                path.close();
            }
        }
        Ok(count)
    }

    /// Offset every selected closed shape by `distance` millimetres.
    ///
    /// Positive values expand the outline and negative values contract it. A
    /// shape that collapses is removed. Returns the number of shapes that kept a
    /// valid outline; the document is left untouched when every shape would
    /// collapse.
    pub fn offset_selection(&mut self, distance: f64) -> std::result::Result<usize, String> {
        type OffsetUpdate = (usize, Vec<(f64, f64)>, bool);

        use crate::geometry::boolean::polygon_area;
        use crate::geometry::offset::{offset_collapsed, offset_polygon_miter};

        if !distance.is_finite() || distance == 0.0 {
            return Err("Offset distance must be a non-zero number".to_string());
        }
        let targets = self.selected_indices();
        if targets.is_empty() {
            return Err("Select at least one closed shape".to_string());
        }
        let mut updates: Vec<OffsetUpdate> = Vec::new();
        let mut valid = 0;
        for index in targets {
            self.ensure_editable(index)?;
            let mut contour = Self::closed_world_contour(&self.document.objects[index])
                .ok_or_else(|| {
                    format!(
                        "'{}' is not a closed shape",
                        self.document.objects[index].name
                    )
                })?;
            if polygon_area(&contour) < 0.0 {
                contour.reverse();
            }
            let result = offset_polygon_miter(&contour, distance);
            let collapsed = offset_collapsed(&contour, &result, distance);
            if !collapsed {
                valid += 1;
            }
            updates.push((index, result, collapsed));
        }
        if valid == 0 {
            return Err("The offset is too large; no shape would remain".to_string());
        }

        self.record_action("Offset");
        let mut removed = Vec::new();
        for (index, points, collapsed) in &updates {
            if *collapsed {
                removed.push(*index);
                continue;
            }
            let object = &mut self.document.objects[*index];
            object.shape = Shape::Path(Path::from_points(points, true));
            object.transform = Transform2::identity();
        }
        removed.sort_unstable_by(|left, right| right.cmp(left));
        for index in removed {
            let object = self.document.objects.remove(index);
            self.selection.remove(&object.id);
        }
        Ok(valid)
    }

    pub fn object_at(&self, point: Point2, tolerance_mm: f64) -> Option<String> {
        for object in self.document.objects.iter().rev() {
            if !object.visible || object.locked {
                continue;
            }
            if let Some(layer) = self.document.layer(&object.layer_id) {
                if !layer.visible || layer.locked {
                    continue;
                }
            }
            if Self::object_hit(object, point, tolerance_mm) {
                return Some(object.id.clone());
            }
        }
        None
    }

    fn object_hit(object: &Object, point: Point2, tolerance_mm: f64) -> bool {
        let Some((min, max)) = object.bounds() else {
            return false;
        };
        if point.x < min.x - tolerance_mm
            || point.x > max.x + tolerance_mm
            || point.y < min.y - tolerance_mm
            || point.y > max.y + tolerance_mm
        {
            return false;
        }
        match &object.shape {
            Shape::Text(_) | Shape::Image(_) => true,
            _ => {
                let path = object.world_path();
                let points = path.flatten(0.25);
                if points.len() < 2 {
                    return false;
                }
                let mut closest = f64::INFINITY;
                for pair in points.windows(2) {
                    closest = closest.min(distance_point_to_segment(
                        point.x, point.y, pair[0].x, pair[0].y, pair[1].x, pair[1].y,
                    ));
                }
                if closest <= tolerance_mm {
                    return true;
                }
                // Filled shapes are selectable anywhere inside their outline.
                object.style.fill.is_some() && point_in_polygon(point, &points)
            }
        }
    }

    pub fn select_at(
        &mut self,
        point: Point2,
        additive: bool,
        tolerance_mm: f64,
    ) -> Option<String> {
        let hit = self.object_at(point, tolerance_mm);
        let members = |editor: &Self, id: &str| -> Vec<String> {
            if editor.select_whole_group {
                editor
                    .group_members(id)
                    .unwrap_or_else(|| vec![id.to_string()])
            } else {
                vec![id.to_string()]
            }
        };
        match hit.clone() {
            Some(id) if additive => {
                for member in members(self, &id) {
                    self.selection.toggle(member);
                }
                Some(id)
            }
            Some(id) => {
                self.selection.clear();
                for member in members(self, &id) {
                    self.selection.add(member);
                }
                Some(id)
            }
            None => {
                if !additive {
                    self.selection.clear();
                }
                None
            }
        }
    }

    /// All objects sharing the object's group, or `None` when it is not grouped.
    pub fn group_members(&self, object_id: &str) -> Option<Vec<String>> {
        let group = self.document.object(object_id)?.group_id.clone()?;
        let members: Vec<String> = self
            .document
            .objects
            .iter()
            .filter(|object| object.group_id.as_deref() == Some(group.as_str()))
            .map(|object| object.id.clone())
            .collect();
        (members.len() > 1).then_some(members)
    }

    /// Group the selection under a new identifier.
    pub fn group_selection(&mut self) -> bool {
        if self.selection.len() < 2 {
            return false;
        }
        self.record_action("Group");
        let group = uuid::Uuid::new_v4().to_string();
        let ids = self.selection.ids().to_vec();
        for id in ids {
            if let Some(object) = self.document.object_mut(&id) {
                object.group_id = Some(group.clone());
            }
        }
        true
    }

    /// Remove the group identifier from every selected object.
    pub fn ungroup_selection(&mut self) -> bool {
        if self.selection.is_empty() {
            return false;
        }
        let grouped = self.selection.ids().iter().any(|id| {
            self.document
                .object(id)
                .is_some_and(|object| object.group_id.is_some())
        });
        if !grouped {
            return false;
        }
        self.record_action("Ungroup");
        let ids = self.selection.ids().to_vec();
        for id in ids {
            if let Some(object) = self.document.object_mut(&id) {
                object.group_id = None;
            }
        }
        true
    }

    pub fn select_all(&mut self) {
        self.selection.clear();
        for object in &self.document.objects {
            if object.visible
                && !object.locked
                && self
                    .document
                    .layer(&object.layer_id)
                    .is_none_or(|layer| layer.visible && !layer.locked)
            {
                self.selection.add(object.id.clone());
            }
        }
    }

    /// Select every selectable object whose bounds intersect `rect`.
    pub fn select_in_rect(&mut self, rect: (Point2, Point2), additive: bool) {
        if !additive {
            self.selection.clear();
        }
        for object in &self.document.objects {
            if !object.visible || object.locked {
                continue;
            }
            if self
                .document
                .layer(&object.layer_id)
                .is_some_and(|layer| !layer.visible || layer.locked)
            {
                continue;
            }
            let Some((min, max)) = object.bounds() else {
                continue;
            };
            let intersects =
                min.x <= rect.1.x && max.x >= rect.0.x && min.y <= rect.1.y && max.y >= rect.0.y;
            if intersects {
                self.selection.add(object.id.clone());
            }
        }
    }

    pub fn move_selection(&mut self, dx: f64, dy: f64) {
        if dx == 0.0 && dy == 0.0 {
            return;
        }
        for id in self.selection.ids().to_vec() {
            let Some(layer_id) = self
                .document
                .object(&id)
                .map(|object| object.layer_id.clone())
            else {
                continue;
            };
            let layer_locked = self
                .document
                .layer(&layer_id)
                .is_some_and(|layer| layer.locked);
            if layer_locked {
                continue;
            }
            if let Some(object) = self.document.object_mut(&id) {
                if object.locked || !object.visible {
                    continue;
                }
                object.translate(dx, dy);
            }
        }
    }

    /// Apply a transform around the selection centre.
    fn transform_selection(&mut self, transform: Transform2, action: &str) -> bool {
        if self.selection.is_empty() {
            return false;
        }
        let Some((min, max)) = self.selected_bounds() else {
            return false;
        };
        let mode = self.transform_center;
        let page_center = Point2::new(self.document.width_mm / 2.0, self.document.height_mm / 2.0);
        let centre_for = move |min: Point2, max: Point2| match mode {
            TransformCenter::Page => page_center,
            TransformCenter::Selection | TransformCenter::Object => {
                Point2::new(f64::midpoint(min.x, max.x), f64::midpoint(min.y, max.y))
            }
        };
        let around = |center: Point2| {
            let moved = Transform2::translate(-center.x, -center.y);
            let back = Transform2::translate(center.x, center.y);
            back.compose(&transform.compose(&moved))
        };
        let combined = around(centre_for(min, max));
        self.record_action(action);
        let ids = self.selection.ids().to_vec();
        let mut changed = false;
        for id in ids {
            let layer_locked = self
                .document
                .object(&id)
                .and_then(|object| self.document.layer(&object.layer_id))
                .is_some_and(|layer| layer.locked);
            if layer_locked {
                continue;
            }
            let per_object = self
                .document
                .object(&id)
                .and_then(Object::bounds)
                .map(|(min, max)| around(centre_for(min, max)));
            if let Some(object) = self.document.object_mut(&id) {
                if object.locked {
                    continue;
                }
                let applied = match mode {
                    TransformCenter::Object => per_object.unwrap_or(combined),
                    _ => combined,
                };
                object.transform = applied.compose(&object.transform);
                changed = true;
            }
        }
        changed
    }

    /// Rotate the selection by `radians` around its centre.
    pub fn rotate_selection(&mut self, radians: f64) -> bool {
        self.transform_selection(Transform2::rotate(radians), "Rotate")
    }

    /// Scale the selection around its centre.
    pub fn scale_selection(&mut self, sx: f64, sy: f64) -> bool {
        if sx <= 0.0 || sy <= 0.0 {
            return false;
        }
        self.transform_selection(Transform2::scale(sx, sy), "Scale")
    }

    /// Mirror the selection around its centre.
    /// Scale the selection to an exact size around its centre.
    ///
    /// With `preserve_proportions` the smaller of the two factors is used on both
    /// axes, so the shape is not distorted.
    pub fn set_selection_size(
        &mut self,
        width: f64,
        height: f64,
        preserve_proportions: bool,
    ) -> bool {
        if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
            return false;
        }
        let Some((min, max)) = self.selected_bounds() else {
            return false;
        };
        let current_width = max.x - min.x;
        let current_height = max.y - min.y;
        if current_width <= 1e-9 || current_height <= 1e-9 {
            return false;
        }
        let mut sx = width / current_width;
        let mut sy = height / current_height;
        if preserve_proportions {
            let factor = sx.min(sy);
            sx = factor;
            sy = factor;
        }
        self.transform_selection(Transform2::scale(sx, sy), "Resize")
    }

    /// Shear the selection around its centre, in degrees per axis.
    pub fn skew_selection(&mut self, x_degrees: f64, y_degrees: f64) -> bool {
        if !x_degrees.is_finite() || !y_degrees.is_finite() {
            return false;
        }
        if x_degrees == 0.0 && y_degrees == 0.0 {
            return false;
        }
        let shear_x = x_degrees.to_radians().tan();
        let shear_y = y_degrees.to_radians().tan();
        let skew = Transform2 {
            a: 1.0,
            b: shear_y,
            c: shear_x,
            d: 1.0,
            tx: 0.0,
            ty: 0.0,
        };
        self.transform_selection(skew, "Skew")
    }

    /// Bake each selected object's transform into its geometry.
    ///
    /// Text and images keep their transform because their content is not vector
    /// geometry. Returns how many objects were reset.
    pub fn reset_transforms(&mut self) -> usize {
        if self.selection.is_empty() {
            return 0;
        }
        self.record_action("Reset transforms");
        let ids = self.selection.ids().to_vec();
        let mut reset = 0;
        for id in ids {
            let Some(object) = self.document.object_mut(&id) else {
                continue;
            };
            if object.locked || !object.visible {
                continue;
            }
            if matches!(object.shape, Shape::Text(_) | Shape::Image(_)) {
                continue;
            }
            let world = object.world_path();
            if world.is_empty() {
                continue;
            }
            object.shape = Shape::Path(world);
            object.transform = Transform2::identity();
            reset += 1;
        }
        reset
    }

    pub fn flip_selection(&mut self, horizontal: bool) -> bool {
        let transform = if horizontal {
            Transform2::scale(-1.0, 1.0)
        } else {
            Transform2::scale(1.0, -1.0)
        };
        self.transform_selection(
            transform,
            if horizontal {
                "Flip horizontally"
            } else {
                "Flip vertically"
            },
        )
    }

    /// Align the selection. A single object aligns to the work area; several
    /// objects align to the selection bounds.
    pub fn align_selection(&mut self, mode: super::alignment::AlignMode) -> bool {
        use super::alignment::AlignMode;
        let ids = self.selection.ids().to_vec();
        if ids.is_empty() {
            return false;
        }
        let bounds: Vec<(String, Point2, Point2)> = ids
            .iter()
            .filter_map(|id| {
                self.document
                    .object(id)
                    .and_then(|object| object.bounds().map(|(min, max)| (id.clone(), min, max)))
            })
            .collect();
        if bounds.is_empty() {
            return false;
        }
        let (target_min, target_max) = if bounds.len() == 1 {
            (
                Point2::origin(),
                Point2::new(self.document.width_mm, self.document.height_mm),
            )
        } else {
            self.selected_bounds()
                .unwrap_or((Point2::origin(), Point2::origin()))
        };
        let delta = |min: Point2, max: Point2| -> (f64, f64) {
            match mode {
                AlignMode::Left => (target_min.x - min.x, 0.0),
                AlignMode::Right => (target_max.x - max.x, 0.0),
                AlignMode::Top => (0.0, target_max.y - max.y),
                AlignMode::Bottom => (0.0, target_min.y - min.y),
                AlignMode::CenterH => (
                    f64::midpoint(target_min.x, target_max.x) - f64::midpoint(min.x, max.x),
                    0.0,
                ),
                AlignMode::CenterV => (
                    0.0,
                    f64::midpoint(target_min.y, target_max.y) - f64::midpoint(min.y, max.y),
                ),
            }
        };
        self.record_action("Align");
        for (id, min, max) in bounds {
            let (dx, dy) = delta(min, max);
            if dx.abs() < 1e-9 && dy.abs() < 1e-9 {
                continue;
            }
            if let Some(object) = self.document.object_mut(&id) {
                object.translate(dx, dy);
            }
        }
        true
    }

    /// Distribute three or more objects evenly by centre.
    #[allow(clippy::cast_precision_loss)]
    pub fn distribute_selection(&mut self, horizontal: bool) -> bool {
        let ids = self.selection.ids().to_vec();
        if ids.len() < 3 {
            return false;
        }
        let mut items: Vec<(String, f64)> = ids
            .iter()
            .filter_map(|id| {
                self.document.object(id).and_then(|object| {
                    object.bounds().map(|(min, max)| {
                        let center = if horizontal {
                            f64::midpoint(min.x, max.x)
                        } else {
                            f64::midpoint(min.y, max.y)
                        };
                        (id.clone(), center)
                    })
                })
            })
            .collect();
        if items.len() < 3 {
            return false;
        }
        items.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        let first = items[0].1;
        let last = items[items.len() - 1].1;
        let step = (last - first) / (items.len() - 1) as f64;
        self.record_action("Distribute");
        for (index, (id, center)) in items.iter().enumerate() {
            let target = first + step * index as f64;
            let delta = target - center;
            if delta.abs() < 1e-9 {
                continue;
            }
            if let Some(object) = self.document.object_mut(id) {
                if horizontal {
                    object.translate(delta, 0.0);
                } else {
                    object.translate(0.0, delta);
                }
            }
        }
        true
    }

    pub fn delete_selection(&mut self) {
        if self.selection.is_empty() {
            return;
        }
        self.record_action("Delete");
        let ids = self.selection.ids().to_vec();
        for id in &ids {
            self.document.remove_object(id);
        }
        self.selection.clear();
    }

    pub fn duplicate_selection(&mut self) {
        if self.selection.is_empty() {
            return;
        }
        self.record_action("Duplicate");
        let ids = self.selection.ids().to_vec();
        let mut new_ids = Vec::new();
        for id in ids {
            if let Some(object) = self.document.object(&id) {
                let mut copy = object.clone();
                copy.id = uuid::Uuid::new_v4().to_string();
                copy.name = format!("{} copy", object.name);
                copy.translate(5.0, -5.0);
                new_ids.push(copy.id.clone());
                self.document.add_object(copy);
            }
        }
        self.selection.clear();
        for id in new_ids {
            self.selection.add(id);
        }
    }

    /// Clone the selected objects for an in-app clipboard.
    pub fn copy_selection(&self) -> Vec<Object> {
        self.selection
            .ids()
            .iter()
            .filter_map(|id| self.document.object(id).cloned())
            .collect()
    }

    /// Remove the selected objects and return them for an in-app clipboard.
    pub fn cut_selection(&mut self) -> Vec<Object> {
        let objects = self.copy_selection();
        if objects.is_empty() {
            return objects;
        }
        self.record_action("Cut");
        for object in &objects {
            self.document.remove_object(&object.id);
        }
        self.selection.clear();
        objects
    }

    /// Paste copied objects with fresh identifiers, offset slightly.
    ///
    /// The pasted objects become the selection. Groups are not recreated, so a
    /// pasted copy cannot silently link back to the original group.
    pub fn paste_objects(&mut self, objects: &[Object]) -> usize {
        if objects.is_empty() {
            return 0;
        }
        self.record_action("Paste");
        let fallback_layer = self.document.layers.first().map(|layer| layer.id.clone());
        let mut pasted = Vec::new();
        for source in objects {
            let mut object = source.clone();
            object.id = uuid::Uuid::new_v4().to_string();
            object.group_id = None;
            object.name = format!("{} copy", source.name);
            if self.document.layer(&object.layer_id).is_none() {
                let Some(layer) = fallback_layer.clone() else {
                    continue;
                };
                object.layer_id = layer;
            }
            object.translate(5.0, -5.0);
            pasted.push(object.id.clone());
            self.document.add_object(object);
        }
        self.selection.clear();
        for id in &pasted {
            self.selection.add(id.clone());
        }
        pasted.len()
    }

    /// Select every unselected visible, unlocked object.
    pub fn invert_selection(&mut self) -> usize {
        let candidates: Vec<String> = self
            .document
            .objects
            .iter()
            .filter(|object| {
                !self.selection.contains(&object.id)
                    && object.visible
                    && !object.locked
                    && self.layer_visible(&object.layer_id)
                    && !self.layer_locked(&object.layer_id)
            })
            .map(|object| object.id.clone())
            .collect();
        self.selection.clear();
        for id in &candidates {
            self.selection.add(id.clone());
        }
        candidates.len()
    }

    /// Lock the selected objects so they cannot be moved or picked.
    pub fn lock_selection(&mut self) -> usize {
        self.set_selection_locked(true)
    }

    /// Unlock the selected objects.
    pub fn unlock_selection(&mut self) -> usize {
        self.set_selection_locked(false)
    }

    fn set_selection_locked(&mut self, locked: bool) -> usize {
        if self.selection.is_empty() {
            return 0;
        }
        self.record_action(if locked { "Lock" } else { "Unlock" });
        let ids = self.selection.ids().to_vec();
        let mut changed = 0;
        for id in ids {
            if let Some(object) = self.document.object_mut(&id) {
                if object.locked != locked {
                    object.locked = locked;
                    changed += 1;
                }
            }
        }
        changed
    }

    /// Duplicate the selection along a line or around a circle.
    ///
    /// Linear copies step by `(dx, dy)` and may be rotated by a fixed angle each
    /// (repetition by distance is the same operation with a normalised step).
    /// Circular copies sit on a circle of `radius` around the selection centre,
    /// evenly spaced with the original as one position, and can follow the
    /// orbit with their own rotation. Copies never join the original's group.
    pub fn repeat_selection(
        &mut self,
        options: &RepeatOptions,
    ) -> std::result::Result<usize, String> {
        if self.selection.is_empty() {
            return Err("Select at least one object".to_string());
        }
        if !(1..=100).contains(&options.copies) {
            return Err("Copies must be between 1 and 100".to_string());
        }
        for value in [
            options.dx,
            options.dy,
            options.radius,
            options.rotation_degrees,
        ] {
            if !value.is_finite() {
                return Err("Repeat parameters must be finite".to_string());
            }
        }
        if options.mode == RepeatMode::Circular && options.radius < 0.0 {
            return Err("Radius must be zero or positive".to_string());
        }
        if self.selected_bounds().is_none() {
            return Err("The selection has no bounds".to_string());
        }
        let sources: Vec<crate::document::Object> = self
            .selection
            .ids()
            .iter()
            .filter_map(|id| self.document.object(id).cloned())
            .collect();
        if sources.is_empty() {
            return Err("The selection is empty".to_string());
        }
        self.record_action("Repeat");
        let mut created = Vec::new();
        for copy in 1..=options.copies {
            #[allow(clippy::cast_precision_loss)]
            let fraction = copy as f64;
            let (offset, angle) = match options.mode {
                RepeatMode::Linear => (
                    Point2::new(options.dx * fraction, options.dy * fraction),
                    options.rotation_degrees.to_radians() * fraction,
                ),
                RepeatMode::Circular => {
                    #[allow(clippy::cast_precision_loss)]
                    let step = std::f64::consts::TAU * fraction / (options.copies as f64 + 1.0);
                    (
                        Point2::new(options.radius * step.cos(), options.radius * step.sin()),
                        if options.rotate_copies { step } else { 0.0 },
                    )
                }
            };
            for source in &sources {
                let mut object = source.clone();
                object.id = uuid::Uuid::new_v4().to_string();
                object.group_id = None;
                object.name = if options.mode == RepeatMode::Linear {
                    format!("{} copy {}", source.name, copy)
                } else {
                    format!("{} around {}", source.name, copy)
                };
                object.translate(offset.x, offset.y);
                if angle != 0.0 {
                    let Some((copy_min, copy_max)) = object.bounds() else {
                        continue;
                    };
                    let copy_center = Point2::new(
                        f64::midpoint(copy_min.x, copy_max.x),
                        f64::midpoint(copy_min.y, copy_max.y),
                    );
                    let forward = Transform2::translate(-copy_center.x, -copy_center.y);
                    let back = Transform2::translate(copy_center.x, copy_center.y);
                    let rotation = back.compose(&Transform2::rotate(angle).compose(&forward));
                    object.transform = rotation.compose(&object.transform);
                }
                created.push(object.id.clone());
                self.document.add_object(object);
            }
        }
        self.selection.clear();
        for id in &created {
            self.selection.add(id.clone());
        }
        Ok(created.len())
    }

    /// Duplicate the selection in a rectangular grid.
    ///
    /// Returns the number of new objects. The original objects stay in place;
    /// the copies become the selection.
    pub fn duplicate_grid(&mut self, columns: usize, rows: usize, dx: f64, dy: f64) -> usize {
        self.duplicate_grid_with(&GridOptions {
            columns,
            rows,
            dx,
            dy,
            ..GridOptions::default()
        })
    }

    /// Duplicate the selection in a grid with margins and a fill order.
    ///
    /// The margins offset the whole grid from the original; the fill order only
    /// changes the order the copies are added, which is the order later
    /// operations see them in.
    pub fn duplicate_grid_with(&mut self, options: &GridOptions) -> usize {
        if self.selection.is_empty()
            || !(1..=100).contains(&options.columns)
            || !(1..=100).contains(&options.rows)
            || !options.dx.is_finite()
            || !options.dy.is_finite()
            || !options.margin_x.is_finite()
            || !options.margin_y.is_finite()
        {
            return 0;
        }
        self.record_action("Grid duplicate");
        let originals: Vec<Object> = self
            .selection
            .ids()
            .iter()
            .filter_map(|id| self.document.object(id).cloned())
            .collect();
        self.selection.clear();
        let mut cells: Vec<(usize, usize)> = Vec::new();
        if options.column_major {
            for column in 0..options.columns {
                for row in 0..options.rows {
                    cells.push((column, row));
                }
            }
        } else {
            for row in 0..options.rows {
                for column in 0..options.columns {
                    cells.push((column, row));
                }
            }
        }
        let mut created = 0;
        for (column, row) in cells {
            if row == 0 && column == 0 {
                continue;
            }
            #[allow(clippy::cast_precision_loss)]
            let offset = (
                options.margin_x + column as f64 * options.dx,
                options.margin_y + row as f64 * options.dy,
            );
            for original in &originals {
                let mut copy = original.clone();
                copy.id = uuid::Uuid::new_v4().to_string();
                copy.translate(offset.0, offset.1);
                self.selection.add(copy.id.clone());
                self.document.add_object(copy);
                created += 1;
            }
        }
        created
    }

    pub fn nudge_selection(&mut self, dx: f64, dy: f64) {
        if self.selection.is_empty() {
            return;
        }
        self.record_action("Nudge");
        self.move_selection(dx, dy);
    }

    /// Undo one step and return the action name that was reverted.
    pub fn undo(&mut self) -> Option<String> {
        let action = self.history.undo(&mut self.document)?;
        self.prune_selection();
        Some(action)
    }

    /// Redo one step and return the action name that was reapplied.
    pub fn redo(&mut self) -> Option<String> {
        let action = self.history.redo(&mut self.document)?;
        self.prune_selection();
        Some(action)
    }

    /// Keep at most `steps` undo snapshots from now on.
    pub fn set_history_limit(&mut self, steps: usize) {
        self.history.set_limit(steps);
    }

    /// Undo several steps at once and return the last reverted action.
    pub fn jump_history(&mut self, steps: usize) -> Option<String> {
        let action = self.history.jump_to(&mut self.document, steps)?;
        self.prune_selection();
        Some(action)
    }

    fn prune_selection(&mut self) {
        let ids = self.selection.ids().to_vec();
        for id in ids {
            if self.document.object(&id).is_none() {
                self.selection.remove(&id);
            }
        }
    }

    pub fn selected_bounds(&self) -> Option<(Point2, Point2)> {
        let mut bounds: Option<(Point2, Point2)> = None;
        for id in self.selection.ids() {
            let Some(object) = self.document.object(id) else {
                continue;
            };
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

    /// Move the selected objects to the top of the drawing order.
    pub fn bring_selection_to_front(&mut self) -> usize {
        self.reorder_selection(true)
    }

    /// Move the selected objects to the bottom of the drawing order.
    pub fn send_selection_to_back(&mut self) -> usize {
        self.reorder_selection(false)
    }

    fn reorder_selection(&mut self, to_front: bool) -> usize {
        if self.selection.is_empty() {
            return 0;
        }
        self.record_action(if to_front {
            "Bring to front"
        } else {
            "Send to back"
        });
        let (selected, others): (Vec<Object>, Vec<Object>) = self
            .document
            .objects
            .drain(..)
            .partition(|object| self.selection.contains(&object.id));
        self.document.objects = if to_front {
            others.into_iter().chain(selected.iter().cloned()).collect()
        } else {
            selected.iter().cloned().chain(others).collect()
        };
        selected.len()
    }

    /// Spread the selected objects evenly across the work area.
    ///
    /// Object centres are placed at equal fractions of the page (with equal
    /// margins at both ends), keeping their order along the chosen axis.
    pub fn distribute_selection_on_page(&mut self, horizontal: bool) -> bool {
        let mut items: Vec<(String, f64)> = self
            .document
            .objects
            .iter()
            .filter(|object| self.selection.contains(&object.id))
            .filter_map(|object| {
                object
                    .bounds()
                    .map(|(min, max)| (object.id.clone(), (min, max)))
            })
            .map(|(id, (min, max))| {
                let centre = if horizontal {
                    f64::midpoint(min.x, max.x)
                } else {
                    f64::midpoint(min.y, max.y)
                };
                (id, centre)
            })
            .collect();
        if items.len() < 2 {
            return false;
        }
        items.sort_by(|left, right| {
            left.1
                .partial_cmp(&right.1)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let span = if horizontal {
            self.document.width_mm
        } else {
            self.document.height_mm
        };
        #[allow(clippy::cast_precision_loss)]
        let count = items.len() as f64;
        self.record_action("Distribute on page");
        for (position, (id, centre)) in items.iter().enumerate() {
            #[allow(clippy::cast_precision_loss)]
            let target = span * (position as f64 + 1.0) / (count + 1.0);
            let delta = target - centre;
            if let Some(object) = self.document.object_mut(id) {
                if horizontal {
                    object.translate(delta, 0.0);
                } else {
                    object.translate(0.0, delta);
                }
            }
        }
        true
    }

    /// Centre the selection on the work area.
    pub fn center_selection_on_page(&mut self) -> bool {
        let Some((min, max)) = self.selected_bounds() else {
            return false;
        };
        let center = Point2::new(f64::midpoint(min.x, max.x), f64::midpoint(min.y, max.y));
        let page = Point2::new(self.document.width_mm / 2.0, self.document.height_mm / 2.0);
        if center.distance(page) <= 1e-9 {
            return true;
        }
        self.record_action("Centre on page");
        self.move_selection(page.x - center.x, page.y - center.y);
        true
    }

    /// Fit the selection into the viewport.
    /// Zoom in by the configured step.
    pub fn zoom_in(&mut self) {
        let step = if self.zoom_step > 1.0 {
            self.zoom_step
        } else {
            1.25
        };
        self.canvas.zoom = (self.canvas.zoom * step).min(crate::editor::canvas::MAX_ZOOM);
    }

    /// Zoom out by the configured step.
    pub fn zoom_out(&mut self) {
        let step = if self.zoom_step > 1.0 {
            self.zoom_step
        } else {
            1.25
        };
        self.canvas.zoom = (self.canvas.zoom / step).max(crate::editor::canvas::MIN_ZOOM);
    }

    pub fn zoom_to_selection(&mut self, viewport_width: f64, viewport_height: f64) -> bool {
        let Some((min, max)) = self.selected_bounds() else {
            return false;
        };
        self.canvas
            .fit_bounds(min, max, viewport_width, viewport_height, 32.0);
        true
    }

    pub fn fit_view(&mut self, viewport_width: f64, viewport_height: f64) {
        self.canvas.fit(
            self.document.width_mm,
            self.document.height_mm,
            viewport_width,
            viewport_height,
            24.0,
        );
    }

    pub fn save(&self, path: &std::path::Path) -> Result<()> {
        serialization::save_to_file(&self.document, path)
    }

    /// Save the document, deciding whether to keep a `.bak` of the previous file.
    pub fn save_with_backup(&self, path: &std::path::Path, backup: bool) -> Result<()> {
        serialization::save_to_file_with_backup(&self.document, path, backup)
    }

    pub fn load(&mut self, path: &std::path::Path) -> Result<()> {
        let document = serialization::load_from_file(path)?;
        self.set_document(document);
        Ok(())
    }

    pub fn layer_visible(&self, layer_id: &str) -> bool {
        self.document
            .layer(layer_id)
            .is_some_and(|layer| layer.visible)
    }

    pub fn layer_locked(&self, layer_id: &str) -> bool {
        self.document
            .layer(layer_id)
            .is_some_and(|layer| layer.locked)
    }

    /// Create an operation and return its id.
    pub fn add_operation(
        &mut self,
        name: impl Into<String>,
        op_type: crate::operations::OperationType,
    ) -> String {
        self.record_action("Add operation");
        let operation = crate::operations::Operation::new(name, op_type);
        let id = operation.id.clone();
        self.document.operations.push(operation);
        id
    }

    /// Declare that an operation must run after another one, or clear it.
    ///
    /// At most one dependency is stored by this command; the model holds a list
    /// so a project can declare more.
    pub fn set_operation_dependency(&mut self, id: &str, depends_on: Option<&str>) -> bool {
        if !self
            .document
            .operations
            .iter()
            .any(|operation| operation.id == id)
        {
            return false;
        }
        if let Some(dependency) = depends_on {
            if dependency == id
                || !self
                    .document
                    .operations
                    .iter()
                    .any(|operation| operation.id == dependency)
            {
                return false;
            }
        }
        let new_dependencies: Vec<String> = depends_on.map(str::to_string).into_iter().collect();
        let Some(operation) = self
            .document
            .operations
            .iter_mut()
            .find(|operation| operation.id == id)
        else {
            return false;
        };
        if operation.depends_on == new_dependencies {
            return true;
        }
        self.record_action("Set operation dependency");
        if let Some(operation) = self
            .document
            .operations
            .iter_mut()
            .find(|operation| operation.id == id)
        {
            operation.depends_on = new_dependencies;
        }
        true
    }

    /// Duplicate an operation, returning the new id.
    ///
    /// The copy keeps the type and parameters, gets a new identifier and a name
    /// ending in "copy", and is inserted just after the source so the job order
    /// stays predictable.
    pub fn duplicate_operation(&mut self, id: &str) -> Option<String> {
        let index = self
            .document
            .operations
            .iter()
            .position(|operation| operation.id == id)?;
        self.record_action("Duplicate operation");
        let mut copy = self.document.operations[index].clone();
        copy.id = uuid::Uuid::new_v4().to_string();
        copy.name = format!("{} copy", copy.name);
        let new_id = copy.id.clone();
        self.document.operations.insert(index + 1, copy);
        Some(new_id)
    }

    /// Remove an operation and clear every reference to it.
    pub fn remove_operation(&mut self, id: &str) -> bool {
        let Some(index) = self
            .document
            .operations
            .iter()
            .position(|operation| operation.id == id)
        else {
            return false;
        };
        self.record_action("Remove operation");
        self.document.operations.remove(index);
        for object in &mut self.document.objects {
            if object.operation_id.as_deref() == Some(id) {
                object.operation_id = None;
            }
        }
        true
    }

    /// Move an operation earlier (`delta` -1) or later (`delta` +1) in the job
    /// order. Returns `false` at the ends of the list or for an unknown id.
    pub fn move_operation(&mut self, id: &str, delta: isize) -> bool {
        let Some(index) = self
            .document
            .operations
            .iter()
            .position(|operation| operation.id == id)
        else {
            return false;
        };
        let Some(target) = index.checked_add_signed(delta) else {
            return false;
        };
        if target >= self.document.operations.len() || target == index {
            return false;
        }
        self.record_action("Reorder operation");
        self.document.operations.swap(index, target);
        true
    }

    /// Assign an operation (or clear it) on every object of a layer.
    ///
    /// This is the layer-level form of the per-object association: the result is
    /// stored on each object, so it stays explicit and editable, and it can be
    /// undone as one step.
    pub fn assign_operation_to_layer(
        &mut self,
        layer_id: &str,
        operation_id: Option<&str>,
    ) -> usize {
        if self.document.layer(layer_id).is_none() {
            return 0;
        }
        if let Some(id) = operation_id {
            if !self
                .document
                .operations
                .iter()
                .any(|operation| operation.id == id)
            {
                return 0;
            }
        }
        let targets: Vec<usize> = self
            .document
            .objects
            .iter()
            .enumerate()
            .filter(|(_, object)| {
                object.layer_id == layer_id && object.operation_id.as_deref() != operation_id
            })
            .map(|(index, _)| index)
            .collect();
        if targets.is_empty() {
            return 0;
        }
        self.record_action("Assign to layer");
        for index in &targets {
            self.document.objects[*index].operation_id = operation_id.map(str::to_string);
        }
        targets.len()
    }

    /// Assign an operation (or clear the assignment) on the whole selection.
    pub fn assign_operation(&mut self, operation_id: Option<&str>) -> usize {
        if self.selection.is_empty() {
            return 0;
        }
        if let Some(id) = operation_id {
            if self
                .document
                .operations
                .iter()
                .all(|operation| operation.id != id)
            {
                return 0;
            }
        }
        self.record_action("Assign operation");
        let ids = self.selection.ids().to_vec();
        let mut changed = 0;
        for id in ids {
            if let Some(object) = self.document.object_mut(&id) {
                object.operation_id = operation_id.map(str::to_string);
                changed += 1;
            }
        }
        changed
    }

    /// Create an operation from a profile and assign it to the selection.
    /// Copy a profile's parameters into an existing operation.
    ///
    /// The operation type must match the profile; a mismatch is reported instead
    /// of silently changing the operation. Returns how many parameters changed.
    pub fn apply_profile_to_operation(
        &mut self,
        operation_id: &str,
        profile: &crate::profiles::OperationProfile,
    ) -> std::result::Result<usize, String> {
        let Some(operation) = self
            .document
            .operations
            .iter()
            .find(|operation| operation.id == operation_id)
        else {
            return Err(format!("unknown operation: {operation_id}"));
        };
        if operation.op_type != profile.op_type {
            return Err(format!(
                "profile '{}' is for {} operations, not {}",
                profile.name,
                profile.op_type.label(),
                operation.op_type.label()
            ));
        }
        let changes = operation.params.changes_to(&profile.params);
        if changes.is_empty() {
            return Ok(0);
        }
        let count = changes.len();
        self.record_action("Apply operation profile");
        if let Some(operation) = self
            .document
            .operations
            .iter_mut()
            .find(|operation| operation.id == operation_id)
        {
            operation.params = profile.params.clone();
        }
        Ok(count)
    }

    pub fn apply_operation_profile(
        &mut self,
        profile: &crate::profiles::OperationProfile,
    ) -> usize {
        if self.selection.is_empty() {
            return 0;
        }
        self.record_action("Apply operation profile");
        let operation = crate::operations::Operation {
            id: uuid::Uuid::new_v4().to_string(),
            name: profile.name.clone(),
            op_type: profile.op_type,
            params: profile.params.clone(),
            enabled: true,
            depends_on: Vec::new(),
        };
        let id = operation.id.clone();
        self.document.operations.push(operation);
        self.assign_operation(Some(&id))
    }

    /// Apply image preparation settings to every selected image object.
    pub fn apply_image_profile(&mut self, profile: &crate::profiles::ImageProfile) -> usize {
        if self.selection.is_empty() || profile.settings.validate().is_err() {
            return 0;
        }
        self.record_action("Apply image profile");
        let ids = self.selection.ids().to_vec();
        let mut changed = 0;
        for id in ids {
            if let Some(object) = self.document.object_mut(&id) {
                if let Shape::Image(image) = &mut object.shape {
                    image.settings = profile.settings;
                    changed += 1;
                }
            }
        }
        changed
    }

    /// Apply project defaults: work area, grid spacing and snap step.
    pub fn apply_project_profile(&mut self, profile: &crate::profiles::ProjectProfile) -> bool {
        if profile.validate().is_err() {
            return false;
        }
        self.record_action("Apply project profile");
        self.document.width_mm = profile.width_mm;
        self.document.height_mm = profile.height_mm;
        self.canvas.grid_spacing_mm = profile.grid_spacing_mm;
        self.snap_step_mm = profile.snap_mm;
        true
    }

    /// The operation shared by the whole selection, when there is exactly one.
    pub fn selection_operation(&self) -> Option<&str> {
        let mut ids = self.selection.ids().iter().map(|id| {
            self.document
                .object(id)
                .and_then(|object| object.operation_id.as_deref())
        });
        let first = ids.next()??;
        if ids.all(|id| id == Some(first)) {
            self.document
                .operations
                .iter()
                .find(|operation| operation.id == first)
                .map(|operation| operation.id.as_str())
        } else {
            None
        }
    }
}

fn point_in_polygon(point: Point2, polygon: &[Point2]) -> bool {
    let mut inside = false;
    let mut previous = polygon[polygon.len() - 1];
    for &current in polygon {
        let intersects = (current.y > point.y) != (previous.y > point.y)
            && point.x
                < (previous.x - current.x) * (point.y - current.y) / (previous.y - current.y)
                    + current.x;
        if intersects {
            inside = !inside;
        }
        previous = current;
    }
    inside
}

#[cfg(test)]
#[allow(clippy::assert_is_empty)]
mod tests {
    use super::*;
    use crate::document::Document;

    fn editor() -> EditorState {
        let mut document = Document::new("Test", 200.0, 100.0);
        document.layers[0].name = "Cut".to_string();
        EditorState::new(document)
    }

    #[test]
    fn draw_select_move_delete() {
        let mut editor = editor();
        let rect = editor
            .add_rectangle(Point2::new(10.0, 10.0), Point2::new(40.0, 30.0))
            .unwrap();
        assert_eq!(editor.document.objects.len(), 1);
        assert!(editor.selection.contains(&rect));

        editor.selection.clear();
        assert_eq!(
            editor.object_at(Point2::new(10.0, 20.0), 0.5),
            Some(rect.clone())
        );
        editor.select_at(Point2::new(10.0, 20.0), false, 0.5);
        assert!(editor.selection.contains(&rect));

        editor.move_selection(5.0, 3.0);
        let (min, _) = editor.document.object(&rect).unwrap().bounds().unwrap();
        assert!((min.x - 15.0).abs() < 1e-9);
        assert!((min.y - 13.0).abs() < 1e-9);

        editor.delete_selection();
        assert!(editor.document.objects.is_empty());
    }

    #[test]
    fn undo_redo_restores_drawing() {
        let mut editor = editor();
        editor.add_rectangle(Point2::new(0.0, 0.0), Point2::new(10.0, 10.0));
        assert_eq!(editor.document.objects.len(), 1);
        assert!(editor.undo().is_some());
        assert!(editor.document.objects.is_empty());
        assert!(editor.redo().is_some());
        assert_eq!(editor.document.objects.len(), 1);
    }

    #[test]
    fn line_and_ellipse_have_geometry() {
        let mut editor = editor();
        let line = editor
            .add_line(Point2::new(0.0, 0.0), Point2::new(10.0, 5.0))
            .unwrap();
        let ellipse = editor
            .add_ellipse(Point2::new(20.0, 10.0), Point2::new(40.0, 30.0))
            .unwrap();
        assert!(
            editor
                .document
                .object(&line)
                .unwrap()
                .world_path()
                .segment_count()
                >= 1
        );
        assert_eq!(
            editor
                .document
                .object(&ellipse)
                .unwrap()
                .world_path()
                .segment_count(),
            4
        );
    }

    #[test]
    fn save_open_round_trip_keeps_geometry() {
        let mut editor = editor();
        editor.add_rectangle(Point2::new(1.0, 2.0), Point2::new(11.0, 7.0));
        editor.add_line(Point2::new(0.0, 0.0), Point2::new(5.0, 5.0));
        editor.add_ellipse(Point2::new(20.0, 20.0), Point2::new(30.0, 26.0));

        let path = std::env::temp_dir().join("beamwright_editor_roundtrip.bwp");
        editor.save(&path).unwrap();
        let mut reopened = EditorState::new(Document::default());
        reopened.load(&path).unwrap();
        assert_eq!(reopened.document.objects.len(), 3);
        assert_eq!(
            reopened.document.objects[0].shape,
            editor.document.objects[0].shape
        );
        assert_eq!(
            reopened.document.objects[2].transform,
            editor.document.objects[2].transform
        );
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn locked_layer_cannot_be_selected() {
        let mut editor = editor();
        let id = editor
            .add_rectangle(Point2::new(0.0, 0.0), Point2::new(10.0, 10.0))
            .unwrap();
        let layer_id = editor.document.layers[0].id.clone();
        editor.document.layer_mut(&layer_id).unwrap().locked = true;
        assert_eq!(editor.object_at(Point2::new(5.0, 5.0), 0.5), None);
        assert_ne!(editor.object_at(Point2::new(0.0, 5.0), 0.5), Some(id));
    }

    #[test]
    fn fill_makes_interior_selectable() {
        let mut editor = editor();
        let id = editor
            .add_rectangle(Point2::new(0.0, 0.0), Point2::new(10.0, 10.0))
            .unwrap();
        // Stroke-only rectangle: the middle is not a hit.
        assert_eq!(editor.object_at(Point2::new(5.0, 5.0), 0.5), None);
        editor.document.object_mut(&id).unwrap().style.fill = Some("#ff0000".to_string());
        assert_eq!(editor.object_at(Point2::new(5.0, 5.0), 0.5), Some(id));
    }

    #[test]
    fn operation_assignment_round_trip() {
        use crate::operations::OperationType;

        let mut editor = editor();
        let id = editor
            .add_rectangle(Point2::new(0.0, 0.0), Point2::new(10.0, 10.0))
            .unwrap();
        let operation = editor.add_operation("Cut", OperationType::OutlineCutting);
        assert_eq!(editor.assign_operation(Some(&operation)), 1);
        assert_eq!(
            editor.document.object(&id).unwrap().operation_id.as_deref(),
            Some(operation.as_str())
        );
        assert_eq!(editor.selection_operation(), Some(operation.as_str()));
        assert_eq!(editor.assign_operation(None), 1);
        assert_eq!(editor.selection_operation(), None);
    }

    #[test]
    fn assigning_unknown_operation_does_nothing() {
        let mut editor = editor();
        editor.add_rectangle(Point2::new(0.0, 0.0), Point2::new(10.0, 10.0));
        assert_eq!(editor.assign_operation(Some("missing")), 0);
        assert!(editor.document.objects[0].operation_id.is_none());
    }

    #[test]
    fn removing_operation_clears_references() {
        use crate::operations::OperationType;

        let mut editor = editor();
        editor.add_rectangle(Point2::new(0.0, 0.0), Point2::new(10.0, 10.0));
        let operation = editor.add_operation("Cut", OperationType::OutlineCutting);
        editor.assign_operation(Some(&operation));
        assert!(editor.remove_operation(&operation));
        assert!(editor.document.operations.is_empty());
        assert!(editor.document.objects[0].operation_id.is_none());
        assert!(editor.document.validate().is_ok());
        assert!(!editor.remove_operation("missing"));
    }

    #[test]
    fn mixed_selection_has_no_shared_operation() {
        use crate::operations::OperationType;

        let mut editor = editor();
        editor.add_rectangle(Point2::new(0.0, 0.0), Point2::new(10.0, 10.0));
        let second = editor
            .add_rectangle(Point2::new(20.0, 0.0), Point2::new(30.0, 10.0))
            .unwrap();
        let first = editor.document.objects[0].id.clone();
        let operation = editor.add_operation("Cut", OperationType::OutlineCutting);
        editor.document.object_mut(&first).unwrap().operation_id = Some(operation.clone());
        editor.selection.clear();
        editor.selection.add(first);
        editor.selection.add(second);
        assert_eq!(editor.selection_operation(), None);
    }

    #[test]
    fn applying_profile_creates_operation_and_assigns_it() {
        use crate::profiles::OperationProfile;

        let mut editor = editor();
        let object = editor
            .add_rectangle(Point2::new(0.0, 0.0), Point2::new(10.0, 10.0))
            .unwrap();
        let mut profile =
            OperationProfile::new("Engrave", crate::operations::OperationType::LineEngraving);
        profile.params.speed = 2500.0;
        profile.params.power = 15.0;
        assert_eq!(editor.apply_operation_profile(&profile), 1);
        assert_eq!(editor.document.operations.len(), 1);
        let operation = &editor.document.operations[0];
        assert_eq!(operation.name, "Engrave");
        assert_eq!(operation.params.speed, 2500.0);
        assert_eq!(
            editor
                .document
                .object(&object)
                .unwrap()
                .operation_id
                .as_deref(),
            Some(operation.id.as_str())
        );
    }

    #[test]
    fn applying_profile_without_selection_does_nothing() {
        use crate::profiles::OperationProfile;

        let mut editor = editor();
        let profile = OperationProfile::new("Cut", crate::operations::OperationType::Line);
        assert_eq!(editor.apply_operation_profile(&profile), 0);
        assert!(editor.document.operations.is_empty());
    }

    #[test]
    fn rotate_quarter_turn_swaps_bounds() {
        let mut editor = editor();
        editor.add_rectangle(Point2::new(10.0, 10.0), Point2::new(20.0, 15.0));
        assert!(editor.rotate_selection(std::f64::consts::FRAC_PI_2));
        let (min, max) = editor.selected_bounds().unwrap();
        assert!((max.x - min.x - 5.0).abs() < 1e-6);
        assert!((max.y - min.y - 10.0).abs() < 1e-6);
        // The centre is preserved.
        assert!((f64::midpoint(min.x, max.x) - 15.0).abs() < 1e-6);
    }

    #[test]
    fn scale_doubles_size() {
        let mut editor = editor();
        editor.add_rectangle(Point2::new(0.0, 0.0), Point2::new(10.0, 5.0));
        assert!(editor.scale_selection(2.0, 2.0));
        let (min, max) = editor.selected_bounds().unwrap();
        assert!((max.x - min.x - 20.0).abs() < 1e-6);
        assert!((max.y - min.y - 10.0).abs() < 1e-6);
    }

    #[test]
    fn invalid_scale_is_ignored() {
        let mut editor = editor();
        editor.add_rectangle(Point2::new(0.0, 0.0), Point2::new(10.0, 5.0));
        assert!(!editor.scale_selection(0.0, 1.0));
        assert!(!editor.scale_selection(-1.0, 1.0));
        let (min, max) = editor.selected_bounds().unwrap();
        assert!((max.x - min.x - 10.0).abs() < 1e-6);
    }

    #[test]
    fn flip_keeps_bounds_and_can_be_undone() {
        let mut editor = editor();
        editor.add_line(Point2::new(0.0, 0.0), Point2::new(10.0, 0.0));
        assert!(editor.flip_selection(true));
        let (min, max) = editor.selected_bounds().unwrap();
        assert!((max.x - min.x - 10.0).abs() < 1e-6);
        assert!(editor.undo().is_some());
        let (min, _max) = editor.selected_bounds().unwrap();
        assert!((min.x).abs() < 1e-6);
    }

    #[test]
    fn transforms_require_a_selection() {
        let mut editor = editor();
        assert!(!editor.rotate_selection(1.0));
        assert!(!editor.flip_selection(false));
    }

    #[test]
    fn align_left_matches_selection_bounds() {
        use crate::editor::alignment::AlignMode;

        let mut editor = editor();
        editor.add_rectangle(Point2::new(0.0, 0.0), Point2::new(10.0, 10.0));
        editor.add_rectangle(Point2::new(30.0, 20.0), Point2::new(40.0, 30.0));
        editor.select_all();
        assert!(editor.align_selection(AlignMode::Left));
        for object in &editor.document.objects {
            let (min, _) = object.bounds().unwrap();
            assert!((min.x).abs() < 1e-9);
        }
    }

    #[test]
    fn single_object_aligns_to_page() {
        use crate::editor::alignment::AlignMode;

        let mut editor = editor();
        editor.add_rectangle(Point2::new(30.0, 20.0), Point2::new(40.0, 30.0));
        assert!(editor.align_selection(AlignMode::Left));
        let (min, _) = editor.selected_bounds().unwrap();
        assert!((min.x).abs() < 1e-9);
        assert!(editor.align_selection(AlignMode::Top));
        let (_, max) = editor.selected_bounds().unwrap();
        assert!((max.y - editor.document.height_mm).abs() < 1e-9);
    }

    #[test]
    fn distribute_evens_centres() {
        let mut editor = editor();
        editor.add_rectangle(Point2::new(0.0, 0.0), Point2::new(10.0, 10.0));
        editor.add_rectangle(Point2::new(20.0, 0.0), Point2::new(30.0, 10.0));
        editor.add_rectangle(Point2::new(80.0, 0.0), Point2::new(90.0, 10.0));
        editor.select_all();
        assert!(editor.distribute_selection(true));
        let mut centers: Vec<f64> = editor
            .document
            .objects
            .iter()
            .map(|object| {
                let (min, max) = object.bounds().unwrap();
                f64::midpoint(min.x, max.x)
            })
            .collect();
        centers.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert!((centers[1] - centers[0] - (centers[2] - centers[1])).abs() < 1e-9);
    }

    #[test]
    fn distribute_requires_three_objects() {
        let mut editor = editor();
        editor.add_rectangle(Point2::new(0.0, 0.0), Point2::new(10.0, 10.0));
        editor.add_rectangle(Point2::new(20.0, 0.0), Point2::new(30.0, 10.0));
        editor.select_all();
        assert!(!editor.distribute_selection(true));
        assert!(!editor.distribute_selection(false));
    }

    #[test]
    fn image_profile_applies_to_selected_images_only() {
        use crate::objects::{ImageMode, ImageObject};
        use crate::profiles::ImageProfile;

        let mut editor = editor();
        let layer_id = editor.document.layers[0].id.clone();
        editor.document.add_object(Object::new(
            layer_id.clone(),
            Shape::Image(ImageObject::new("photo.png", 10, 10, 96.0)),
        ));
        editor.document.add_object(Object::new(
            layer_id,
            Shape::Rect {
                width: 5.0,
                height: 5.0,
                corner_radius: 0.0,
            },
        ));
        editor.select_all();
        let mut profile = ImageProfile::new("Dither");
        profile.settings.mode = ImageMode::FloydSteinberg;
        profile.settings.contrast = 1.4;
        assert_eq!(editor.apply_image_profile(&profile), 1);
        match &editor.document.objects[0].shape {
            Shape::Image(image) => {
                assert_eq!(image.settings.mode, ImageMode::FloydSteinberg);
                assert!((image.settings.contrast - 1.4).abs() < 1e-9);
            }
            other => panic!("unexpected shape: {other:?}"),
        }
    }

    #[test]
    fn image_profile_without_selection_does_nothing() {
        use crate::profiles::ImageProfile;
        let mut editor = editor();
        assert_eq!(editor.apply_image_profile(&ImageProfile::new("x")), 0);
    }

    #[test]
    fn project_profile_sets_work_area_and_can_be_undone() {
        use crate::profiles::ProjectProfile;

        let mut editor = editor();
        let before = editor.document.width_mm;
        let profile = ProjectProfile::new("Small", 120.0, 80.0);
        assert!(editor.apply_project_profile(&profile));
        assert_eq!(editor.document.width_mm, 120.0);
        assert_eq!(editor.document.height_mm, 80.0);
        assert_eq!(editor.canvas.grid_spacing_mm, 10.0);
        assert!(editor.undo().is_some());
        assert_eq!(editor.document.width_mm, before);
    }

    #[test]
    fn invalid_project_profile_is_rejected() {
        use crate::profiles::ProjectProfile;

        let mut editor = editor();
        let mut profile = ProjectProfile::new("Bad", 0.0, 80.0);
        assert!(!editor.apply_project_profile(&profile));
        profile.width_mm = 100.0;
        profile.snap_mm = 0.0;
        assert!(!editor.apply_project_profile(&profile));
    }

    #[test]
    fn polygon_has_the_requested_number_of_sides() {
        let mut editor = editor();
        let id = editor
            .add_polygon(Point2::new(20.0, 20.0), Point2::new(30.0, 20.0), 6)
            .unwrap();
        let path = editor.document.object(&id).unwrap().world_path();
        assert!(path.closed);
        assert_eq!(path.segment_count(), 6);
        let (min, max) = editor.document.object(&id).unwrap().bounds().unwrap();
        assert!((min.x - 10.0).abs() < 1e-6);
        assert!((max.x - 30.0).abs() < 1e-6);
    }

    #[test]
    fn polygon_rejects_invalid_parameters() {
        let mut editor = editor();
        assert!(
            editor
                .add_polygon(Point2::new(0.0, 0.0), Point2::new(10.0, 0.0), 2)
                .is_none()
        );
        assert!(
            editor
                .add_polygon(Point2::new(0.0, 0.0), Point2::new(0.0, 0.0), 6)
                .is_none()
        );
        assert_eq!(editor.document.objects.len(), 0);
    }

    #[test]
    fn polygon_is_undoable() {
        let mut editor = editor();
        editor.add_polygon(Point2::new(20.0, 20.0), Point2::new(25.0, 20.0), 5);
        assert_eq!(editor.document.objects.len(), 1);
        assert!(editor.undo().is_some());
        assert_eq!(editor.document.objects.len(), 0);
    }

    #[test]
    fn freehand_stroke_becomes_a_polyline() {
        let mut editor = editor();
        let points = [
            Point2::new(0.0, 0.0),
            Point2::new(1.0, 0.0),
            Point2::new(2.0, 1.0),
        ];
        let id = editor.add_polyline(&points, false).unwrap();
        let path = editor.document.object(&id).unwrap().world_path();
        assert_eq!(path.segment_count(), 2);
        assert!(!path.closed);
    }

    #[test]
    fn freehand_drops_duplicate_points_and_rejects_short_strokes() {
        let mut editor = editor();
        let points = [
            Point2::new(0.0, 0.0),
            Point2::new(0.0, 0.0),
            Point2::new(0.0, 0.0),
        ];
        assert!(editor.add_polyline(&points, false).is_none());
        let points = [Point2::new(0.0, 0.0), Point2::new(10.0, 0.0)];
        let id = editor.add_polyline(&points, true).unwrap();
        let path = editor.document.object(&id).unwrap().world_path();
        assert!(path.closed);
    }

    #[test]
    fn grid_duplication_places_copies_on_the_grid() {
        let mut editor = editor();
        editor.add_rectangle(Point2::new(0.0, 0.0), Point2::new(10.0, 10.0));
        assert_eq!(editor.duplicate_grid(2, 3, 20.0, 15.0), 5);
        assert_eq!(editor.document.objects.len(), 6);
        assert_eq!(editor.selection.len(), 5);
        let mut xs: Vec<f64> = editor
            .document
            .objects
            .iter()
            .map(|object| object.bounds().unwrap().0.x)
            .collect();
        xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
        xs.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
        assert_eq!(xs, vec![0.0, 20.0]);
    }

    #[test]
    fn grid_duplication_rejects_invalid_counts() {
        let mut editor = editor();
        editor.add_rectangle(Point2::new(0.0, 0.0), Point2::new(10.0, 10.0));
        assert_eq!(editor.duplicate_grid(0, 2, 10.0, 10.0), 0);
        assert_eq!(editor.duplicate_grid(2, 0, 10.0, 10.0), 0);
        assert_eq!(editor.duplicate_grid(101, 1, 10.0, 10.0), 0);
        assert_eq!(editor.document.objects.len(), 1);
    }

    #[test]
    fn grid_duplication_is_undoable() {
        let mut editor = editor();
        editor.add_rectangle(Point2::new(0.0, 0.0), Point2::new(10.0, 10.0));
        editor.duplicate_grid(2, 2, 20.0, 20.0);
        assert_eq!(editor.document.objects.len(), 4);
        assert!(editor.undo().is_some());
        assert_eq!(editor.document.objects.len(), 1);
    }

    #[test]
    fn grouping_selects_and_ungroups_together() {
        let mut editor = editor();
        let first = editor
            .add_rectangle(Point2::new(0.0, 0.0), Point2::new(10.0, 10.0))
            .unwrap();
        editor.selection.clear();
        let second = editor
            .add_rectangle(Point2::new(20.0, 0.0), Point2::new(30.0, 10.0))
            .unwrap();
        editor.selection.clear();
        editor.selection.add(first.clone());
        editor.selection.add(second.clone());
        assert!(editor.group_selection());
        assert_eq!(editor.group_members(&first).unwrap().len(), 2);
        editor.selection.clear();
        editor.select_at(Point2::new(0.0, 5.0), false, 0.5);
        assert_eq!(editor.selection.len(), 2);
        assert!(editor.ungroup_selection());
        assert!(editor.group_members(&first).is_none());
    }

    #[test]
    fn layers_can_be_duplicated_with_their_objects() {
        let mut editor = editor();
        editor.add_rectangle(Point2::new(0.0, 0.0), Point2::new(10.0, 10.0));
        let layer = editor.document.layers[0].id.clone();
        let copy = editor.duplicate_layer(&layer).unwrap();
        assert_ne!(copy, layer);
        assert_eq!(editor.document.layers.len(), 2);
        assert_eq!(editor.document.objects.len(), 2);
        assert_eq!(
            editor
                .document
                .objects
                .iter()
                .filter(|object| object.layer_id == copy)
                .count(),
            1
        );
        assert!(editor.undo().is_some());
        assert_eq!(editor.document.objects.len(), 1);
        assert!(editor.duplicate_layer("missing").is_none());
    }

    #[test]
    fn layer_objects_can_be_selected_and_grouped() {
        let mut editor = editor();
        editor.add_rectangle(Point2::new(0.0, 0.0), Point2::new(4.0, 4.0));
        editor.add_rectangle(Point2::new(10.0, 0.0), Point2::new(14.0, 4.0));
        let layer = editor.document.layers[0].id.clone();
        let other = editor.document.add_layer("Other").id.clone();
        editor.selection.clear();
        assert_eq!(editor.select_layer_objects(&layer), 2);
        assert_eq!(editor.select_layer_objects(&other), 0);
        assert_eq!(editor.group_layer_objects(&layer), 2);
        assert!(editor.document.objects[0].group_id.is_some());
        assert_eq!(editor.group_layer_objects(&other), 0);
    }

    #[test]
    fn objects_are_selected_by_colour() {
        use crate::document::Style;

        let mut editor = editor();
        editor.add_rectangle(Point2::new(0.0, 0.0), Point2::new(4.0, 4.0));
        editor.add_rectangle(Point2::new(10.0, 0.0), Point2::new(14.0, 4.0));
        editor.document.objects[0].style = Style {
            stroke: Some("#ff0000".to_string()),
            fill: None,
            stroke_width_mm: 0.2,
        };
        editor.selection.clear();
        assert_eq!(editor.select_by_color("#FF0000"), 1);
        assert!(editor.selection.contains(&editor.document.objects[0].id));
        assert_eq!(editor.select_by_color("not-a-colour"), 0);
    }

    #[test]
    fn group_selection_can_be_disabled() {
        let mut editor = editor();
        let first = editor
            .add_rectangle(Point2::new(0.0, 0.0), Point2::new(10.0, 10.0))
            .unwrap();
        editor.selection.clear();
        let second = editor
            .add_rectangle(Point2::new(20.0, 0.0), Point2::new(30.0, 10.0))
            .unwrap();
        editor.selection.clear();
        editor.selection.add(first.clone());
        editor.selection.add(second);
        assert!(editor.group_selection());
        editor.select_whole_group = false;
        editor.selection.clear();
        editor.select_at(Point2::new(0.0, 5.0), false, 0.5);
        assert_eq!(editor.selection.len(), 1);
        assert!(editor.selection.contains(&first));
        editor.select_whole_group = true;
        editor.selection.clear();
        editor.select_at(Point2::new(0.0, 5.0), false, 0.5);
        assert_eq!(editor.selection.len(), 2);
    }

    #[test]
    fn grouping_requires_two_objects() {
        let mut editor = editor();
        editor.add_rectangle(Point2::new(0.0, 0.0), Point2::new(10.0, 10.0));
        assert!(!editor.group_selection());
        assert!(!editor.ungroup_selection());
    }

    #[test]
    fn ungroup_keeps_objects_and_can_be_undone() {
        let mut editor = editor();
        let first = editor
            .add_rectangle(Point2::new(0.0, 0.0), Point2::new(10.0, 10.0))
            .unwrap();
        editor.selection.clear();
        let second = editor
            .add_line(Point2::new(20.0, 0.0), Point2::new(30.0, 10.0))
            .unwrap();
        editor.selection.add(first.clone());
        editor.selection.add(second);
        editor.group_selection();
        assert!(editor.ungroup_selection());
        assert_eq!(editor.document.objects.len(), 2);
        assert!(editor.undo().is_some());
        assert!(
            editor.document.object(&first).unwrap().group_id.is_some(),
            "undo restores the grouped state"
        );
    }

    #[test]
    fn grid_array_applies_margins_and_fill_order() {
        let mut editor = editor();
        editor.add_rectangle(Point2::new(0.0, 0.0), Point2::new(4.0, 4.0));
        let options = GridOptions {
            columns: 2,
            rows: 2,
            dx: 10.0,
            dy: 10.0,
            margin_x: 5.0,
            margin_y: 3.0,
            column_major: true,
        };
        assert_eq!(editor.duplicate_grid_with(&options), 3);
        let mut mins: Vec<(f64, f64)> = editor
            .document
            .objects
            .iter()
            .filter_map(crate::document::Object::bounds)
            .map(|(min, _)| (min.x, min.y))
            .collect();
        mins.sort_by(|left, right| {
            left.0
                .partial_cmp(&right.0)
                .unwrap()
                .then(left.1.partial_cmp(&right.1).unwrap())
        });
        assert_eq!(
            mins,
            vec![(0.0, 0.0), (5.0, 13.0), (15.0, 3.0), (15.0, 13.0)]
        );
    }

    #[test]
    fn grid_fill_order_changes_the_document_order() {
        let mut editor = editor();
        editor.add_rectangle(Point2::new(0.0, 0.0), Point2::new(2.0, 2.0));
        let row_major = GridOptions {
            columns: 2,
            rows: 2,
            dx: 10.0,
            dy: 10.0,
            ..GridOptions::default()
        };
        editor.duplicate_grid_with(&row_major);
        let x_row: Vec<f64> = editor
            .document
            .objects
            .iter()
            .skip(1)
            .filter_map(crate::document::Object::bounds)
            .map(|(min, _)| min.x)
            .collect();
        assert_eq!(x_row, vec![10.0, 0.0, 10.0]);

        let mut editor = editor_with_one_rect();
        let column_major = GridOptions {
            columns: 2,
            rows: 2,
            dx: 10.0,
            dy: 10.0,
            column_major: true,
            ..GridOptions::default()
        };
        editor.duplicate_grid_with(&column_major);
        let x_column: Vec<f64> = editor
            .document
            .objects
            .iter()
            .skip(1)
            .filter_map(crate::document::Object::bounds)
            .map(|(min, _)| min.x)
            .collect();
        assert_eq!(x_column, vec![0.0, 10.0, 10.0]);
    }

    fn editor_with_one_rect() -> EditorState {
        let mut editor = editor();
        editor.add_rectangle(Point2::new(0.0, 0.0), Point2::new(2.0, 2.0));
        editor
    }

    #[test]
    fn grid_array_rejects_bad_parameters() {
        let mut editor = editor_with_one_rect();
        let good = GridOptions::default();
        assert_eq!(editor.duplicate_grid_with(&good), 3);
        assert_eq!(
            editor.duplicate_grid_with(&GridOptions { columns: 0, ..good }),
            0
        );
        assert_eq!(
            editor.duplicate_grid_with(&GridOptions { rows: 101, ..good }),
            0
        );
        assert_eq!(
            editor.duplicate_grid_with(&GridOptions {
                dx: f64::NAN,
                ..good
            }),
            0
        );
    }

    #[test]
    fn selected_objects_move_to_the_front_and_back() {
        let mut editor = editor();
        let first = editor
            .add_rectangle(Point2::new(0.0, 0.0), Point2::new(2.0, 2.0))
            .unwrap();
        let second = editor
            .add_rectangle(Point2::new(4.0, 0.0), Point2::new(6.0, 2.0))
            .unwrap();
        let third = editor
            .add_rectangle(Point2::new(8.0, 0.0), Point2::new(10.0, 2.0))
            .unwrap();
        let order = |editor: &EditorState| -> Vec<String> {
            editor
                .document
                .objects
                .iter()
                .map(|object| object.id.clone())
                .collect()
        };
        editor.selection.clear();
        editor.selection.add(first.clone());
        assert_eq!(editor.bring_selection_to_front(), 1);
        assert_eq!(
            order(&editor),
            vec![second.clone(), third.clone(), first.clone()]
        );
        assert!(editor.undo().is_some());
        editor.selection.clear();
        editor.selection.add(third.clone());
        assert_eq!(editor.send_selection_to_back(), 1);
        assert_eq!(order(&editor), vec![third, first, second]);
        editor.selection.clear();
        assert_eq!(editor.bring_selection_to_front(), 0);
    }

    #[test]
    fn selection_can_be_distributed_across_the_page() {
        let mut editor = editor();
        let first = editor
            .add_rectangle(Point2::new(0.0, 0.0), Point2::new(10.0, 10.0))
            .unwrap();
        let second = editor
            .add_rectangle(Point2::new(20.0, 0.0), Point2::new(30.0, 10.0))
            .unwrap();
        let third = editor
            .add_rectangle(Point2::new(80.0, 0.0), Point2::new(90.0, 10.0))
            .unwrap();
        editor.select_all();
        assert!(editor.distribute_selection_on_page(true));
        let centres: Vec<f64> = [first, second, third]
            .iter()
            .map(|id| {
                let (min, max) = editor.document.object(id).unwrap().bounds().unwrap();
                f64::midpoint(min.x, max.x)
            })
            .collect();
        assert!((centres[0] - 50.0).abs() < 1e-6);
        assert!((centres[1] - 100.0).abs() < 1e-6);
        assert!((centres[2] - 150.0).abs() < 1e-6);
        assert!(editor.undo().is_some());
        editor.selection.clear();
        assert!(!editor.distribute_selection_on_page(true));
    }

    #[test]
    fn selection_can_be_centred_on_the_page() {
        let mut editor = editor();
        let id = editor
            .add_rectangle(Point2::new(0.0, 0.0), Point2::new(10.0, 10.0))
            .unwrap();
        editor.select_all();
        assert!(editor.center_selection_on_page());
        let (min, max) = editor.document.object(&id).unwrap().bounds().unwrap();
        assert!((f64::midpoint(min.x, max.x) - 100.0).abs() < 1e-6);
        assert!((f64::midpoint(min.y, max.y) - 50.0).abs() < 1e-6);
        assert!(editor.undo().is_some());
        editor.selection.clear();
        assert!(!editor.center_selection_on_page());
    }

    #[test]
    fn zoom_to_selection_centres_the_bounds() {
        let mut editor = editor();
        editor.add_rectangle(Point2::new(20.0, 10.0), Point2::new(30.0, 20.0));
        editor.select_all();
        assert!(editor.zoom_to_selection(800.0, 600.0));
        let (min, max) = editor.selected_bounds().unwrap();
        let (mid_x, mid_y) = (f64::midpoint(min.x, max.x), f64::midpoint(min.y, max.y));
        let (screen_x, screen_y) = editor.canvas.doc_to_screen(mid_x, mid_y);
        assert!((screen_x - 400.0).abs() < 1e-6);
        assert!((screen_y - 300.0).abs() < 1e-6);
        editor.selection.clear();
        assert!(!editor.zoom_to_selection(800.0, 600.0));
    }

    #[test]
    fn operations_can_be_assigned_to_a_whole_layer() {
        use crate::operations::OperationType;

        let mut editor = editor();
        let first = editor
            .add_rectangle(Point2::new(0.0, 0.0), Point2::new(4.0, 4.0))
            .unwrap();
        let second = editor
            .add_rectangle(Point2::new(10.0, 0.0), Point2::new(14.0, 4.0))
            .unwrap();
        let operation = editor.add_operation("Cut", OperationType::OutlineCutting);
        let layer = editor.document.layers[0].id.clone();
        assert_eq!(
            editor.assign_operation_to_layer(&layer, Some(&operation)),
            2
        );
        for id in [&first, &second] {
            assert_eq!(
                editor.document.object(id).unwrap().operation_id.as_deref(),
                Some(operation.as_str())
            );
        }
        assert_eq!(
            editor.assign_operation_to_layer(&layer, Some(&operation)),
            0
        );
        assert!(editor.undo().is_some());
        assert_eq!(
            editor
                .document
                .object(&first)
                .unwrap()
                .operation_id
                .as_deref(),
            None
        );
        assert_eq!(
            editor.assign_operation_to_layer(&layer, Some(&operation)),
            2
        );
        assert_eq!(editor.assign_operation_to_layer(&layer, None), 2);
        assert_eq!(
            editor.assign_operation_to_layer("missing", Some(&operation)),
            0
        );
        assert_eq!(editor.assign_operation_to_layer(&layer, Some("missing")), 0);
    }

    #[test]
    fn a_profile_can_be_applied_to_an_existing_operation() {
        use crate::operations::OperationType;
        use crate::profiles::OperationProfile;

        let mut editor = editor();
        let id = editor.add_operation("Cut", OperationType::OutlineCutting);
        let mut profile = OperationProfile::new("Fast cut", OperationType::OutlineCutting);
        profile.params.speed = 1800.0;
        profile.params.power = 70.0;
        assert_eq!(editor.apply_profile_to_operation(&id, &profile).unwrap(), 2);
        assert_eq!(editor.document.operations[0].params.speed, 1800.0);
        assert_eq!(editor.document.operations[0].params.power, 70.0);
        assert!(editor.undo().is_some());
        assert_eq!(editor.document.operations[0].params.speed, 1000.0);

        // Applying the same values again is a no-op.
        let operation = &mut editor.document.operations[0];
        operation.params = profile.params.clone();
        assert_eq!(editor.apply_profile_to_operation(&id, &profile).unwrap(), 0);

        let wrong = OperationProfile::new("Engrave", OperationType::LineEngraving);
        let error = editor.apply_profile_to_operation(&id, &wrong).unwrap_err();
        assert!(error.contains("not"));
        assert!(
            editor
                .apply_profile_to_operation("missing", &profile)
                .is_err()
        );
    }

    #[test]
    fn operation_dependencies_can_be_set() {
        use crate::operations::OperationType;

        let mut editor = editor();
        let first = editor.add_operation("First", OperationType::Line);
        let second = editor.add_operation("Second", OperationType::Fill);
        assert!(editor.set_operation_dependency(&second, Some(&first)));
        assert_eq!(
            editor.document.operations[1].depends_on,
            vec![first.clone()]
        );
        assert!(editor.set_operation_dependency(&second, None));
        assert_eq!(editor.document.operations[1].depends_on.len(), 0);
        assert!(!editor.set_operation_dependency(&first, Some(&first)));
        assert!(!editor.set_operation_dependency(&first, Some("missing")));
        assert!(!editor.set_operation_dependency("missing", None));
        assert!(editor.undo().is_some());
        assert_eq!(
            editor.document.operations[1].depends_on,
            vec![first.clone()]
        );
        assert!(editor.undo().is_some());
        assert_eq!(editor.document.operations[1].depends_on.len(), 0);
    }

    #[test]
    fn operations_can_be_duplicated() {
        use crate::operations::{OperationParams, OperationType};

        let mut editor = editor();
        let id = editor.add_operation("Cut", OperationType::OutlineCutting);
        if let Some(operation) = editor
            .document
            .operations
            .iter_mut()
            .find(|operation| operation.id == id)
        {
            operation.params = OperationParams {
                speed: 456.0,
                power: 78.0,
                passes: 3,
                line_interval_mm: 0.2,
                ..OperationParams::default()
            };
        }
        let copy = editor.duplicate_operation(&id).unwrap();
        assert_ne!(copy, id);
        assert_eq!(editor.document.operations.len(), 2);
        assert_eq!(editor.document.operations[1].id, copy);
        assert!(editor.document.operations[1].name.ends_with("copy"));
        assert_eq!(
            editor.document.operations[1].params,
            editor.document.operations[0].params
        );
        assert!(editor.undo().is_some());
        assert_eq!(editor.document.operations.len(), 1);
        assert!(editor.duplicate_operation("missing").is_none());
    }

    #[test]
    fn operations_can_be_reordered() {
        use crate::operations::OperationType;

        let mut editor = editor();
        let first = editor.add_operation("First", OperationType::OutlineCutting);
        let second = editor.add_operation("Second", OperationType::LineEngraving);
        let third = editor.add_operation("Third", OperationType::Fill);
        assert!(editor.move_operation(&third, -1));
        let order: Vec<&str> = editor
            .document
            .operations
            .iter()
            .map(|operation| operation.name.as_str())
            .collect();
        assert_eq!(order, vec!["First", "Third", "Second"]);
        assert!(editor.move_operation(&first, 1));
        let order: Vec<&str> = editor
            .document
            .operations
            .iter()
            .map(|operation| operation.name.as_str())
            .collect();
        assert_eq!(order, vec!["Third", "First", "Second"]);
        assert!(editor.undo().is_some());
        let order: Vec<&str> = editor
            .document
            .operations
            .iter()
            .map(|operation| operation.name.as_str())
            .collect();
        assert_eq!(order, vec!["First", "Third", "Second"]);
        let _ = second;
    }

    #[test]
    fn reordering_stops_at_the_ends() {
        use crate::operations::OperationType;

        let mut editor = editor();
        let first = editor.add_operation("First", OperationType::OutlineCutting);
        let second = editor.add_operation("Second", OperationType::Fill);
        assert!(!editor.move_operation(&first, -1));
        assert!(!editor.move_operation(&second, 1));
        assert!(!editor.move_operation("missing", 1));
        assert_eq!(editor.document.operations.len(), 2);
    }

    #[test]
    fn reordering_keeps_object_assignments() {
        use crate::operations::OperationType;

        let mut editor = editor();
        let first = editor.add_operation("First", OperationType::OutlineCutting);
        let second = editor.add_operation("Second", OperationType::Fill);
        editor.add_rectangle(Point2::new(0.0, 0.0), Point2::new(10.0, 10.0));
        editor.assign_operation(Some(&second));
        assert!(editor.move_operation(&second, -1));
        assert_eq!(
            editor.document.objects[0].operation_id.as_deref(),
            Some(second.as_str())
        );
        assert_eq!(editor.document.operations[0].id, second);
        assert_eq!(editor.document.operations[1].id, first);
    }

    fn area_of(editor: &EditorState, id: &str) -> f64 {
        let object = editor
            .document
            .objects
            .iter()
            .find(|object| object.id == id)
            .expect("object");
        let points: Vec<(f64, f64)> = object
            .world_path()
            .flatten(0.05)
            .into_iter()
            .map(|point| (point.x, point.y))
            .collect();
        crate::geometry::boolean::polygon_area(&points).abs()
    }

    #[test]
    fn union_merges_two_overlapping_shapes() {
        let mut editor = editor();
        let first = editor
            .add_rectangle(Point2::new(0.0, 0.0), Point2::new(4.0, 4.0))
            .unwrap();
        let second = editor
            .add_rectangle(Point2::new(2.0, 2.0), Point2::new(6.0, 6.0))
            .unwrap();
        editor.selection.clear();
        editor.selection.add(first.clone());
        editor.selection.add(second);
        assert_eq!(
            editor
                .boolean_selection(crate::geometry::BooleanOp::Union)
                .unwrap(),
            1
        );
        assert_eq!(editor.document.objects.len(), 1);
        let id = editor.document.objects[0].id.clone();
        assert!((area_of(&editor, &id) - 28.0).abs() < 1e-3);
        assert!(editor.selection.contains(&id));
        assert!(editor.document.objects[0].name.starts_with("Union"));
    }

    #[test]
    fn difference_subtracts_the_upper_shape() {
        let mut editor = editor();
        let lower = editor
            .add_rectangle(Point2::new(0.0, 0.0), Point2::new(4.0, 4.0))
            .unwrap();
        let upper = editor
            .add_rectangle(Point2::new(2.0, 2.0), Point2::new(6.0, 6.0))
            .unwrap();
        editor.selection.clear();
        editor.selection.add(lower);
        editor.selection.add(upper);
        editor
            .boolean_selection(crate::geometry::BooleanOp::Difference)
            .unwrap();
        assert_eq!(editor.document.objects.len(), 1);
        let id = editor.document.objects[0].id.clone();
        assert!((area_of(&editor, &id) - 12.0).abs() < 1e-3);
    }

    #[test]
    fn boolean_keeps_style_and_operation() {
        use crate::operations::OperationType;

        let mut editor = editor();
        let first = editor
            .add_rectangle(Point2::new(0.0, 0.0), Point2::new(4.0, 4.0))
            .unwrap();
        let second = editor
            .add_rectangle(Point2::new(2.0, 2.0), Point2::new(6.0, 6.0))
            .unwrap();
        let operation = editor.add_operation("Engrave", OperationType::LineEngraving);
        editor.selection.clear();
        editor.selection.add(first);
        editor.selection.add(second);
        editor.assign_operation(Some(&operation));
        editor
            .boolean_selection(crate::geometry::BooleanOp::Intersection)
            .unwrap();
        let object = &editor.document.objects[0];
        assert_eq!(object.operation_id.as_deref(), Some(operation.as_str()));
        assert_eq!(object.layer_id, editor.document.layers[0].id);
    }

    #[test]
    fn boolean_requires_two_closed_shapes() {
        let mut editor = editor();
        let rect = editor
            .add_rectangle(Point2::new(0.0, 0.0), Point2::new(4.0, 4.0))
            .unwrap();
        assert!(
            editor
                .boolean_selection(crate::geometry::BooleanOp::Union)
                .is_err()
        );
        let line = editor
            .add_line(Point2::new(0.0, 0.0), Point2::new(10.0, 0.0))
            .unwrap();
        editor.selection.clear();
        editor.selection.add(rect);
        editor.selection.add(line);
        let error = editor
            .boolean_selection(crate::geometry::BooleanOp::Union)
            .unwrap_err();
        assert!(error.contains("closed"));
        assert_eq!(editor.document.objects.len(), 2);
    }

    #[test]
    fn boolean_without_area_is_rejected() {
        let mut editor = editor();
        let first = editor
            .add_rectangle(Point2::new(0.0, 0.0), Point2::new(4.0, 4.0))
            .unwrap();
        let second = editor
            .add_rectangle(Point2::new(10.0, 10.0), Point2::new(14.0, 14.0))
            .unwrap();
        editor.selection.clear();
        editor.selection.add(first);
        editor.selection.add(second);
        let error = editor
            .boolean_selection(crate::geometry::BooleanOp::Intersection)
            .unwrap_err();
        assert!(error.contains("no area"));
        assert_eq!(editor.document.objects.len(), 2);
    }

    #[test]
    fn boolean_is_undoable() {
        let mut editor = editor();
        let first = editor
            .add_rectangle(Point2::new(0.0, 0.0), Point2::new(4.0, 4.0))
            .unwrap();
        let second = editor
            .add_rectangle(Point2::new(2.0, 2.0), Point2::new(6.0, 6.0))
            .unwrap();
        editor.selection.clear();
        editor.selection.add(first);
        editor.selection.add(second);
        editor
            .boolean_selection(crate::geometry::BooleanOp::Union)
            .unwrap();
        assert_eq!(editor.document.objects.len(), 1);
        assert!(editor.undo().is_some());
        assert_eq!(editor.document.objects.len(), 2);
    }

    #[test]
    fn detects_duplicate_outlines() {
        let mut editor = editor();
        let first = editor
            .add_rectangle(Point2::new(0.0, 0.0), Point2::new(4.0, 4.0))
            .unwrap();
        let second = editor
            .add_rectangle(Point2::new(0.0, 0.0), Point2::new(4.0, 4.0))
            .unwrap();
        let third = editor
            .add_rectangle(Point2::new(10.0, 0.0), Point2::new(14.0, 4.0))
            .unwrap();
        let groups = editor.duplicate_groups();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0], vec![first, second]);
        assert!(!groups[0].contains(&third));
    }

    #[test]
    fn duplicate_removal_keeps_one_copy() {
        let mut editor = editor();
        editor
            .add_rectangle(Point2::new(0.0, 0.0), Point2::new(4.0, 4.0))
            .unwrap();
        editor
            .add_rectangle(Point2::new(0.0, 0.0), Point2::new(4.0, 4.0))
            .unwrap();
        editor
            .add_rectangle(Point2::new(0.0, 0.0), Point2::new(4.0, 4.0))
            .unwrap();
        assert_eq!(editor.remove_duplicates(), 2);
        assert_eq!(editor.document.objects.len(), 1);
        assert!(editor.undo().is_some());
        assert_eq!(editor.document.objects.len(), 3);
    }

    #[test]
    fn duplicate_removal_does_nothing_without_duplicates() {
        let mut editor = editor();
        editor
            .add_rectangle(Point2::new(0.0, 0.0), Point2::new(4.0, 4.0))
            .unwrap();
        editor
            .add_rectangle(Point2::new(10.0, 0.0), Point2::new(14.0, 4.0))
            .unwrap();
        assert_eq!(editor.remove_duplicates(), 0);
        assert_eq!(editor.document.objects.len(), 2);
    }

    #[test]
    fn finds_intersections_between_selected_objects() {
        let mut editor = editor();
        let first = editor
            .add_rectangle(Point2::new(0.0, 0.0), Point2::new(4.0, 4.0))
            .unwrap();
        let second = editor
            .add_rectangle(Point2::new(2.0, 2.0), Point2::new(6.0, 6.0))
            .unwrap();
        editor.selection.clear();
        editor.selection.add(first.clone());
        editor.selection.add(second.clone());
        let hits = editor.intersections();
        assert_eq!(hits.len(), 2);
        assert!(
            hits.iter()
                .all(|hit| hit.first == first && hit.second == second)
        );
        let points: Vec<(f64, f64)> = hits.iter().map(|hit| (hit.point.x, hit.point.y)).collect();
        assert!(points.contains(&(2.0, 4.0)));
        assert!(points.contains(&(4.0, 2.0)));
    }

    #[test]
    fn intersections_ignore_disjoint_outlines() {
        let mut editor = editor();
        editor
            .add_rectangle(Point2::new(0.0, 0.0), Point2::new(4.0, 4.0))
            .unwrap();
        editor
            .add_rectangle(Point2::new(10.0, 0.0), Point2::new(14.0, 4.0))
            .unwrap();
        assert_eq!(editor.intersections().len(), 0);
    }

    #[test]
    fn intersections_use_the_whole_document_without_a_selection() {
        let mut editor = editor();
        editor
            .add_rectangle(Point2::new(0.0, 0.0), Point2::new(4.0, 4.0))
            .unwrap();
        editor
            .add_rectangle(Point2::new(2.0, 2.0), Point2::new(6.0, 6.0))
            .unwrap();
        editor.selection.clear();
        assert_eq!(editor.intersections().len(), 2);
    }

    #[test]
    fn trim_removes_the_parts_inside_the_trimmer() {
        let mut editor = editor();
        let line = editor
            .add_line(Point2::new(0.0, 0.0), Point2::new(10.0, 0.0))
            .unwrap();
        let trimmer = editor
            .add_rectangle(Point2::new(4.0, -1.0), Point2::new(6.0, 1.0))
            .unwrap();
        editor.selection.clear();
        editor.selection.add(line);
        editor.selection.add(trimmer.clone());
        assert_eq!(editor.trim_selection().unwrap(), 1);
        let mut pieces: Vec<(f64, f64)> = editor
            .document
            .objects
            .iter()
            .filter(|object| object.id != trimmer)
            .filter_map(Object::bounds)
            .map(|(min, max)| (min.x, max.x))
            .collect();
        pieces.sort_by(|left, right| left.0.partial_cmp(&right.0).unwrap());
        assert_eq!(pieces.len(), 2);
        assert!((pieces[0].0 - 0.0).abs() < 1e-6);
        assert!((pieces[0].1 - 4.0).abs() < 1e-6);
        assert!((pieces[1].0 - 6.0).abs() < 1e-6);
        assert!((pieces[1].1 - 10.0).abs() < 1e-6);
        assert!(editor.document.object(&trimmer).is_some());
    }

    #[test]
    fn trim_removes_an_object_fully_inside() {
        let mut editor = editor();
        let line = editor
            .add_line(Point2::new(4.5, 0.0), Point2::new(5.5, 0.0))
            .unwrap();
        let trimmer = editor
            .add_rectangle(Point2::new(4.0, -1.0), Point2::new(6.0, 1.0))
            .unwrap();
        editor.selection.clear();
        editor.selection.add(line.clone());
        editor.selection.add(trimmer);
        assert_eq!(editor.trim_selection().unwrap(), 1);
        assert!(editor.document.object(&line).is_none());
        assert_eq!(editor.document.objects.len(), 1);
    }

    #[test]
    fn trim_rejects_an_open_or_non_crossing_trimmer() {
        let mut editor = editor();
        let first = editor
            .add_line(Point2::new(0.0, 0.0), Point2::new(10.0, 0.0))
            .unwrap();
        let second = editor
            .add_line(Point2::new(0.0, 5.0), Point2::new(10.0, 5.0))
            .unwrap();
        editor.selection.clear();
        editor.selection.add(first.clone());
        editor.selection.add(second.clone());
        let error = editor.trim_selection().unwrap_err();
        assert!(error.contains("not a closed trimmer"), "{error}");
        editor.selection.clear();
        editor.selection.add(first);
        assert!(editor.trim_selection().is_err());
    }

    #[test]
    fn trim_is_undoable() {
        let mut editor = editor();
        let line = editor
            .add_line(Point2::new(0.0, 0.0), Point2::new(10.0, 0.0))
            .unwrap();
        let trimmer = editor
            .add_rectangle(Point2::new(4.0, -1.0), Point2::new(6.0, 1.0))
            .unwrap();
        editor.selection.clear();
        editor.selection.add(line);
        editor.selection.add(trimmer);
        editor.trim_selection().unwrap();
        assert_eq!(editor.document.objects.len(), 3);
        assert!(editor.undo().is_some());
        assert_eq!(editor.document.objects.len(), 2);
    }

    #[test]
    fn delete_overlapping_segments_splits_shared_edges() {
        let mut editor = editor();
        let first = editor
            .add_rectangle(Point2::new(0.0, 0.0), Point2::new(10.0, 10.0))
            .unwrap();
        let second = editor
            .add_rectangle(Point2::new(10.0, 0.0), Point2::new(20.0, 10.0))
            .unwrap();
        editor.selection.clear();
        editor.selection.add(first);
        editor.selection.add(second);
        assert_eq!(editor.delete_overlapping_segments().unwrap(), 2);
        // The first rectangle splits in two; the second keeps one run of three
        // segments. Nothing is closed any more, because the shared edge is gone.
        assert_eq!(editor.document.objects.len(), 3);
        for object in &editor.document.objects {
            assert!(matches!(&object.shape, Shape::Path(path) if !path.closed));
        }
    }

    #[test]
    fn delete_overlapping_segments_removes_identical_outlines() {
        let mut editor = editor();
        let first = editor
            .add_rectangle(Point2::new(0.0, 0.0), Point2::new(10.0, 10.0))
            .unwrap();
        let second = editor
            .add_rectangle(Point2::new(0.0, 0.0), Point2::new(10.0, 10.0))
            .unwrap();
        editor.selection.clear();
        editor.selection.add(first);
        editor.selection.add(second);
        assert_eq!(editor.delete_overlapping_segments().unwrap(), 2);
        assert_eq!(editor.document.objects.len(), 0);
        assert!(editor.undo().is_some());
        assert_eq!(editor.document.objects.len(), 2);
    }

    #[test]
    fn delete_overlapping_segments_reports_nothing_found() {
        let mut editor = editor();
        let first = editor
            .add_rectangle(Point2::new(0.0, 0.0), Point2::new(10.0, 10.0))
            .unwrap();
        let second = editor
            .add_rectangle(Point2::new(20.0, 0.0), Point2::new(30.0, 10.0))
            .unwrap();
        editor.selection.clear();
        editor.selection.add(first);
        editor.selection.add(second);
        let error = editor.delete_overlapping_segments().unwrap_err();
        assert!(error.contains("No overlapping"), "{error}");
    }

    #[test]
    fn outline_turns_a_closed_stroke_into_two_contours() {
        let mut editor = editor();
        let rect = editor
            .add_rectangle(Point2::new(10.0, 10.0), Point2::new(20.0, 20.0))
            .unwrap();
        {
            let object = editor.document.object_mut(&rect).unwrap();
            object.style.stroke_width_mm = 2.0;
        }
        editor.selection.clear();
        editor.selection.add(rect);
        assert_eq!(editor.outline_selection().unwrap(), 2);
        assert_eq!(editor.document.objects.len(), 2);
        let mut bounds: Vec<_> = editor
            .document
            .objects
            .iter()
            .filter_map(Object::bounds)
            .map(|(min, max)| (min.x, min.y, max.x, max.y))
            .collect();
        bounds.sort_by(|left, right| left.0.partial_cmp(&right.0).unwrap());
        assert!((bounds[0].0 - 9.0).abs() < 1e-6);
        assert!((bounds[0].3 - 21.0).abs() < 1e-6);
        assert!((bounds[1].0 - 11.0).abs() < 1e-6);
        assert!((bounds[1].3 - 19.0).abs() < 1e-6);
        assert!(
            editor
                .document
                .objects
                .iter()
                .all(|object| { matches!(&object.shape, Shape::Path(path) if path.closed) })
        );
    }

    #[test]
    fn outline_turns_an_open_stroke_into_offsets() {
        let mut editor = editor();
        let line = editor
            .add_line(Point2::new(0.0, 0.0), Point2::new(10.0, 0.0))
            .unwrap();
        {
            let object = editor.document.object_mut(&line).unwrap();
            object.style.stroke_width_mm = 2.0;
        }
        editor.selection.clear();
        editor.selection.add(line);
        assert_eq!(editor.outline_selection().unwrap(), 2);
        assert_eq!(editor.document.objects.len(), 2);
        for object in &editor.document.objects {
            let path = object.world_path();
            assert!(!path.closed);
            let (min, max) = object.bounds().unwrap();
            assert!((max.y - min.y).abs() < 1e-6);
        }
        let mut heights: Vec<f64> = editor
            .document
            .objects
            .iter()
            .map(|object| object.bounds().unwrap().0.y)
            .collect();
        heights.sort_by(|left, right| left.partial_cmp(right).unwrap());
        assert!((heights[0] + 1.0).abs() < 1e-6);
        assert!((heights[1] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn outline_rejects_shapes_without_geometry() {
        let mut editor = editor();
        let line = editor
            .add_line(Point2::new(0.0, 0.0), Point2::new(10.0, 0.0))
            .unwrap();
        {
            let object = editor.document.object_mut(&line).unwrap();
            object.style.stroke_width_mm = 0.0;
        }
        editor.selection.clear();
        editor.selection.add(line);
        let error = editor.outline_selection().unwrap_err();
        assert!(error.contains("stroke width"), "{error}");
        editor.select_all();
        assert!(editor.outline_selection().is_err());
    }

    #[test]
    fn outline_is_undoable() {
        let mut editor = editor();
        let rect = editor
            .add_rectangle(Point2::new(0.0, 0.0), Point2::new(10.0, 10.0))
            .unwrap();
        editor.selection.clear();
        editor.selection.add(rect);
        editor.outline_selection().unwrap();
        assert_eq!(editor.document.objects.len(), 2);
        assert!(editor.undo().is_some());
        assert_eq!(editor.document.objects.len(), 1);
    }

    #[test]
    fn join_paths_welds_the_nearest_ends() {
        let mut editor = editor();
        let first = editor
            .add_line(Point2::new(0.0, 0.0), Point2::new(10.0, 0.0))
            .unwrap();
        let second = editor
            .add_line(Point2::new(10.0, 0.0), Point2::new(10.0, 10.0))
            .unwrap();
        editor.selection.clear();
        editor.selection.add(first);
        editor.selection.add(second);
        let joined = editor.join_paths().unwrap();
        assert_eq!(editor.document.objects.len(), 1);
        let object = &editor.document.objects[0];
        assert_eq!(object.id, joined);
        assert_eq!(object.name, "Joined Path");
        assert!(editor.selection.contains(&joined));
        let path = object.world_path();
        assert!(!path.closed);
        // Two original lines already touch, so no extra segment is added.
        assert_eq!(path.segments.len(), 2);
        assert_eq!(path.start_point().unwrap(), Point2::new(0.0, 0.0));
        assert_eq!(path.last_point().unwrap(), Point2::new(10.0, 10.0));
    }

    #[test]
    fn join_paths_reverses_the_shorter_end() {
        let mut editor = editor();
        let first = editor
            .add_line(Point2::new(0.0, 0.0), Point2::new(10.0, 0.0))
            .unwrap();
        // Drawn from the far end back to the shared end.
        let second = editor
            .add_line(Point2::new(10.0, 10.0), Point2::new(10.0, 0.0))
            .unwrap();
        editor.selection.clear();
        editor.selection.add(first);
        editor.selection.add(second);
        editor.join_paths().unwrap();
        let path = editor.document.objects[0].world_path();
        assert_eq!(path.last_point().unwrap(), Point2::new(10.0, 10.0));
        assert_eq!(path.segments.len(), 2);
    }

    #[test]
    fn join_paths_bridges_a_gap() {
        let mut editor = editor();
        let first = editor
            .add_line(Point2::new(0.0, 0.0), Point2::new(10.0, 0.0))
            .unwrap();
        let second = editor
            .add_line(Point2::new(20.0, 0.0), Point2::new(30.0, 0.0))
            .unwrap();
        editor.selection.clear();
        editor.selection.add(first);
        editor.selection.add(second);
        editor.join_paths().unwrap();
        let path = editor.document.objects[0].world_path();
        assert_eq!(path.segments.len(), 3);
        assert_eq!(path.start_point().unwrap(), Point2::new(0.0, 0.0));
        assert_eq!(path.last_point().unwrap(), Point2::new(30.0, 0.0));
    }

    #[test]
    fn join_paths_rejects_invalid_selection() {
        let mut editor = editor();
        let line = editor
            .add_line(Point2::new(0.0, 0.0), Point2::new(10.0, 0.0))
            .unwrap();
        assert!(editor.join_paths().is_err());
        let rect = editor
            .add_rectangle(Point2::new(0.0, 0.0), Point2::new(4.0, 4.0))
            .unwrap();
        editor.selection.clear();
        editor.selection.add(line);
        editor.selection.add(rect);
        let error = editor.join_paths().unwrap_err();
        assert!(error.contains("not a path"), "{error}");
        assert_eq!(editor.document.objects.len(), 2);
    }

    #[test]
    fn join_paths_is_undoable() {
        let mut editor = editor();
        let first = editor
            .add_line(Point2::new(0.0, 0.0), Point2::new(10.0, 0.0))
            .unwrap();
        let second = editor
            .add_line(Point2::new(10.0, 0.0), Point2::new(10.0, 10.0))
            .unwrap();
        editor.selection.clear();
        editor.selection.add(first);
        editor.selection.add(second);
        editor.join_paths().unwrap();
        assert_eq!(editor.document.objects.len(), 1);
        assert!(editor.undo().is_some());
        assert_eq!(editor.document.objects.len(), 2);
    }

    #[test]
    fn close_paths_closes_only_open_paths() {
        let mut editor = editor();
        let open = editor
            .add_polyline(
                &[
                    Point2::new(0.0, 0.0),
                    Point2::new(10.0, 0.0),
                    Point2::new(10.0, 10.0),
                ],
                false,
            )
            .unwrap();
        let closed = editor
            .add_polyline(
                &[
                    Point2::new(20.0, 0.0),
                    Point2::new(30.0, 0.0),
                    Point2::new(30.0, 10.0),
                ],
                true,
            )
            .unwrap();
        editor.select_all();
        assert_eq!(editor.close_paths().unwrap(), 1);
        let is_closed = |id: &str| {
            let object = editor
                .document
                .objects
                .iter()
                .find(|object| object.id == id)
                .unwrap();
            matches!(&object.shape, Shape::Path(path) if path.closed)
        };
        assert!(is_closed(&open));
        assert!(is_closed(&closed));
        assert!(editor.undo().is_some());
    }

    #[test]
    fn close_paths_reports_nothing_to_close() {
        let mut editor = editor();
        let rect = editor
            .add_rectangle(Point2::new(0.0, 0.0), Point2::new(4.0, 4.0))
            .unwrap();
        editor.selection.clear();
        editor.selection.add(rect);
        let error = editor.close_paths().unwrap_err();
        assert!(error.contains("No open paths"), "{error}");
    }

    #[test]
    fn copy_cut_and_paste_round_trip() {
        let mut editor = editor();
        let original = editor
            .add_rectangle(Point2::new(0.0, 0.0), Point2::new(10.0, 10.0))
            .unwrap();
        editor.select_all();
        let clipboard = editor.copy_selection();
        assert_eq!(clipboard.len(), 1);
        assert_eq!(editor.document.objects.len(), 1);
        assert_eq!(editor.paste_objects(&clipboard), 1);
        assert_eq!(editor.document.objects.len(), 2);
        let pasted = &editor.document.objects[1];
        assert_ne!(pasted.id, original);
        assert_eq!(pasted.style, editor.document.objects[0].style);
        assert_eq!(editor.selection.ids(), std::slice::from_ref(&pasted.id));

        editor.selection.clear();
        editor.selection.add(original.clone());
        let cut = editor.cut_selection();
        assert_eq!(cut.len(), 1);
        assert_eq!(editor.document.objects.len(), 1);
        assert!(editor.document.object(&original).is_none());
        assert!(editor.undo().is_some());
        assert_eq!(editor.document.objects.len(), 2);
    }

    #[test]
    fn paste_without_a_clipboard_does_nothing() {
        let mut editor = editor();
        assert_eq!(editor.paste_objects(&[]), 0);
        assert_eq!(editor.document.objects.len(), 0);
    }

    #[test]
    fn invert_selection_skips_locked_and_hidden_objects() {
        let mut editor = editor();
        let first = editor
            .add_rectangle(Point2::new(0.0, 0.0), Point2::new(4.0, 4.0))
            .unwrap();
        let second = editor
            .add_rectangle(Point2::new(10.0, 0.0), Point2::new(14.0, 4.0))
            .unwrap();
        let third = editor
            .add_rectangle(Point2::new(20.0, 0.0), Point2::new(24.0, 4.0))
            .unwrap();
        {
            let object = editor.document.object_mut(&third).unwrap();
            object.visible = false;
        }
        editor.selection.clear();
        editor.selection.add(first);
        assert_eq!(editor.invert_selection(), 1);
        assert!(editor.selection.contains(&second));
        assert!(!editor.selection.contains(&third));
    }

    #[test]
    fn locking_prevents_movement_until_unlocked() {
        let mut editor = editor();
        let id = editor
            .add_rectangle(Point2::new(0.0, 0.0), Point2::new(4.0, 4.0))
            .unwrap();
        editor.select_all();
        assert_eq!(editor.lock_selection(), 1);
        assert!(editor.document.object(&id).unwrap().locked);
        editor.nudge_selection(10.0, 0.0);
        assert!((editor.document.object(&id).unwrap().bounds().unwrap().0.x).abs() < 1e-9);
        assert_eq!(editor.unlock_selection(), 1);
        editor.nudge_selection(10.0, 0.0);
        assert!((editor.document.object(&id).unwrap().bounds().unwrap().0.x - 10.0).abs() < 1e-9);
        assert!(editor.undo().is_some());
    }

    #[test]
    fn rotation_can_use_the_page_centre() {
        let mut editor = editor();
        editor.add_rectangle(Point2::new(0.0, 0.0), Point2::new(10.0, 2.0));
        editor.select_all();
        editor.transform_center = TransformCenter::Page;
        assert!(editor.rotate_selection(std::f64::consts::FRAC_PI_2));
        let (min, max) = editor.selected_bounds().unwrap();
        assert!(min.x > 140.0);
        assert!(max.y < 0.0);
    }

    #[test]
    fn each_object_can_rotate_around_its_own_centre() {
        let mut editor = editor();
        let first = editor
            .add_rectangle(Point2::new(0.0, 0.0), Point2::new(10.0, 2.0))
            .unwrap();
        let second = editor
            .add_rectangle(Point2::new(20.0, 0.0), Point2::new(30.0, 2.0))
            .unwrap();
        editor.select_all();
        editor.transform_center = TransformCenter::Object;
        editor.rotate_selection(std::f64::consts::FRAC_PI_2);
        for id in [first, second] {
            let (min, max) = editor.document.object(&id).unwrap().bounds().unwrap();
            assert!((max.x - min.x - 2.0).abs() < 1e-6);
            assert!((max.y - min.y - 10.0).abs() < 1e-6);
        }
        let (first_min, first_max) = editor
            .document
            .object(&editor.document.objects[0].id)
            .unwrap()
            .bounds()
            .unwrap();
        let (second_min, _) = editor
            .document
            .object(&editor.document.objects[1].id)
            .unwrap()
            .bounds()
            .unwrap();
        assert!((f64::midpoint(first_min.x, first_max.x) - 5.0).abs() < 1e-6);
        assert!((second_min.x - 24.0).abs() < 1e-6);
    }

    #[test]
    fn setting_the_size_scales_around_the_centre() {
        let mut editor = editor();
        let id = editor
            .add_rectangle(Point2::new(10.0, 20.0), Point2::new(30.0, 30.0))
            .unwrap();
        editor.select_all();
        // Centre (20, 25): 20 x 10 becomes 40 x 20 around it.
        assert!(editor.set_selection_size(40.0, 20.0, false));
        let (min, max) = editor.document.object(&id).unwrap().bounds().unwrap();
        assert!((min.x - 0.0).abs() < 1e-6);
        assert!((min.y - 15.0).abs() < 1e-6);
        assert!((max.x - 40.0).abs() < 1e-6);
        assert!((max.y - 35.0).abs() < 1e-6);
    }

    #[test]
    fn preserving_proportions_uses_the_smaller_factor() {
        let mut editor = editor();
        editor.add_rectangle(Point2::new(0.0, 0.0), Point2::new(20.0, 10.0));
        editor.select_all();
        assert!(editor.set_selection_size(40.0, 100.0, true));
        let (min, max) = editor.selected_bounds().unwrap();
        assert!((max.x - min.x - 40.0).abs() < 1e-6);
        assert!((max.y - min.y - 20.0).abs() < 1e-6);
    }

    #[test]
    fn size_changes_are_validated_and_undoable() {
        let mut editor = editor();
        editor.add_rectangle(Point2::new(0.0, 0.0), Point2::new(10.0, 10.0));
        editor.select_all();
        assert!(!editor.set_selection_size(0.0, 10.0, false));
        assert!(!editor.set_selection_size(f64::NAN, 10.0, false));
        assert!(editor.set_selection_size(20.0, 20.0, false));
        assert!(editor.undo().is_some());
        let (min, max) = editor.selected_bounds().unwrap();
        assert!((max.x - min.x - 10.0).abs() < 1e-6);
        editor.selection.clear();
        assert!(!editor.set_selection_size(5.0, 5.0, false));
    }

    #[test]
    fn skew_shears_the_selection() {
        let mut editor = editor();
        editor.add_rectangle(Point2::new(0.0, 0.0), Point2::new(10.0, 10.0));
        editor.select_all();
        let (_, before) = editor.selected_bounds().unwrap();
        assert!(editor.skew_selection(45.0, 0.0));
        let (after_min, after_max) = editor.selected_bounds().unwrap();
        assert!(after_max.x - after_min.x > before.x - 1e-9);
        assert!(editor.undo().is_some());
        assert!(!editor.skew_selection(0.0, 0.0));
        assert!(!editor.skew_selection(f64::NAN, 0.0));
    }

    #[test]
    fn resetting_bakes_the_transform_into_the_geometry() {
        let mut editor = editor();
        let id = editor
            .add_rectangle(Point2::new(10.0, 10.0), Point2::new(20.0, 30.0))
            .unwrap();
        editor.select_all();
        editor.rotate_selection(std::f64::consts::FRAC_PI_4);
        let before: Vec<(f64, f64)> = editor
            .document
            .object(&id)
            .unwrap()
            .world_path()
            .flatten(0.1)
            .into_iter()
            .map(|point| (point.x, point.y))
            .collect();
        assert_eq!(editor.reset_transforms(), 1);
        let object = editor.document.object(&id).unwrap();
        assert_eq!(object.transform, Transform2::identity());
        let after: Vec<(f64, f64)> = object
            .world_path()
            .flatten(0.1)
            .into_iter()
            .map(|point| (point.x, point.y))
            .collect();
        assert_eq!(before.len(), after.len());
        for (a, b) in before.iter().zip(&after) {
            assert!((a.0 - b.0).abs() < 1e-6 && (a.1 - b.1).abs() < 1e-6);
        }
        assert!(editor.undo().is_some());
        assert_ne!(
            editor.document.object(&id).unwrap().transform,
            Transform2::identity()
        );
    }

    #[test]
    fn linear_repeat_steps_by_the_offset() {
        let mut editor = editor();
        editor.add_rectangle(Point2::new(0.0, 0.0), Point2::new(10.0, 10.0));
        let options = RepeatOptions {
            mode: RepeatMode::Linear,
            copies: 3,
            dx: 10.0,
            dy: 0.0,
            ..RepeatOptions::default()
        };
        assert_eq!(editor.repeat_selection(&options).unwrap(), 3);
        assert_eq!(editor.document.objects.len(), 4);
        let mut mins: Vec<f64> = editor
            .document
            .objects
            .iter()
            .filter_map(crate::document::Object::bounds)
            .map(|(min, _)| min.x)
            .collect();
        mins.sort_by(|left, right| left.partial_cmp(right).unwrap());
        assert_eq!(mins, vec![0.0, 10.0, 20.0, 30.0]);
        assert_eq!(editor.selection.len(), 3);
    }

    #[test]
    fn repeat_by_distance_is_a_normalised_step() {
        let mut editor = editor();
        editor.add_rectangle(Point2::new(0.0, 0.0), Point2::new(4.0, 4.0));
        // A 3-4-5 step repeated twice covers 10 mm.
        let options = RepeatOptions {
            mode: RepeatMode::Linear,
            copies: 2,
            dx: 6.0,
            dy: 8.0,
            ..RepeatOptions::default()
        };
        editor.repeat_selection(&options).unwrap();
        let mut mins: Vec<(f64, f64)> = editor
            .document
            .objects
            .iter()
            .filter_map(crate::document::Object::bounds)
            .map(|(min, _)| (min.x, min.y))
            .collect();
        mins.sort_by(|left, right| left.0.partial_cmp(&right.0).unwrap());
        assert_eq!(mins, vec![(0.0, 0.0), (6.0, 8.0), (12.0, 16.0)]);
    }

    #[test]
    fn linear_repeat_can_rotate_each_copy() {
        let mut editor = editor();
        editor.add_rectangle(Point2::new(0.0, 0.0), Point2::new(10.0, 2.0));
        let options = RepeatOptions {
            mode: RepeatMode::Linear,
            copies: 1,
            dx: 0.0,
            dy: 20.0,
            rotation_degrees: 90.0,
            ..RepeatOptions::default()
        };
        editor.repeat_selection(&options).unwrap();
        let copy = &editor.document.objects[1];
        let (min, max) = copy.bounds().unwrap();
        assert!((max.x - min.x - 2.0).abs() < 1e-6);
        assert!((max.y - min.y - 10.0).abs() < 1e-6);
    }

    #[test]
    fn circular_repeat_orbit() {
        let mut editor = editor();
        editor.add_rectangle(Point2::new(0.0, 0.0), Point2::new(10.0, 10.0));
        let options = RepeatOptions {
            mode: RepeatMode::Circular,
            copies: 3,
            radius: 10.0,
            rotate_copies: false,
            ..RepeatOptions::default()
        };
        editor.repeat_selection(&options).unwrap();
        let centres: Vec<(f64, f64)> = editor
            .document
            .objects
            .iter()
            .skip(1)
            .filter_map(crate::document::Object::bounds)
            .map(|(min, max)| (f64::midpoint(min.x, max.x), f64::midpoint(min.y, max.y)))
            .collect();
        assert_eq!(centres.len(), 3);
        let expected = [
            (f64::midpoint(5.0, 5.0), 15.0),
            (-5.0, f64::midpoint(5.0, 5.0)),
            (f64::midpoint(5.0, 5.0), -5.0),
        ];
        for (index, expected) in expected.iter().enumerate() {
            assert!((centres[index].0 - expected.0).abs() < 1e-6);
            assert!((centres[index].1 - expected.1).abs() < 1e-6);
        }
    }

    #[test]
    fn circular_repeat_can_rotate_copies() {
        let mut editor = editor();
        editor.add_rectangle(Point2::new(0.0, 0.0), Point2::new(10.0, 2.0));
        let options = RepeatOptions {
            mode: RepeatMode::Circular,
            copies: 2,
            radius: 0.0,
            rotate_copies: true,
            ..RepeatOptions::default()
        };
        editor.repeat_selection(&options).unwrap();
        let copy = &editor.document.objects[1];
        let (min, max) = copy.bounds().unwrap();
        // 120 degrees is not a multiple of 90, so the bounds are no longer
        // axis-aligned with the original 10 x 2 rectangle.
        assert!((max.x - min.x - 10.0).abs() > 1e-3 || (max.y - min.y - 2.0).abs() > 1e-3);
    }

    #[test]
    fn repeat_requires_a_selection_and_valid_parameters() {
        let mut editor = editor();
        editor.add_rectangle(Point2::new(0.0, 0.0), Point2::new(4.0, 4.0));
        editor.selection.clear();
        assert!(editor.repeat_selection(&RepeatOptions::default()).is_err());
        editor.select_all();
        for options in [
            RepeatOptions {
                copies: 0,
                ..RepeatOptions::default()
            },
            RepeatOptions {
                copies: 101,
                ..RepeatOptions::default()
            },
            RepeatOptions {
                copies: 2,
                dx: f64::NAN,
                ..RepeatOptions::default()
            },
            RepeatOptions {
                copies: 2,
                mode: RepeatMode::Circular,
                radius: -1.0,
                ..RepeatOptions::default()
            },
        ] {
            assert!(editor.repeat_selection(&options).is_err());
        }
        assert_eq!(editor.document.objects.len(), 1);
    }

    #[test]
    fn repeat_is_undoable_and_detaches_groups() {
        let mut editor = editor();
        editor.add_rectangle(Point2::new(0.0, 0.0), Point2::new(4.0, 4.0));
        editor.add_rectangle(Point2::new(20.0, 0.0), Point2::new(24.0, 4.0));
        editor.select_all();
        assert!(editor.group_selection());
        assert!(editor.document.objects[0].group_id.is_some());
        let options = RepeatOptions {
            copies: 1,
            dx: 10.0,
            dy: 0.0,
            ..RepeatOptions::default()
        };
        editor.repeat_selection(&options).unwrap();
        assert_eq!(editor.document.objects.len(), 4);
        assert!(
            editor.document.objects[2..]
                .iter()
                .all(|object| object.group_id.is_none())
        );
        assert!(editor.undo().is_some());
        assert_eq!(editor.document.objects.len(), 2);
    }

    #[test]
    fn text_converts_to_outline_paths() {
        let mut editor = editor();
        let id = editor.add_text(Point2::new(10.0, 20.0)).unwrap();
        if let Some(crate::document::Object {
            shape: Shape::Text(text),
            ..
        }) = editor.document.object_mut(&id)
        {
            text.content = "AB".to_string();
            text.size = 10.0;
        }
        let outlines = editor.convert_text_to_outlines(&id).unwrap();
        assert!(outlines.len() >= 2);
        assert!(editor.document.object(&id).is_none());
        assert_eq!(editor.document.objects.len(), outlines.len());
        assert_eq!(editor.selection.len(), outlines.len());
        for outline in &editor.document.objects {
            assert!(matches!(&outline.shape, Shape::Path(path) if path.closed));
            let (min, max) = outline.bounds().unwrap();
            assert!(max.x > min.x && max.y > min.y);
        }
        let min = editor
            .document
            .objects
            .iter()
            .filter_map(crate::document::Object::bounds)
            .map(|(min, _)| min.x)
            .fold(f64::INFINITY, f64::min);
        assert!((9.0..=11.0).contains(&min));
        assert!(editor.undo().is_some());
        assert!(editor.document.object(&id).is_some());
    }

    #[test]
    fn text_outlines_follow_the_object_transform() {
        let mut editor = editor();
        let id = editor.add_text(Point2::new(0.0, 0.0)).unwrap();
        if let Some(object) = editor.document.object_mut(&id) {
            object.transform = Transform2::translate(30.0, 40.0);
            if let Shape::Text(text) = &mut object.shape {
                text.content = "I".to_string();
            }
        }
        let outlines = editor.convert_text_to_outlines(&id).unwrap();
        let min = futures_min_x(&editor, &outlines);
        assert!((30.0..32.0).contains(&min));
    }

    fn futures_min_x(editor: &EditorState, ids: &[String]) -> f64 {
        ids.iter()
            .filter_map(|id| editor.document.object(id))
            .filter_map(crate::document::Object::bounds)
            .map(|(min, _)| min.x)
            .fold(f64::INFINITY, f64::min)
    }

    #[test]
    fn text_conversion_rejects_other_shapes() {
        let mut editor = editor();
        let rect = editor
            .add_rectangle(Point2::new(0.0, 0.0), Point2::new(4.0, 4.0))
            .unwrap();
        assert!(editor.convert_text_to_outlines(&rect).is_err());
        assert!(editor.convert_text_to_outlines("missing").is_err());
    }

    #[test]
    fn text_follows_a_horizontal_path() {
        let mut editor = editor();
        let text = editor.add_text(Point2::new(0.0, 0.0)).unwrap();
        if let Some(crate::document::Object {
            shape: Shape::Text(content),
            ..
        }) = editor.document.object_mut(&text)
        {
            content.content = "AB".to_string();
        }
        let path = editor
            .add_line(Point2::new(0.0, 0.0), Point2::new(40.0, 0.0))
            .unwrap();
        let outlines = editor.fit_text_to_path(&text, &path).unwrap();
        assert!(outlines.len() >= 2);
        assert!(editor.document.object(&text).is_none());
        assert!(editor.document.object(&path).is_some());
        let (min, max) = editor.selected_bounds().unwrap();
        assert!(min.x >= -1.0 && max.x <= 41.0);
        assert!(min.y >= -0.5 && max.y <= 11.0);
    }

    #[test]
    fn text_rotates_with_a_vertical_path() {
        let mut editor = editor();
        let text = editor.add_text(Point2::new(0.0, 0.0)).unwrap();
        if let Some(crate::document::Object {
            shape: Shape::Text(content),
            ..
        }) = editor.document.object_mut(&text)
        {
            content.content = "AB".to_string();
        }
        let path = editor
            .add_line(Point2::new(0.0, 0.0), Point2::new(0.0, 40.0))
            .unwrap();
        editor.fit_text_to_path(&text, &path).unwrap();
        let (min, max) = editor.selected_bounds().unwrap();
        assert!(max.y - min.y > 8.0);
        assert!(max.x <= 1.0);
    }

    #[test]
    fn text_longer_than_the_path_is_scaled_down() {
        let mut editor = editor();
        let text = editor.add_text(Point2::new(0.0, 0.0)).unwrap();
        if let Some(crate::document::Object {
            shape: Shape::Text(content),
            ..
        }) = editor.document.object_mut(&text)
        {
            content.content = "ABCDEFGH".to_string();
        }
        let path = editor
            .add_line(Point2::new(0.0, 0.0), Point2::new(5.0, 0.0))
            .unwrap();
        editor.fit_text_to_path(&text, &path).unwrap();
        let (min, max) = editor.selected_bounds().unwrap();
        assert!(max.x - min.x <= 5.0 + 1e-6);
        assert!(max.x > 3.0);
    }

    #[test]
    fn fitting_rejects_invalid_pairs() {
        let mut editor = editor();
        let text = editor.add_text(Point2::new(0.0, 0.0)).unwrap();
        let rect = editor
            .add_rectangle(Point2::new(0.0, 0.0), Point2::new(4.0, 4.0))
            .unwrap();
        assert!(editor.fit_text_to_path(&rect, &text).is_err());
        assert!(editor.fit_text_to_path(&text, &text).is_err());
        assert!(editor.fit_text_to_path(&text, "missing").is_err());
        assert!(editor.fit_text_to_path("missing", &rect).is_err());
    }

    #[test]
    fn bezier_nodes_become_a_path() {
        use crate::geometry::path::Node;

        let mut editor = editor();
        let nodes = [
            Node {
                point: Point2::new(0.0, 0.0),
                handle_in: None,
                handle_out: Some(Point2::new(3.0, 3.0)),
            },
            Node {
                point: Point2::new(10.0, 0.0),
                handle_in: Some(Point2::new(7.0, 3.0)),
                handle_out: None,
            },
        ];
        let id = editor.add_bezier_nodes(&nodes, false).unwrap();
        let path = editor.document.object(&id).unwrap().world_path();
        assert!(!path.closed);
        assert!(matches!(
            path.segments[0],
            crate::geometry::path::Segment::Cubic(_)
        ));
        assert!(editor.add_bezier_nodes(&nodes[..1], false).is_none());

        let closed = editor
            .add_bezier_nodes(&[nodes[0], nodes[1]], true)
            .unwrap();
        assert!(editor.document.object(&closed).unwrap().world_path().closed);
    }

    #[test]
    fn an_image_can_be_relinked() {
        use crate::objects::{ImageCrop, ImageObject};

        let mut editor = editor();
        // Build an image object directly, since the editor has no image tool.
        let layer = editor.document.layers[0].id.clone();
        let mut image = ImageObject::new("old.png", 100, 50, 96.0);
        image.crop = Some(ImageCrop {
            x: 10,
            y: 10,
            width: 50,
            height: 30,
        });
        let object = Object::new(layer, Shape::Image(image));
        let image_id = object.id.clone();
        editor.document.add_object(object);

        assert!(
            editor
                .relink_image(&image_id, std::path::Path::new("new.png"), 40, 20)
                .is_ok()
        );
        let Shape::Image(image) = &editor.document.object(&image_id).unwrap().shape else {
            panic!("expected image");
        };
        assert_eq!(image.path, "new.png");
        assert_eq!(image.effective_size_px(), (40, 20));
        assert!(image.crop.is_none());
        assert!(editor.undo().is_some());
        let text = editor.add_text(Point2::new(0.0, 0.0)).unwrap();
        assert!(
            editor
                .relink_image(&text, std::path::Path::new("x.png"), 4, 4)
                .is_err()
        );
        assert!(
            editor
                .relink_image("missing", std::path::Path::new("x.png"), 4, 4)
                .is_err()
        );
    }

    #[test]
    fn zoom_uses_the_configured_step() {
        let mut editor = editor();
        editor.zoom_step = 2.0;
        editor.canvas.zoom = 1.0;
        editor.zoom_in();
        assert!((editor.canvas.zoom - 2.0).abs() < 1e-9);
        editor.zoom_in();
        assert!((editor.canvas.zoom - 4.0).abs() < 1e-9);
        editor.zoom_out();
        assert!((editor.canvas.zoom - 2.0).abs() < 1e-9);
        editor.canvas.zoom = crate::editor::canvas::MAX_ZOOM;
        editor.zoom_in();
        assert!((editor.canvas.zoom - crate::editor::canvas::MAX_ZOOM).abs() < 1e-9);
        editor.canvas.zoom = crate::editor::canvas::MIN_ZOOM;
        editor.zoom_out();
        assert!((editor.canvas.zoom - crate::editor::canvas::MIN_ZOOM).abs() < 1e-9);
    }

    #[test]
    fn guides_can_be_added_removed_and_snapped_to() {
        let mut editor = editor();
        let guide = editor.add_guide(true, 25.0).unwrap();
        assert_eq!(editor.document.guides.len(), 1);
        editor.snap_guides = true;
        editor.snap_tolerance_px = 3.0;
        assert_eq!(
            editor.snap(Point2::new(10.0, 24.5)),
            Point2::new(10.0, 25.0)
        );
        editor.snap_guides = false;
        assert_eq!(
            editor.snap(Point2::new(10.0, 24.5)),
            Point2::new(10.0, 24.5)
        );
        editor.guides_visible = false;
        editor.snap_guides = true;
        assert_eq!(
            editor.snap(Point2::new(10.0, 24.5)),
            Point2::new(10.0, 24.5)
        );

        editor.guides_visible = true;
        assert!(editor.remove_guide(&guide));
        assert_eq!(editor.document.guides.len(), 0);
        assert!(editor.undo().is_some());
        assert_eq!(editor.document.guides.len(), 1);
        assert!(!editor.remove_guide("missing"));
        assert!(editor.add_guide(false, f64::NAN).is_none());
    }

    #[test]
    fn text_is_inserted_at_the_click() {
        let mut editor = editor();
        let id = editor.add_text(Point2::new(12.0, 34.0)).unwrap();
        let object = editor.document.object(&id).unwrap();
        let crate::document::Shape::Text(text) = &object.shape else {
            panic!("expected text");
        };
        assert_eq!(text.content, "Text");
        assert!((text.x - 12.0).abs() < 1e-9);
        assert!((text.y - 34.0).abs() < 1e-9);
        assert!(editor.selection.contains(&id));
        let (min, max) = object.bounds().unwrap();
        assert!(max.x > min.x && max.y > min.y);
    }

    #[test]
    fn star_has_twice_as_many_nodes_as_tips() {
        let mut editor = editor();
        let id = editor
            .add_star(Point2::new(0.0, 0.0), Point2::new(10.0, 0.0), 5, 0.5)
            .unwrap();
        let object = editor.document.object(&id).unwrap();
        let path = object.world_path();
        assert!(path.closed);
        assert_eq!(path.nodes().len(), 10);
        let first = path.nodes()[0].point;
        assert!((first.x - 10.0).abs() < 1e-9);
        assert!(first.y.abs() < 1e-9);
        let inner = path.nodes()[1].point;
        assert!((inner.x * inner.x + inner.y * inner.y).sqrt() - 5.0 < 1e-9);
        let (min, max) = object.bounds().unwrap();
        assert!(min.x >= -10.0 && max.x <= 10.0);
    }

    #[test]
    fn star_rejects_invalid_parameters() {
        let mut editor = editor();
        assert!(
            editor
                .add_star(Point2::new(0.0, 0.0), Point2::new(10.0, 0.0), 2, 0.5)
                .is_none()
        );
        assert!(
            editor
                .add_star(Point2::new(0.0, 0.0), Point2::new(10.0, 0.0), 5, 0.0)
                .is_none()
        );
        assert!(
            editor
                .add_star(Point2::new(0.0, 0.0), Point2::new(0.0, 0.0), 5, 0.5)
                .is_none()
        );
        assert_eq!(editor.document.objects.len(), 0);
    }

    #[test]
    fn offset_expands_and_contracts_a_rectangle() {
        let mut editor = editor();
        let rect = editor
            .add_rectangle(Point2::new(0.0, 0.0), Point2::new(10.0, 10.0))
            .unwrap();
        assert_eq!(editor.offset_selection(2.0).unwrap(), 1);
        assert!((area_of(&editor, &rect) - 196.0).abs() < 1e-3);
        assert!(editor.undo().is_some());
        assert_eq!(editor.offset_selection(-2.0).unwrap(), 1);
        assert!((area_of(&editor, &rect) - 36.0).abs() < 1e-3);
    }

    #[test]
    fn offset_removes_a_collapsed_shape() {
        let mut editor = editor();
        let small = editor
            .add_rectangle(Point2::new(0.0, 0.0), Point2::new(2.0, 2.0))
            .unwrap();
        editor.selection.clear();
        editor.selection.add(small);
        let error = editor.offset_selection(-5.0).unwrap_err();
        assert!(error.contains("too large"));
        assert_eq!(editor.document.objects.len(), 1);
    }

    #[test]
    fn offset_requires_a_selection_and_a_distance() {
        let mut editor = editor();
        editor
            .add_rectangle(Point2::new(0.0, 0.0), Point2::new(4.0, 4.0))
            .unwrap();
        editor.selection.clear();
        assert!(editor.offset_selection(1.0).is_err());
        editor.select_all();
        assert!(editor.offset_selection(0.0).is_err());
        assert!(editor.offset_selection(f64::NAN).is_err());
    }

    #[test]
    fn offset_skips_open_paths() {
        let mut editor = editor();
        let line = editor
            .add_line(Point2::new(0.0, 0.0), Point2::new(10.0, 0.0))
            .unwrap();
        editor.selection.clear();
        editor.selection.add(line);
        let error = editor.offset_selection(1.0).unwrap_err();
        assert!(error.contains("closed"));
    }

    #[test]
    fn node_snapping_pulls_to_a_corner() {
        let mut editor = editor();
        editor
            .add_rectangle(Point2::new(0.0, 0.0), Point2::new(10.0, 10.0))
            .unwrap();
        editor.canvas.zoom = 1.0;
        editor.snap_nodes = true;
        editor.snap_tolerance_px = 8.0;
        let snapped = editor.snap(Point2::new(10.4, 9.6));
        assert_eq!(snapped, Point2::new(10.0, 10.0));
    }

    #[test]
    fn endpoint_snapping_uses_open_path_ends() {
        let mut editor = editor();
        editor
            .add_line(Point2::new(0.0, 0.0), Point2::new(20.0, 0.0))
            .unwrap();
        editor.snap_nodes = false;
        editor.snap_endpoints = true;
        editor.snap_tolerance_px = 8.0;
        assert_eq!(editor.snap(Point2::new(20.5, 0.4)), Point2::new(20.0, 0.0));
        // A point near the middle of the line is not an endpoint target.
        assert_eq!(editor.snap(Point2::new(10.0, 0.2)), Point2::new(10.0, 0.2));
    }

    #[test]
    fn centre_snapping_uses_object_bounds() {
        let mut editor = editor();
        editor
            .add_rectangle(Point2::new(10.0, 20.0), Point2::new(30.0, 40.0))
            .unwrap();
        editor.snap_centers = true;
        editor.snap_tolerance_px = 8.0;
        assert_eq!(
            editor.snap(Point2::new(20.4, 30.3)),
            Point2::new(20.0, 30.0)
        );
    }

    #[test]
    fn object_snapping_respects_the_pull_radius() {
        let mut editor = editor();
        editor
            .add_rectangle(Point2::new(0.0, 0.0), Point2::new(10.0, 10.0))
            .unwrap();
        editor.snap_nodes = true;
        editor.snap_tolerance_px = 4.0;
        // 4 px at 1 px/mm is 4 mm, so 5 mm away is outside the radius.
        assert_eq!(editor.object_snap(Point2::new(15.0, 15.0)), None);
        assert_eq!(
            editor.object_snap(Point2::new(12.0, 12.0)),
            Some(Point2::new(10.0, 10.0))
        );
    }

    #[test]
    fn object_snapping_wins_over_the_grid() {
        let mut editor = editor();
        editor
            .add_line(Point2::new(0.0, 0.0), Point2::new(10.3, 0.0))
            .unwrap();
        editor.snap_endpoints = true;
        editor.snap_enabled = true;
        editor.snap_step_mm = 1.0;
        editor.snap_tolerance_px = 8.0;
        // The grid would give (10, 0); the endpoint pulls to (10.3, 0).
        assert_eq!(editor.snap(Point2::new(10.4, 0.2)), Point2::new(10.3, 0.0));
    }

    #[test]
    fn snapping_is_off_by_default() {
        let editor = editor();
        assert!(!editor.snap_nodes);
        assert!(!editor.snap_endpoints);
        assert!(!editor.snap_centers);
        assert!(!editor.snap_midpoints);
        assert!(!editor.snap_intersections);
        assert!(!editor.snap_limits);
        assert_eq!(editor.object_snap(Point2::new(1.0, 1.0)), None);
    }

    #[test]
    fn midpoint_snapping_uses_the_segment_centre() {
        let mut editor = editor();
        editor
            .add_line(Point2::new(0.0, 0.0), Point2::new(10.0, 0.0))
            .unwrap();
        editor.snap_midpoints = true;
        assert_eq!(editor.snap(Point2::new(5.4, 0.3)), Point2::new(5.0, 0.0));
    }

    #[test]
    fn limit_snapping_uses_bounds_corners_and_edges() {
        let mut editor = editor();
        editor
            .add_rectangle(Point2::new(0.0, 0.0), Point2::new(10.0, 20.0))
            .unwrap();
        editor.snap_limits = true;
        assert_eq!(editor.snap(Point2::new(0.3, 19.6)), Point2::new(0.0, 20.0));
        assert_eq!(editor.snap(Point2::new(5.4, 20.3)), Point2::new(5.0, 20.0));
    }

    #[test]
    fn intersection_snapping_finds_crossings() {
        let mut editor = editor();
        editor
            .add_line(Point2::new(0.0, 0.0), Point2::new(10.0, 10.0))
            .unwrap();
        editor
            .add_line(Point2::new(0.0, 10.0), Point2::new(10.0, 0.0))
            .unwrap();
        editor.snap_intersections = true;
        editor.snap_tolerance_px = 3.0;
        assert_eq!(editor.snap(Point2::new(5.3, 4.7)), Point2::new(5.0, 5.0));
        assert_eq!(editor.snap(Point2::new(2.0, 8.0)), Point2::new(2.0, 8.0));
    }

    #[test]
    fn snapping_rounds_points() {
        let mut editor = editor();
        editor.snap_enabled = true;
        editor.snap_step_mm = 5.0;
        let snapped = editor.snap(Point2::new(12.0, 13.0));
        assert!((snapped.x - 10.0).abs() < 1e-9);
        assert!((snapped.y - 15.0).abs() < 1e-9);
    }

    #[test]
    fn undo_and_redo_report_action_names() {
        let mut editor = editor();
        editor
            .add_rectangle(Point2::new(0.0, 0.0), Point2::new(10.0, 10.0))
            .unwrap();
        editor.record_action("Move");
        editor.move_selection(2.0, 0.0);

        assert_eq!(editor.history.undo_label(), Some("Move"));
        assert_eq!(editor.undo().as_deref(), Some("Move"));
        assert_eq!(editor.history.redo_label(), Some("Move"));
        assert_eq!(editor.redo().as_deref(), Some("Move"));
        assert_eq!(editor.undo().as_deref(), Some("Move"));
        assert_eq!(editor.undo().as_deref(), Some("Draw Rectangle"));
    }
}
