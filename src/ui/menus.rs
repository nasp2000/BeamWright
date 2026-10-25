#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuAction {
    New,
    Open,
    Save,
    Undo,
    Redo,
    Quit,
}

#[cfg(test)]
mod tests {
    use super::MenuAction;

    #[test]
    fn equality() {
        assert_eq!(MenuAction::New, MenuAction::New);
        assert_ne!(MenuAction::New, MenuAction::Open);
    }

    #[test]
    fn copy() {
        let a = MenuAction::Save;
        let b = a;
        assert_eq!(a, b);
    }

    #[test]
    fn debug() {
        assert_ne!(format!("{:?}", MenuAction::Quit), "");
    }
}
