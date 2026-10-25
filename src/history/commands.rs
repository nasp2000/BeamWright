/// A reversible action.
pub trait Command {
    fn name(&self) -> &'static str;
    fn apply(&mut self);
    fn revert(&mut self);
}

/// Undo/redo stack with a bounded history.
pub struct UndoStack {
    undo: Vec<Box<dyn Command>>,
    redo: Vec<Box<dyn Command>>,
    limit: usize,
}

impl Default for UndoStack {
    fn default() -> Self {
        Self::new(256)
    }
}

impl UndoStack {
    pub fn new(limit: usize) -> Self {
        Self {
            undo: Vec::new(),
            redo: Vec::new(),
            limit: limit.max(1),
        }
    }

    pub fn push(&mut self, mut cmd: Box<dyn Command>) {
        cmd.apply();
        self.undo.push(cmd);
        if self.undo.len() > self.limit {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    pub fn undo(&mut self) -> bool {
        if let Some(mut cmd) = self.undo.pop() {
            cmd.revert();
            self.redo.push(cmd);
            true
        } else {
            false
        }
    }

    pub fn redo(&mut self) -> bool {
        if let Some(mut cmd) = self.redo.pop() {
            cmd.apply();
            self.undo.push(cmd);
            true
        } else {
            false
        }
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

    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }
}

// ---- test command ----

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::rc::Rc;

    struct Inc {
        counter: Rc<Cell<i32>>,
        delta: i32,
    }

    impl Command for Inc {
        fn name(&self) -> &'static str {
            "inc"
        }
        fn apply(&mut self) {
            self.counter.set(self.counter.get() + self.delta);
        }
        fn revert(&mut self) {
            self.counter.set(self.counter.get() - self.delta);
        }
    }

    #[test]
    fn push_applies() {
        let c = Rc::new(Cell::new(0));
        let mut s = UndoStack::default();
        s.push(Box::new(Inc {
            counter: Rc::clone(&c),
            delta: 5,
        }));
        assert_eq!(c.get(), 5);
        assert!(s.can_undo());
    }

    #[test]
    fn undo_reverts() {
        let c = Rc::new(Cell::new(0));
        let mut s = UndoStack::default();
        s.push(Box::new(Inc {
            counter: Rc::clone(&c),
            delta: 5,
        }));
        assert!(s.undo());
        assert_eq!(c.get(), 0);
        assert!(s.can_redo());
    }

    #[test]
    fn redo_reapplies() {
        let c = Rc::new(Cell::new(0));
        let mut s = UndoStack::default();
        s.push(Box::new(Inc {
            counter: Rc::clone(&c),
            delta: 5,
        }));
        s.undo();
        assert!(s.redo());
        assert_eq!(c.get(), 5);
    }

    #[test]
    fn new_push_clears_redo() {
        let c = Rc::new(Cell::new(0));
        let mut s = UndoStack::default();
        s.push(Box::new(Inc {
            counter: Rc::clone(&c),
            delta: 5,
        }));
        s.undo();
        s.push(Box::new(Inc {
            counter: Rc::clone(&c),
            delta: 2,
        }));
        assert!(!s.can_redo());
        assert_eq!(c.get(), 2);
    }

    #[test]
    fn limit_enforced() {
        let c = Rc::new(Cell::new(0));
        let mut s = UndoStack::new(2);
        for _ in 0..5 {
            s.push(Box::new(Inc {
                counter: Rc::clone(&c),
                delta: 1,
            }));
        }
        assert_eq!(s.undo_len(), 2);
    }

    #[test]
    fn undo_empty_returns_false() {
        let mut s = UndoStack::default();
        assert!(!s.undo());
        assert!(!s.redo());
    }
}
