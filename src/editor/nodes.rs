//! Node editing (PLAN.md §6.3, §6.4).
//!
//! Editing works on one path object at a time. Entering the mode bakes the
//! object transform into the path, so node positions are document millimetres
//! and every edit is a plain path change that can be undone.

use crate::document::Object;
use crate::document::shape::Shape;
use crate::geometry::{Path, Point2, Transform2};

use super::state::EditorState;

/// Samples per segment when mapping a click back to a parameter.
const SEGMENT_SAMPLES: usize = 64;

impl EditorState {
    /// Enter node editing for one path object, baking its transform.
    pub fn begin_node_edit(&mut self, object_id: &str) -> std::result::Result<(), String> {
        let index = self
            .document
            .objects
            .iter()
            .position(|object| object.id == object_id)
            .ok_or_else(|| format!("unknown object: {object_id}"))?;
        self.ensure_editable(index)?;
        let object = &self.document.objects[index];
        // Rectangles and ellipses are converted to paths on the way in, like
        // any other shape with an outline. Text and images have none.
        let world = object.world_path();
        if world.is_empty() {
            return Err(format!("'{}' has no geometry", object.name));
        }
        self.record();
        if let Some(object) = self.document.object_mut(object_id) {
            object.shape = Shape::Path(world);
            object.transform = Transform2::identity();
        }
        self.node_object = Some(object_id.to_string());
        self.node_selection.clear();
        Ok(())
    }

    /// Leave node editing and clear the node selection.
    pub fn end_node_edit(&mut self) {
        self.node_object = None;
        self.node_selection.clear();
    }

    pub fn is_editing_nodes(&self) -> bool {
        self.node_object.is_some()
    }

    /// The path being edited, if any.
    pub fn node_path(&self) -> Option<&Path> {
        let id = self.node_object.as_deref()?;
        match &self.document.object(id)?.shape {
            Shape::Path(path) => Some(path),
            _ => None,
        }
    }

    fn node_path_mut(&mut self) -> Option<&mut Path> {
        let id = self.node_object.clone()?;
        match &mut self.document.object_mut(&id)?.shape {
            Shape::Path(path) => Some(path),
            _ => None,
        }
    }

    /// Node positions in document millimetres.
    pub fn node_points(&self) -> Vec<Point2> {
        self.node_path()
            .map(|path| path.nodes().into_iter().map(|node| node.point).collect())
            .unwrap_or_default()
    }

    /// Handles of every node, in the same order as [`Self::node_points`].
    pub fn node_handles(&self) -> Vec<(Option<Point2>, Option<Point2>)> {
        self.node_path()
            .map(|path| {
                path.nodes()
                    .into_iter()
                    .map(|node| (node.handle_in, node.handle_out))
                    .collect()
            })
            .unwrap_or_default()
    }

    fn record_node_edit(&mut self) {
        self.record();
    }

    /// Nearest point on the edited path, as a segment index and parameter.
    ///
    /// Used to insert a node where the user double-clicked. The parameter is
    /// sampled, so it is accurate to roughly one percent of the segment.
    #[allow(clippy::cast_precision_loss)]
    pub fn nearest_segment_on_path(&self, point: Point2) -> Option<(usize, f64)> {
        use crate::geometry::path::Segment;

        let path = self.node_path()?;
        let mut best: Option<(usize, f64, f64)> = None;
        for (index, segment) in path.segments.iter().enumerate() {
            for step in 0..=SEGMENT_SAMPLES {
                let t = step as f64 / SEGMENT_SAMPLES as f64;
                let candidate = match segment {
                    Segment::Line { from, to } => {
                        Point2::new(from.x + (to.x - from.x) * t, from.y + (to.y - from.y) * t)
                    }
                    Segment::Cubic(cubic) => cubic.eval(t),
                };
                let distance = candidate.distance(point);
                if best.is_none_or(|(_, _, current)| distance < current) {
                    best = Some((index, t, distance));
                }
            }
        }
        best.map(|(index, t, _)| (index, t))
    }

