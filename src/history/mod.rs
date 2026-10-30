use crate::document::Document;

/// Snapshot-based undo/redo.
///
/// Each entry is a complete document state. Documents are small enough for the
/// editor's scope, and snapshots cannot drift out of sync the way hand-written
/// inverse commands can.
#[derive(Debug, Clone)]
pub struct History {
    undo: Vec<Document>,
    redo: Vec<Document>,
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

    /// Record the state *before* a change. Repeated identical snapshots are
    /// ignored so a click without a drag does not create an empty undo step.
    pub fn record(&mut self, document: &Document) {
        if self.undo.last() == Some(document) {
            return;
        }
        self.undo.push(document.clone());
        if self.undo.len() > self.limit {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    /// Restore the previous state. Returns `false` when there is nothing to
    /// undo, leaving `document` untouched.
    pub fn undo(&mut self, document: &mut Document) -> bool {
        let Some(previous) = self.undo.pop() else {
            return false;
        };
        self.redo.push(std::mem::replace(document, previous));
        true
    }

    pub fn redo(&mut self, document: &mut Document) -> bool {
        let Some(next) = self.redo.pop() else {
            return false;
        };
        self.undo.push(std::mem::replace(document, next));
        true
    }

    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
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
    fn undo_and_redo_restore_snapshots() {
        let mut document = Document::default();
        let mut history = History::new(10);
        history.record(&document);
        document = document_with_rect();
        assert_eq!(document.objects.len(), 1);

        assert!(history.undo(&mut document));
        assert!(document.objects.is_empty());
        assert!(history.can_redo());

        assert!(history.redo(&mut document));
        assert_eq!(document.objects.len(), 1);
    }

    #[test]
    fn new_edit_clears_redo() {
        let mut document = Document::default();
        let mut history = History::new(10);
        history.record(&document);
        document = document_with_rect();
        assert!(history.undo(&mut document));
        history.record(&document);
        assert!(!history.can_redo());
    }

    #[test]
    fn duplicate_snapshots_are_ignored() {
        let document = Document::default();
        let mut history = History::new(10);
        history.record(&document);
        history.record(&document);
        assert_eq!(history.undo_len(), 1);
    }

    #[test]
    fn undo_on_empty_history_is_a_no_op() {
        let mut document = Document::default();
        let mut history = History::new(10);
        assert!(!history.undo(&mut document));
        assert!(!history.redo(&mut document));
    }

    #[test]
    fn limit_drops_oldest_snapshot() {
        let mut document = Document::default();
        let mut history = History::new(2);
        for _ in 0..5 {
            history.record(&document);
            document.name.push('x');
        }
        assert_eq!(history.undo_len(), 2);
    }
}
