#[derive(Debug, Clone, Default)]
pub struct DialogState {
    pub open: bool,
}

impl DialogState {
    pub fn open(&mut self) {
        self.open = true;
    }
    pub fn close(&mut self) {
        self.open = false;
    }
}

#[cfg(test)]
mod tests {
    use super::DialogState;

    #[test]
    fn open_close() {
        let mut d = DialogState::default();
        assert!(!d.open);
        d.open();
        assert!(d.open);
        d.close();
        assert!(!d.open);
    }

    #[test]
    fn default_closed() {
        assert!(!DialogState::default().open);
    }

    #[test]
    fn clone() {
        let mut d = DialogState::default();
        d.open();
        let c = d.clone();
        assert_eq!(c.open, d.open);
    }
}