    /// Select the nearest node within `tolerance`.
    ///
    /// Returns the node index. Without `additive` the previous selection is
    /// replaced, and clicking away from every node clears it.
    pub fn select_node_at(
        &mut self,
        point: Point2,
        tolerance: f64,
        additive: bool,
    ) -> Option<usize> {
        let nearest = self
            .node_points()
            .iter()
            .enumerate()
            .map(|(index, node)| (index, node.distance(point)))
            .filter(|(_, distance)| *distance <= tolerance)
            .min_by(|left, right| {
                left.1
                    .partial_cmp(&right.1)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        let Some((index, _)) = nearest else {
            if !additive {
                self.node_selection.clear();
            }
            return None;
        };
        if !additive {
            self.node_selection.clear();
        }
        if self.node_selection.contains(&index) {
            if additive {
                self.node_selection.retain(|selected| *selected != index);
            }
        } else {
            self.node_selection.push(index);
        }
        Some(index)
    }

    pub fn node_is_selected(&self, index: usize) -> bool {
        self.node_selection.contains(&index)
    }

    /// Move every selected node by `(dx, dy)` millimetres.
    ///
    /// Like [`EditorState::move_selection`], this does not record history: a
    /// drag records once when it starts. Use [`Self::nudge_selected_nodes`] for
    /// a discrete move.
    pub fn move_selected_nodes(&mut self, dx: f64, dy: f64) -> bool {
        if self.node_selection.is_empty() || (dx == 0.0 && dy == 0.0) {
            return false;
        }
        if self.node_path().is_none() {
            return false;
        }
        let indices = self.node_selection.clone();
        let mut moved = false;
        if let Some(path) = self.node_path_mut() {
            for index in indices {
                moved |= path.move_node(index, dx, dy);
            }
        }
        moved
    }

    /// Move the selected nodes and record one undo step.
    pub fn nudge_selected_nodes(&mut self, dx: f64, dy: f64) -> bool {
        if self.node_selection.is_empty() || (dx == 0.0 && dy == 0.0) {
            return false;
        }
        self.record();
        self.move_selected_nodes(dx, dy)
    }

    /// Position a single selected node exactly.
    pub fn set_selected_node_position(&mut self, point: Point2) -> bool {
        if self.node_selection.len() != 1 || self.node_path().is_none() {
            return false;
        }
        self.record_node_edit();
        let index = self.node_selection[0];
        self.node_path_mut()
            .is_some_and(|path| path.set_node_point(index, point))
    }

    /// Insert a node on a segment at parameter `t` and select it.
    pub fn insert_node_at(&mut self, segment: usize, t: f64) -> Option<usize> {
        self.node_path()?;
        self.record_node_edit();
        let index = self.node_path_mut()?.insert_node(segment, t)?;
        self.node_selection.clear();
        self.node_selection.push(index);
        Some(index)
    }

    /// Delete the selected nodes. Returns how many were removed.
    ///
    /// Removing every node of a path removes the object and ends node editing.
    pub fn delete_selected_nodes(&mut self) -> usize {
        if self.node_selection.is_empty() {
            return 0;
        }
        self.record_node_edit();
        let mut indices = self.node_selection.clone();
        indices.sort_unstable();
        indices.reverse();
        let mut removed = 0;
        if let Some(path) = self.node_path_mut() {
            for index in indices {
                if path.remove_node(index) {
                    removed += 1;
                }
            }
        }
        if removed > 0 {
            self.node_selection.clear();
            let empty = self.node_path().is_none_or(|path| path.nodes().len() < 2);
            if empty {
                if let Some(id) = self.node_object.clone() {
                    self.document.remove_object(&id);
                    self.selection.remove(&id);
                }
                self.end_node_edit();
            }
        }
        removed
    }

    /// Convert the selected nodes to smooth curves or corners.
    pub fn convert_selected_nodes(&mut self, smooth: bool) -> usize {
        if self.node_selection.is_empty() || self.node_path().is_none() {
            return 0;
        }
        self.record_node_edit();
        let indices = self.node_selection.clone();
        let mut converted = 0;
        if let Some(path) = self.node_path_mut() {
            for index in indices {
                if path.convert_node(index, smooth) {
                    converted += 1;
                }
            }
        }
        converted
    }

    /// Move one handle of a node to an absolute position.
    pub fn set_node_handle(&mut self, index: usize, outgoing: bool, point: Point2) -> bool {
        let Some(path) = self.node_path() else {
            return false;
        };
        let Some(node) = path.nodes().get(index).copied() else {
            return false;
        };
        // Dragging a handle records once when the drag starts.
        let (handle_in, handle_out) = if outgoing {
            (node.handle_in, Some(point))
        } else {
            (Some(point), node.handle_out)
        };
        self.node_path_mut()
            .is_some_and(|path| path.set_node_handles(index, handle_in, handle_out))
    }

    /// Clear one handle, making the segment on that side a straight line.
    ///
    /// Both ends of a segment are cleared together, because a segment with a
    /// single remaining handle would still be a curve.
    pub fn clear_node_handle(&mut self, index: usize, outgoing: bool) -> bool {
        let Some(path) = self.node_path() else {
            return false;
        };
        let nodes = path.nodes();
        if index >= nodes.len() || nodes.len() < 2 {
            return false;
        }
        let segment = if outgoing {
            index
        } else if index > 0 {
            index - 1
        } else {
            nodes.len() - 1
        };
        self.record_node_edit();
        self.node_path_mut()
            .is_some_and(|path| path.clear_segment(segment))
    }

    /// Open a closed path at the single selected node.
    pub fn open_path_at_selected_node(&mut self) -> bool {
        if self.node_selection.len() != 1 || self.node_path().is_none() {
            return false;
        }
        let index = self.node_selection[0];
        self.record_node_edit();
        let opened = self
            .node_path_mut()
            .is_some_and(|path| path.open_at_node(index));
        if opened {
            self.node_selection.clear();
            self.node_selection.push(0);
        }
        opened
    }

    /// Split an open path at the single selected node, creating a second object.
    ///
    /// Returns the new object id; the original keeps the first part.
    pub fn split_path_at_selected_node(&mut self) -> Option<String> {
        if self.node_selection.len() != 1 {
            return None;
        }
        let index = self.node_selection[0];
        let path = self.node_path()?.clone();
        let (left, right) = path.split_at_node(index)?;
        let id = self.node_object.clone()?;
        let template = self.document.object(&id)?.clone();
        let position = self
            .document
            .objects
            .iter()
            .position(|object| object.id == id)?;
        self.record_node_edit();
        if let Some(object) = self.document.object_mut(&id) {
            object.shape = Shape::Path(left);
        }
        let mut second = Object::new(template.layer_id.clone(), Shape::Path(right))
            .with_style(template.style.clone());
        second.name = format!("{} part 2", template.name);
        second.operation_id.clone_from(&template.operation_id);
        let new_id = second.id.clone();
        self.document.objects.insert(position + 1, second);
        self.node_selection.clear();
        Some(new_id)
    }

    /// Replace the edited path with a simplified polyline.
    pub fn simplify_selected_path(&mut self, tolerance: f64) -> std::result::Result<usize, String> {
        if !tolerance.is_finite() || tolerance <= 0.0 {
            return Err("Simplify tolerance must be positive".to_string());
        }
        let path = self
            .node_path()
            .ok_or_else(|| "No path is being edited".to_string())?
            .clone();
        let simplified = path.simplified(tolerance);
        if simplified.segments.len() >= path.segments.len() {
            return Err("The tolerance does not remove any node".to_string());
        }
        self.record_node_edit();
        let nodes = simplified.nodes().len();
        if let Some(path) = self.node_path_mut() {
            *path = simplified;
        }
        self.node_selection.clear();
        Ok(nodes)
    }
}

#[cfg(test)]
mod tests {
    use crate::document::Document;
    use crate::geometry::{Point2, Transform2};

    use super::super::state::EditorState;

    fn editor() -> EditorState {
        let mut document = Document::new("Nodes", 200.0, 100.0);
        document.layers[0].name = "Cut".to_string();
        EditorState::new(document)
    }

    fn open_polyline(editor: &mut EditorState) -> String {
        editor
            .add_polyline(
                &[
                    Point2::new(0.0, 0.0),
                    Point2::new(10.0, 0.0),
                    Point2::new(10.0, 10.0),
                ],
                false,
            )
            .unwrap()
    }

    #[test]
    fn entering_node_edit_bakes_the_transform() {
        let mut editor = editor();
        let rect = editor
            .add_rectangle(Point2::new(5.0, 5.0), Point2::new(15.0, 10.0))
            .unwrap();
        assert_eq!(
            editor.document.object(&rect).unwrap().transform,
            Transform2::translate(5.0, 5.0)
        );
        editor.begin_node_edit(&rect).unwrap();
        assert_eq!(
            editor.document.object(&rect).unwrap().transform,
            Transform2::identity()
        );
        let points = editor.node_points();
        assert_eq!(points.len(), 4);
        assert!(points.contains(&Point2::new(15.0, 10.0)));
        assert!(editor.undo());
        assert_eq!(
            editor.document.object(&rect).unwrap().transform,
            Transform2::translate(5.0, 5.0)
        );
    }

    #[test]
    fn node_edit_requires_a_path() {
        let mut editor = editor();
        let image = editor.add_line(Point2::new(0.0, 0.0), Point2::new(1.0, 0.0));
        assert!(image.is_some());
        let rect = editor
            .add_rectangle(Point2::new(0.0, 0.0), Point2::new(4.0, 4.0))
            .unwrap();
        // Entering node editing converts a primitive to a path; an unknown id is
        // always rejected.
        assert!(editor.begin_node_edit("missing").is_err());
        editor.begin_node_edit(&rect).unwrap();
        assert!(editor.is_editing_nodes());
        editor.end_node_edit();
        assert!(!editor.is_editing_nodes());
        assert_eq!(editor.node_points().len(), 0);
    }

    #[test]
    fn selecting_nodes_is_click_and_shift_click() {
        let mut editor = editor();
        let path = open_polyline(&mut editor);
        editor.begin_node_edit(&path).unwrap();
        assert_eq!(
            editor.select_node_at(Point2::new(0.2, 0.2), 1.0, false),
            Some(0)
        );
        assert_eq!(
            editor.select_node_at(Point2::new(10.2, 0.2), 1.0, true),
            Some(1)
        );
        assert_eq!(editor.node_selection, vec![0, 1]);
        // Shift-clicking a selected node removes it.
        editor.select_node_at(Point2::new(10.2, 0.2), 1.0, true);
        assert_eq!(editor.node_selection, vec![0]);
        // Clicking empty space clears the selection.
        editor.select_node_at(Point2::new(50.0, 50.0), 1.0, false);
        assert_eq!(editor.node_selection.len(), 0);
    }

    #[test]
    fn moving_nodes_moves_the_geometry() {
        let mut editor = editor();
        let path = open_polyline(&mut editor);
        editor.begin_node_edit(&path).unwrap();
        editor.select_node_at(Point2::new(10.0, 10.0), 0.5, false);
        assert!(editor.nudge_selected_nodes(5.0, -5.0));
        let points = editor.node_points();
        assert!(points.contains(&Point2::new(15.0, 5.0)));
        let (min, max) = editor.document.object(&path).unwrap().bounds().unwrap();
        assert!((max.x - 15.0).abs() < 1e-9);
        assert!((min.y).abs() < 1e-9);
        assert!(editor.undo());
        assert!(editor.node_points().contains(&Point2::new(10.0, 10.0)));
    }

    #[test]
    fn a_single_node_can_be_positioned_exactly() {
        let mut editor = editor();
        let path = open_polyline(&mut editor);
        editor.begin_node_edit(&path).unwrap();
        editor.select_node_at(Point2::new(0.0, 0.0), 0.5, false);
        assert!(editor.set_selected_node_position(Point2::new(2.5, 3.5)));
        assert!(editor.node_points().contains(&Point2::new(2.5, 3.5)));
        editor.select_node_at(Point2::new(10.0, 0.0), 0.5, true);
        assert!(!editor.set_selected_node_position(Point2::new(0.0, 0.0)));
    }

    #[test]
    fn nearest_segment_finds_the_click_parameter() {
        let mut editor = editor();
        let path = open_polyline(&mut editor);
        editor.begin_node_edit(&path).unwrap();
        let (segment, t) = editor
            .nearest_segment_on_path(Point2::new(5.0, 1.0))
            .unwrap();
        assert_eq!(segment, 0);
        assert!((t - 0.5).abs() < 0.02);
        let (segment, _) = editor
            .nearest_segment_on_path(Point2::new(10.0, 5.0))
            .unwrap();
        assert_eq!(segment, 1);
    }

    #[test]
    fn inserting_a_node_selects_it() {
        let mut editor = editor();
        let path = open_polyline(&mut editor);
        editor.begin_node_edit(&path).unwrap();
        let index = editor.insert_node_at(0, 0.5).unwrap();
        assert_eq!(index, 1);
        assert_eq!(editor.node_selection, vec![1]);
        assert_eq!(editor.node_points().len(), 4);
        assert!(editor.node_points().contains(&Point2::new(5.0, 0.0)));
    }

    #[test]
    fn deleting_nodes_keeps_the_path_usable() {
        let mut editor = editor();
        let path = open_polyline(&mut editor);
        editor.begin_node_edit(&path).unwrap();
        editor.select_node_at(Point2::new(10.0, 0.0), 0.5, false);
        assert_eq!(editor.delete_selected_nodes(), 1);
        assert_eq!(editor.node_points().len(), 2);
        // Two nodes cannot lose one more.
        editor.select_node_at(Point2::new(0.0, 0.0), 0.5, false);
        assert_eq!(editor.delete_selected_nodes(), 0);
        assert_eq!(editor.document.objects.len(), 1);
    }

    #[test]
    fn converting_nodes_changes_the_segments() {
        let mut editor = editor();
        let path = open_polyline(&mut editor);
        editor.begin_node_edit(&path).unwrap();
        editor.select_node_at(Point2::new(10.0, 0.0), 0.5, false);
        assert_eq!(editor.convert_selected_nodes(true), 1);
        let shape = &editor.document.object(&path).unwrap().shape;
        assert!(matches!(shape, crate::document::Shape::Path(path) if !path.closed));
        assert!(editor.node_handles()[1].0.is_some());
        assert!(editor.node_handles()[1].1.is_some());
        assert_eq!(editor.convert_selected_nodes(false), 1);
        assert!(editor.node_handles()[1].0.is_none());
        assert!(editor.node_handles()[1].1.is_none());
    }

    #[test]
    fn handles_can_be_dragged_and_cleared() {
        let mut editor = editor();
        let path = open_polyline(&mut editor);
        editor.begin_node_edit(&path).unwrap();
        editor.select_node_at(Point2::new(10.0, 0.0), 0.5, false);
        editor.convert_selected_nodes(true);
        assert!(editor.set_node_handle(1, true, Point2::new(14.0, 2.0)));
        assert_eq!(editor.node_handles()[1].1, Some(Point2::new(14.0, 2.0)));
        assert!(editor.clear_node_handle(1, true));
        assert!(editor.node_handles()[1].1.is_none());
        assert!(!editor.set_node_handle(99, true, Point2::new(0.0, 0.0)));
    }

    #[test]
    fn opening_a_closed_path_at_a_node() {
        let mut editor = editor();
        let rect = editor
            .add_rectangle(Point2::new(0.0, 0.0), Point2::new(10.0, 10.0))
            .unwrap();
        editor.begin_node_edit(&rect).unwrap();
        editor.select_node_at(Point2::new(0.0, 0.0), 0.5, false);
        assert!(editor.open_path_at_selected_node());
        let path = editor.document.object(&rect).unwrap().world_path();
        assert!(!path.closed);
        assert_eq!(editor.node_points().len(), 5);
    }

    #[test]
    fn splitting_an_open_path_creates_a_second_object() {
        let mut editor = editor();
        let line = editor
            .add_polyline(
                &[
                    Point2::new(0.0, 0.0),
                    Point2::new(5.0, 5.0),
                    Point2::new(10.0, 0.0),
                ],
                false,
            )
            .unwrap();
        editor.begin_node_edit(&line).unwrap();
        editor.select_node_at(Point2::new(5.0, 5.0), 0.5, false);
        let second = editor.split_path_at_selected_node().unwrap();
        assert_eq!(editor.document.objects.len(), 2);
        assert_eq!(
            editor
                .document
                .object(&line)
                .unwrap()
                .world_path()
                .last_point(),
            Some(Point2::new(5.0, 5.0))
        );
        assert_eq!(
            editor
                .document
                .object(&second)
                .unwrap()
                .world_path()
                .start_point(),
            Some(Point2::new(5.0, 5.0))
        );
    }

    #[test]
    fn simplifying_reduces_the_node_count() {
        let mut editor = editor();
        let path = editor
            .add_polyline(
                &[
                    Point2::new(0.0, 0.0),
                    Point2::new(2.0, 0.01),
                    Point2::new(4.0, 0.0),
                    Point2::new(6.0, 0.0),
                ],
                false,
            )
            .unwrap();
        editor.begin_node_edit(&path).unwrap();
        let nodes = editor.simplify_selected_path(0.1).unwrap();
        assert_eq!(nodes, 2);
        assert_eq!(editor.node_points().len(), 2);
        assert!(editor.simplify_selected_path(0.0).is_err());
        assert_eq!(editor.node_points().len(), 2);
    }
}
