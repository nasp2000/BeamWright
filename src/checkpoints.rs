//! Bounded snapshots an agent session can return to.
//!
//! Checkpoints are independent of undo history: they stay available after the
//! agent makes new edits or undoes them, and the oldest one is discarded when
//! [`MAX_CHECKPOINTS`] is reached.

use crate::document::Document;

/// How many checkpoints a session keeps.
pub const MAX_CHECKPOINTS: usize = 16;

#[derive(Debug, Clone)]
struct Entry {
    name: String,
    document: Document,
}

/// A ring of named document snapshots, oldest first.
#[derive(Debug, Default, Clone)]
pub struct Checkpoints {
    entries: Vec<Entry>,
}

impl Checkpoints {
    pub fn new() -> Self {
        Self::default()
    }

    /// Store a copy of `document` and return its index.
    pub fn push(&mut self, name: impl Into<String>, document: &Document) -> usize {
        if self.entries.len() >= MAX_CHECKPOINTS {
            self.entries.remove(0);
        }
        let index = self.entries.len();
        self.entries.push(Entry {
            name: name.into(),
            document: document.clone(),
        });
        index
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn names(&self) -> Vec<&str> {
        self.entries
            .iter()
            .map(|entry| entry.name.as_str())
            .collect()
    }

    pub fn name(&self, index: usize) -> Option<&str> {
        self.entries.get(index).map(|entry| entry.name.as_str())
    }

    /// The most recently stored checkpoint index, if any.
    pub fn last_index(&self) -> Option<usize> {
        self.entries.len().checked_sub(1)
    }

    /// The first checkpoint with this name.
    pub fn find(&self, name: &str) -> Option<usize> {
        self.entries.iter().position(|entry| entry.name == name)
    }

    /// A copy of the stored document.
    pub fn get(&self, index: usize) -> Option<Document> {
        self.entries.get(index).map(|entry| entry.document.clone())
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::{Checkpoints, MAX_CHECKPOINTS};
    use crate::document::Document;

    fn document(name: &str) -> Document {
        Document::new(name, 100.0, 100.0)
    }

    #[test]
    fn checkpoints_keep_named_snapshots() {
        let mut checkpoints = Checkpoints::new();
        assert!(checkpoints.is_empty());
        let index = checkpoints.push("first", &document("One"));
        assert_eq!(index, 0);
        checkpoints.push("second", &document("Two"));
        assert_eq!(checkpoints.len(), 2);
        assert_eq!(checkpoints.names(), vec!["first", "second"]);
        assert_eq!(checkpoints.find("second"), Some(1));
        assert_eq!(checkpoints.name(0), Some("first"));
        assert_eq!(checkpoints.get(1).unwrap().name, "Two");
        assert!(checkpoints.get(5).is_none());
        assert_eq!(checkpoints.last_index(), Some(1));
    }

    #[test]
    fn the_oldest_checkpoint_is_dropped_at_the_limit() {
        let mut checkpoints = Checkpoints::new();
        for index in 0..MAX_CHECKPOINTS + 2 {
            checkpoints.push(format!("cp{index}"), &document(&format!("Doc {index}")));
        }
        assert_eq!(checkpoints.len(), MAX_CHECKPOINTS);
        assert_eq!(checkpoints.names()[0], "cp2");
        assert_eq!(checkpoints.names()[MAX_CHECKPOINTS - 1], "cp17");
        assert_eq!(checkpoints.last_index(), Some(MAX_CHECKPOINTS - 1));
    }

    #[test]
    fn a_restored_copy_is_independent() {
        let mut checkpoints = Checkpoints::new();
        let mut original = document("One");
        checkpoints.push("one", &original);
        original.name = "Changed".to_string();
        let restored = checkpoints.get(0).unwrap();
        assert_eq!(restored.name, "One");
        checkpoints.clear();
        assert!(checkpoints.is_empty());
        assert!(checkpoints.last_index().is_none());
    }
}
