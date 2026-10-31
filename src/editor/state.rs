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

/// Testable editor state: document, view and interaction state without egui.
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
        }
    }

    pub fn record(&mut self) {
        self.history.record(&self.document);
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

    pub fn snap(&self, point: Point2) -> Point2 {
        if !self.snap_enabled || self.snap_step_mm <= 0.0 {
            return point;
        }
        Point2::new(
            (point.x / self.snap_step_mm).round() * self.snap_step_mm,
            (point.y / self.snap_step_mm).round() * self.snap_step_mm,
        )
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
        self.record();
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
    fn selected_indices(&self) -> Vec<usize> {
        self.document
            .objects
            .iter()
            .enumerate()
            .filter(|(_, object)| self.selection.contains(&object.id))
            .map(|(index, _)| index)
            .collect()
    }

    fn ensure_editable(&self, index: usize) -> std::result::Result<(), String> {
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

        self.record();
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

        self.record();
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
        self.record();
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

        self.record();
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
        match hit.clone() {
            Some(id) if additive => {
                for member in self.group_members(&id).unwrap_or_else(|| vec![id.clone()]) {
                    self.selection.toggle(member);
                }
                Some(id)
            }
            Some(id) => {
                self.selection.clear();
                for member in self.group_members(&id).unwrap_or_else(|| vec![id.clone()]) {
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
        self.record();
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
        self.record();
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
    fn transform_selection(&mut self, transform: Transform2) -> bool {
        if self.selection.is_empty() {
            return false;
        }
        let Some((min, max)) = self.selected_bounds() else {
            return false;
        };
        let center = Point2::new(f64::midpoint(min.x, max.x), f64::midpoint(min.y, max.y));
        let moved = Transform2::translate(-center.x, -center.y);
        let back = Transform2::translate(center.x, center.y);
        let combined = back.compose(&transform.compose(&moved));
        self.record();
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
            if let Some(object) = self.document.object_mut(&id) {
                if object.locked {
                    continue;
                }
                object.transform = combined.compose(&object.transform);
                changed = true;
            }
        }
        changed
    }

    /// Rotate the selection by `radians` around its centre.
    pub fn rotate_selection(&mut self, radians: f64) -> bool {
        self.transform_selection(Transform2::rotate(radians))
    }

    /// Scale the selection around its centre.
    pub fn scale_selection(&mut self, sx: f64, sy: f64) -> bool {
        if sx <= 0.0 || sy <= 0.0 {
            return false;
        }
        self.transform_selection(Transform2::scale(sx, sy))
    }

    /// Mirror the selection around its centre.
    pub fn flip_selection(&mut self, horizontal: bool) -> bool {
        let transform = if horizontal {
            Transform2::scale(-1.0, 1.0)
        } else {
            Transform2::scale(1.0, -1.0)
        };
        self.transform_selection(transform)
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
        self.record();
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
        self.record();
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
        self.record();
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
        self.record();
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

    /// Duplicate the selection in a rectangular grid.
    ///
    /// Returns the number of new objects. The original objects stay in place;
    /// the copies become the selection.
    pub fn duplicate_grid(&mut self, columns: usize, rows: usize, dx: f64, dy: f64) -> usize {
        if self.selection.is_empty()
            || !(1..=100).contains(&columns)
            || !(1..=100).contains(&rows)
            || !dx.is_finite()
            || !dy.is_finite()
        {
            return 0;
        }
        self.record();
        let originals: Vec<Object> = self
            .selection
            .ids()
            .iter()
            .filter_map(|id| self.document.object(id).cloned())
            .collect();
        self.selection.clear();
        let mut created = 0;
        for row in 0..rows {
            for column in 0..columns {
                if row == 0 && column == 0 {
                    continue;
                }
                #[allow(clippy::cast_precision_loss)]
                let offset = (column as f64 * dx, row as f64 * dy);
                for original in &originals {
                    let mut copy = original.clone();
                    copy.id = uuid::Uuid::new_v4().to_string();
                    copy.translate(offset.0, offset.1);
                    self.selection.add(copy.id.clone());
                    self.document.add_object(copy);
                    created += 1;
                }
            }
        }
        created
    }

    pub fn nudge_selection(&mut self, dx: f64, dy: f64) {
        if self.selection.is_empty() {
            return;
        }
        self.record();
        self.move_selection(dx, dy);
    }

    pub fn undo(&mut self) -> bool {
        if self.history.undo(&mut self.document) {
            self.prune_selection();
            true
        } else {
            false
        }
    }

    pub fn redo(&mut self) -> bool {
        if self.history.redo(&mut self.document) {
            self.prune_selection();
            true
        } else {
            false
        }
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
        self.record();
        let operation = crate::operations::Operation::new(name, op_type);
        let id = operation.id.clone();
        self.document.operations.push(operation);
        id
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
        self.record();
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
        self.record();
        self.document.operations.swap(index, target);
        true
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
        self.record();
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
    pub fn apply_operation_profile(
        &mut self,
        profile: &crate::profiles::OperationProfile,
    ) -> usize {
        if self.selection.is_empty() {
            return 0;
        }
        self.record();
        let operation = crate::operations::Operation {
            id: uuid::Uuid::new_v4().to_string(),
            name: profile.name.clone(),
            op_type: profile.op_type,
            params: profile.params.clone(),
            enabled: true,
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
        self.record();
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
        self.record();
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
        assert!(editor.undo());
        assert!(editor.document.objects.is_empty());
        assert!(editor.redo());
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
        assert!(editor.undo());
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
        assert!(editor.undo());
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
        assert!(editor.undo());
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
        assert!(editor.undo());
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
        assert!(editor.undo());
        assert!(
            editor.document.object(&first).unwrap().group_id.is_some(),
            "undo restores the grouped state"
        );
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
        assert!(editor.undo());
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
        assert!(editor.undo());
        assert_eq!(editor.document.objects.len(), 2);
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
        assert!(editor.undo());
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
        assert!(editor.undo());
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
    fn offset_expands_and_contracts_a_rectangle() {
        let mut editor = editor();
        let rect = editor
            .add_rectangle(Point2::new(0.0, 0.0), Point2::new(10.0, 10.0))
            .unwrap();
        assert_eq!(editor.offset_selection(2.0).unwrap(), 1);
        assert!((area_of(&editor, &rect) - 196.0).abs() < 1e-3);
        assert!(editor.undo());
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
    fn snapping_rounds_points() {
        let mut editor = editor();
        editor.snap_enabled = true;
        editor.snap_step_mm = 5.0;
        let snapped = editor.snap(Point2::new(12.0, 13.0));
        assert!((snapped.x - 10.0).abs() < 1e-9);
        assert!((snapped.y - 15.0).abs() < 1e-9);
    }
}
