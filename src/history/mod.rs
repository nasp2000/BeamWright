use crate::document::Document;

/// One undo step: the document state before a change, plus the action name.
#[derive(Debug, Clone)]
struct Entry {
    document: Document,
    action: String,
}

impl Entry {
    fn new(document: &Document, action: &str) -> Self {
        Self {
            document: document.clone(),
            action: action.to_string(),
        }
    }
}

/// Snapshot-based undo/redo with readable action names.
///
/// Each entry is a complete document state plus the name of the change that
/// produced the next state. Documents are small enough for the editor's scope,
/// and snapshots cannot drift out of sync the way hand-written inverse commands
/// can.
#[derive(Debug, Clone)]
pub struct History {
    undo: Vec<Entry>,
    redo: Vec<Entry>,
    limit: usize,
}

impl Default for History {
    fn default() -> Self {
        Self::new(100)
    }
}

impl History {
    pub fn new(limit: usize) -> Self {
        Self {
            undo: Vec::new(),
            redo: Vec::new(),
            limit: limit.max(1),
        }
    }

    /// Record the state *before* a change and the name of the change.
    ///
    /// Repeated identical snapshots are ignored so a click without a drag does
    /// not create an empty undo step.
    pub fn record(&mut self, document: &Document, action: &str) {
        if self
            .undo
            .last()
            .is_some_and(|entry| &entry.document == document)
        {
            return;
        }
        self.undo.push(Entry::new(document, action));
        if self.undo.len() > self.limit {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    /// Restore the previous state. Returns the name of the undone action, or
    /// `None` when there is nothing to undo.
    pub fn undo(&mut self, document: &mut Document) -> Option<String> {
        let entry = self.undo.pop()?;
        let action = entry.action.clone();
        let current = std::mem::replace(document, entry.document);
        self.redo.push(Entry {
            document: current,
            action: action.clone(),
        });
        Some(action)
    }

    /// Reapply the last undone state. Returns the name of the redone action.
    pub fn redo(&mut self, document: &mut Document) -> Option<String> {
        let entry = self.redo.pop()?;
        let action = entry.action.clone();
        let current = std::mem::replace(document, entry.document);
        self.undo.push(Entry {
            document: current,
            action: action.clone(),
        });
        Some(action)
    }

    /// Change how many snapshots are kept, dropping the oldest ones.
    pub fn set_limit(&mut self, limit: usize) {
        self.limit = limit.max(1);
        while self.undo.len() > self.limit {
            self.undo.remove(0);
        }
    }

    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }

    /// Action names of the undo steps, oldest first.
    pub fn labels(&self) -> Vec<String> {
        self.undo.iter().map(|entry| entry.action.clone()).collect()
    }

    /// Action names of the redo steps, next redo first.
    pub fn redo_labels(&self) -> Vec<String> {
        self.redo
            .iter()
            .rev()
            .map(|entry| entry.action.clone())
            .collect()
    }

    /// Undo `steps` actions at once, returning the name of the last undone one.
    pub fn jump_to(&mut self, document: &mut Document, steps: usize) -> Option<String> {
        let steps = steps.min(self.undo.len());
        let mut last = None;
        for _ in 0..steps {
            last = self.undo(document);
        }
        last
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Name of the action that [`Self::undo`] would revert.
    pub fn undo_label(&self) -> Option<&str> {
        self.undo.last().map(|entry| entry.action.as_str())
    }

    /// Name of the action that [`Self::redo`] would reapply.
    pub fn redo_label(&self) -> Option<&str> {
        self.redo.last().map(|entry| entry.action.as_str())
    }

    pub fn undo_len(&self) -> usize {
        self.undo.len()
    }

    pub fn redo_len(&self) -> usize {
        self.redo.len()
    }
}

#[cfg(test)]
#[allow(clippy::assert_is_empty)]
mod tests {
    use super::History;
    use crate::document::shape::Shape;
    use crate::document::{Document, Object};

    fn document_with_rect() -> Document {
        let mut document = Document::default();
        let layer_id = document.layers[0].id.clone();
        document.add_object(Object::new(
            layer_id,
            Shape::Rect {
                width: 10.0,
                height: 10.0,
                corner_radius: 0.0,
            },
        ));
        document
    }

    #[test]
    fn lowering_the_limit_drops_only_the_oldest_steps() {
        let mut document = Document::default();
        let mut history = History::new(10);
        let layer_id = document.layers[0].id.clone();
        for index in 0..5 {
            history.record(&document, &format!("Step {index}"));
            document.add_object(Object::new(
                layer_id.clone(),
                Shape::Rect {
                    width: 1.0,
                    height: 1.0,
                    corner_radius: 0.0,
                },
            ));
        }
        assert_eq!(history.labels().len(), 5);
        history.set_limit(2);
        assert_eq!(history.labels(), vec!["Step 3", "Step 4"]);
        assert_eq!(
            history.jump_to(&mut document, 10),
            Some("Step 3".to_string())
        );
        assert_eq!(history.undo_len(), 0);
        assert_eq!(history.limit, 2);
    }

    #[test]
    fn jumping_back_moves_the_steps_to_redo() {
        let mut document = Document::default();
        let mut history = History::new(10);
        let layer_id = document.layers[0].id.clone();
        history.record(&document, "One");
        document.add_object(Object::new(
            layer_id.clone(),
            Shape::Rect {
                width: 1.0,
                height: 1.0,
                corner_radius: 0.0,
            },
        ));
        history.record(&document, "Two");
        document.add_object(Object::new(
            layer_id.clone(),
            Shape::Rect {
                width: 2.0,
                height: 2.0,
                corner_radius: 0.0,
            },
        ));
        history.record(&document, "Three");
        assert_eq!(history.labels(), vec!["One", "Two", "Three"]);
        assert_eq!(history.redo_labels(), Vec::<String>::new());

        assert_eq!(history.jump_to(&mut document, 2), Some("Two".to_string()));
        assert_eq!(document.objects.len(), 1);
        assert_eq!(history.redo_labels(), vec!["Two", "Three"]);

        // Jumping past the oldest step stops there instead of failing.
        assert_eq!(history.jump_to(&mut document, 99), Some("One".to_string()));
        assert_eq!(history.undo_len(), 0);
        assert_eq!(history.jump_to(&mut document, 1), None);
    }

    #[test]
    fn undo_and_redo_restore_snapshots() {
        let mut document = Document::default();
        let mut history = History::new(10);
        history.record(&document, "Draw rectangle");
        document = document_with_rect();
        assert_eq!(document.objects.len(), 1);

        assert_eq!(
            history.undo(&mut document).as_deref(),
            Some("Draw rectangle")
        );
        assert!(document.objects.is_empty());
        assert!(history.can_redo());

        assert_eq!(
            history.redo(&mut document).as_deref(),
            Some("Draw rectangle")
        );
        assert_eq!(document.objects.len(), 1);
    }

    #[test]
    fn labels_track_the_next_undo_and_redo() {
        let mut document = Document::default();
        let mut history = History::new(10);
        assert_eq!(history.undo_label(), None);
        assert_eq!(history.redo_label(), None);

        history.record(&document, "Move");
        document = document_with_rect();
        assert_eq!(history.undo_label(), Some("Move"));

        assert_eq!(history.undo(&mut document).as_deref(), Some("Move"));
        assert_eq!(history.undo_label(), None);
        assert_eq!(history.redo_label(), Some("Move"));

        assert_eq!(history.redo(&mut document).as_deref(), Some("Move"));
        assert_eq!(history.undo_label(), Some("Move"));
        assert_eq!(history.redo_label(), None);
    }

    #[test]
    fn new_edit_clears_redo() {
        let mut document = Document::default();
        let mut history = History::new(10);
        history.record(&document, "Move");
        document = document_with_rect();
        assert!(history.undo(&mut document).is_some());
        history.record(&document, "Delete");
        assert!(!history.can_redo());
        assert_eq!(history.undo_label(), Some("Delete"));
    }

    #[test]
    fn duplicate_snapshots_are_ignored() {
        let document = Document::default();
        let mut history = History::new(10);
        history.record(&document, "Move");
        history.record(&document, "Move again");
        assert_eq!(history.undo_len(), 1);
        assert_eq!(history.undo_label(), Some("Move"));
    }

    #[test]
    fn undo_on_empty_history_is_a_no_op() {
        let mut document = Document::default();
        let mut history = History::new(10);
        assert_eq!(history.undo(&mut document), None);
        assert_eq!(history.redo(&mut document), None);
    }

    #[test]
    fn limit_drops_oldest_snapshot() {
        let mut document = Document::default();
        let mut history = History::new(2);
        for action in ["One", "Two", "Three", "Four", "Five"] {
            history.record(&document, action);
            document.name.push('x');
        }
        assert_eq!(history.undo_len(), 2);
        assert_eq!(history.undo_label(), Some("Five"));
    }
}
