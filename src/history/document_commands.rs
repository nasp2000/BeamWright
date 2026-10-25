use crate::history::Command;

pub struct AddObjectCommand {
    pub object_id: String,
    applied: bool,
}

impl AddObjectCommand {
    pub fn new(object_id: String) -> Self {
        Self {
            object_id,
            applied: false,
        }
    }
}

impl Command for AddObjectCommand {
    fn name(&self) -> &'static str {
        "add_object"
    }
    fn apply(&mut self) {
        self.applied = true;
    }
    fn revert(&mut self) {
        self.applied = false;
    }
}

pub struct RemoveObjectCommand {
    pub object_id: String,
    applied: bool,
}

impl RemoveObjectCommand {
    pub fn new(object_id: String) -> Self {
        Self {
            object_id,
            applied: false,
        }
    }
}

impl Command for RemoveObjectCommand {
    fn name(&self) -> &'static str {
        "remove_object"
    }
    fn apply(&mut self) {
        self.applied = true;
    }
    fn revert(&mut self) {
        self.applied = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::UndoStack;

    #[test]
    fn add_apply() {
        let mut cmd = AddObjectCommand::new("o1".to_string());
        cmd.apply();
        assert!(cmd.applied);
        cmd.revert();
        assert!(!cmd.applied);
    }

    #[test]
    fn remove_apply() {
        let mut cmd = RemoveObjectCommand::new("o1".to_string());
        cmd.apply();
        assert!(cmd.applied);
    }

    #[test]
    fn undo_stack_integration() {
        let mut s = UndoStack::default();
        s.push(Box::new(AddObjectCommand::new("o1".to_string())));
        assert!(s.can_undo());
        assert!(s.undo());
        assert!(s.can_redo());
    }
}
