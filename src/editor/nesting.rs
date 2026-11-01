//! Nesting selected parts into the work area.
//!
//! The proposal is plain data: it never touches the document, so the interface
//! can show it and the user decides whether to apply it.

use crate::document::Object;
use crate::geometry::nesting::{NestOptions, nest};
use crate::geometry::{Point2, Transform2};

use super::state::EditorState;

/// Where one part goes in the proposal.
#[derive(Debug, Clone, PartialEq)]
pub struct NestPlacement {
    pub object_id: String,
    /// Bottom-left corner of the part after placement, in document millimetres.
    pub min: Point2,
    /// Width of the placed part (after rotation).
    pub width: f64,
    /// Height of the placed part (after rotation).
    pub height: f64,
    pub rotated: bool,
}

/// A nesting proposal that has not been applied yet.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct NestProposal {
    pub placements: Vec<NestPlacement>,
    /// Parts that do not fit in the available area.
    pub unplaced: Vec<String>,
    /// Area of the placed parts, in square millimetres.
    pub used_area: f64,
    /// Area available inside the margins, in square millimetres.
    pub available_area: f64,
}

impl NestProposal {
    /// Fraction of the available area used by the placed parts (0..1).
    pub fn utilization(&self) -> f64 {
        if self.available_area <= 0.0 {
            0.0
        } else {
            self.used_area / self.available_area
        }
    }
}

impl EditorState {
    /// Propose positions for the selected parts inside the work area.
    ///
    /// The document is not modified. Parts that do not fit are listed in
    /// `unplaced` instead of being moved somewhere invalid.
    pub fn nest_selection(
        &self,
        spacing: f64,
        margin: f64,
        allow_rotation: bool,
    ) -> std::result::Result<NestProposal, String> {
        let ids: Vec<String> = self
            .document
            .objects
            .iter()
            .filter(|object| self.selection.contains(&object.id))
            .map(|object| object.id.clone())
            .collect();
        if ids.is_empty() {
            return Err("Select the parts to nest".to_string());
        }
        if !spacing.is_finite() || spacing < 0.0 {
            return Err("Spacing must be zero or positive".to_string());
        }
        if !margin.is_finite() || margin < 0.0 {
            return Err("Margin must be zero or positive".to_string());
        }
        let items: Vec<crate::geometry::nesting::RectItem> = ids
            .iter()
            .filter_map(|id| self.document.object(id))
            .filter_map(Object::bounds)
            .map(|(min, max)| crate::geometry::nesting::RectItem {
                w: max.x - min.x,
                h: max.y - min.y,
            })
            .collect();
        if items.len() != ids.len() {
            return Err("Every selected part must have geometry".to_string());
        }
        let result = nest(
            &items,
            self.document.width_mm,
            self.document.height_mm,
            NestOptions {
                spacing,
                margin,
                allow_rotation,
            },
        );
        let available_area = (self.document.width_mm - 2.0 * margin).max(0.0)
            * (self.document.height_mm - 2.0 * margin).max(0.0);
        let mut proposal = NestProposal {
            placements: Vec::with_capacity(result.placements.len()),
            unplaced: Vec::new(),
            used_area: 0.0,
            available_area,
        };
        for placement in &result.placements {
            let item = &items[placement.item];
            let (width, height) = if placement.rotated {
                (item.h, item.w)
            } else {
                (item.w, item.h)
            };
            proposal.used_area += width * height;
            proposal.placements.push(NestPlacement {
                object_id: ids[placement.item].clone(),
                min: Point2::new(placement.x, placement.y),
                width,
                height,
                rotated: placement.rotated,
            });
        }
        proposal.unplaced = result
            .unplaced
            .iter()
            .map(|index| ids[*index].clone())
            .collect();
        Ok(proposal)
    }

    /// Move the parts of a proposal into place, recording one undo step.
    ///
    /// Returns how many objects moved. Parts whose identifiers no longer exist
    /// are skipped.
    pub fn apply_nest(&mut self, proposal: &NestProposal) -> usize {
        if proposal.placements.is_empty() {
            return 0;
        }
        self.record_action("Nest selection");
        let mut moved = 0;
        for placement in &proposal.placements {
            if let Some(object) = self.document.object_mut(&placement.object_id) {
                if placement.rotated {
                    object.transform =
                        rotation_around_bounds(object, 90.0).compose(&object.transform);
                }
                if let Some((min, _)) = object.bounds() {
                    object.translate(placement.min.x - min.x, placement.min.y - min.y);
                }
                moved += 1;
            }
        }
        moved
    }
}

/// A 90-degree counter-clockwise rotation around the object's bounds centre.
fn rotation_around_bounds(object: &Object, degrees: f64) -> Transform2 {
    let Some((min, max)) = object.bounds() else {
        return Transform2::identity();
    };
    let center = Point2::new(f64::midpoint(min.x, max.x), f64::midpoint(min.y, max.y));
    let back = Transform2::translate(center.x, center.y);
    let forward = Transform2::translate(-center.x, -center.y);
    back.compose(&Transform2::rotate(degrees.to_radians()).compose(&forward))
}

#[cfg(test)]
mod tests {
    use crate::document::Document;
    use crate::geometry::Point2;

    use super::super::state::EditorState;
    use super::NestProposal;

