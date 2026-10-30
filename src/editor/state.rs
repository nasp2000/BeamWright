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
                self.selection.toggle(id.clone());
                Some(id)
            }
            Some(id) => {
                self.selection.clear();
                self.selection.add(id.clone());
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
    fn snapping_rounds_points() {
        let mut editor = editor();
        editor.snap_enabled = true;
        editor.snap_step_mm = 5.0;
        let snapped = editor.snap(Point2::new(12.0, 13.0));
        assert!((snapped.x - 10.0).abs() < 1e-9);
        assert!((snapped.y - 15.0).abs() < 1e-9);
    }
}