    fn editor() -> EditorState {
        let mut document = Document::new("Nest", 100.0, 100.0);
        document.layers[0].name = "Cut".to_string();
        EditorState::new(document)
    }

    fn two_parts(editor: &mut EditorState) -> (String, String) {
        let first = editor
            .add_rectangle(Point2::new(10.0, 10.0), Point2::new(30.0, 25.0))
            .unwrap();
        let second = editor
            .add_rectangle(Point2::new(40.0, 40.0), Point2::new(50.0, 60.0))
            .unwrap();
        editor.select_all();
        (first, second)
    }

    #[test]
    fn proposal_does_not_modify_the_document() {
        let mut editor = editor();
        two_parts(&mut editor);
        let before = editor.document.clone();
        let proposal = editor.nest_selection(1.0, 0.0, false).unwrap();
        assert_eq!(proposal.placements.len(), 2);
        assert_eq!(editor.document, before);
        assert!(proposal.utilization() > 0.0);
    }

    #[test]
    fn proposal_honours_spacing_and_margin() {
        let mut editor = editor();
        two_parts(&mut editor);
        let proposal = editor.nest_selection(2.0, 5.0, false).unwrap();
        assert_eq!(proposal.placements.len(), 2);
        let first = &proposal.placements[0];
        let second = &proposal.placements[1];
        assert_eq!((first.min.x, first.min.y), (5.0, 5.0));
        // The tallest part is placed first (20 x 15 and 10 x 20).
        assert!(second.min.x > first.min.x || second.min.y > first.min.y);
        assert!(
            (first.width * first.height + second.width * second.height - proposal.used_area).abs()
                < 1e-9
        );
    }

    #[test]
    fn parts_that_do_not_fit_are_reported() {
        let mut editor = editor();
        editor
            .add_rectangle(Point2::new(0.0, 0.0), Point2::new(200.0, 200.0))
            .unwrap();
        editor.select_all();
        let proposal = editor.nest_selection(0.0, 0.0, false).unwrap();
        assert_eq!(proposal.placements.len(), 0);
        assert_eq!(proposal.unplaced.len(), 1);
    }

    #[test]
    fn nest_requires_a_selection() {
        let mut editor = editor();
        editor
            .add_rectangle(Point2::new(0.0, 0.0), Point2::new(10.0, 10.0))
            .unwrap();
        editor.selection.clear();
        assert!(editor.nest_selection(1.0, 0.0, false).is_err());
        editor.select_all();
        assert!(editor.nest_selection(-1.0, 0.0, false).is_err());
        assert!(editor.nest_selection(1.0, f64::NAN, false).is_err());
    }

    #[test]
    fn applying_moves_parts_to_their_places() {
        let mut editor = editor();
        let (first, second) = two_parts(&mut editor);
        let proposal = editor.nest_selection(1.0, 0.0, false).unwrap();
        assert_eq!(editor.apply_nest(&proposal), 2);
        for placement in &proposal.placements {
            let (min, max) = editor
                .document
                .object(&placement.object_id)
                .unwrap()
                .bounds()
                .unwrap();
            assert!((min.x - placement.min.x).abs() < 1e-6);
            assert!((min.y - placement.min.y).abs() < 1e-6);
            assert!((max.x - min.x - placement.width).abs() < 1e-6);
            assert!((max.y - min.y - placement.height).abs() < 1e-6);
        }
        assert!(editor.document.object(&first).is_some());
        assert!(editor.document.object(&second).is_some());
    }

    #[test]
    fn applying_is_undoable() {
        let mut editor = editor();
        two_parts(&mut editor);
        let before = editor.document.clone();
        let proposal = editor.nest_selection(1.0, 0.0, false).unwrap();
        editor.apply_nest(&proposal);
        assert_ne!(editor.document, before);
        assert!(editor.undo().is_some());
        assert_eq!(editor.document, before);
    }

    #[test]
    fn rotated_placements_swap_the_bounds() {
        let mut editor = editor();
        editor
            .add_rectangle(Point2::new(0.0, 0.0), Point2::new(30.0, 10.0))
            .unwrap();
        editor.select_all();
        let proposal = editor.nest_selection(0.0, 0.0, true).unwrap();
        // A 30 x 10 part fits without rotation, so force the rotated case by
        // checking the transform path directly.
        let mut rotated = proposal.clone();
        rotated.placements[0].rotated = true;
        rotated.placements[0].width = 10.0;
        rotated.placements[0].height = 30.0;
        rotated.placements[0].min = Point2::new(0.0, 0.0);
        assert_eq!(editor.apply_nest(&rotated), 1);
        let object = &editor.document.objects[0];
        let (min, max) = object.bounds().unwrap();
        assert!((max.x - min.x - 10.0).abs() < 1e-6);
        assert!((max.y - min.y - 30.0).abs() < 1e-6);
    }

    #[test]
    fn missing_objects_are_skipped() {
        let mut editor = editor();
        two_parts(&mut editor);
        let mut proposal = editor.nest_selection(0.0, 0.0, false).unwrap();
        proposal.placements[0].object_id = "missing".to_string();
        assert_eq!(editor.apply_nest(&proposal), 1);
        assert_eq!(editor.apply_nest(&NestProposal::default()), 0);
    }
}
