#[derive(Debug, Clone, Default)]
pub struct Toolbar {
    pub visible: bool,
}

impl Toolbar {
    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }
}

#[cfg(test)]
mod tests {
    use super::Toolbar;

    #[test]
    fn toggle() {
        let mut t = Toolbar::default();
        assert!(!t.visible);
        t.toggle();
        assert!(t.visible);
    }

    #[test]
    fn default_hidden() {
        assert!(!Toolbar::default().visible);
    }

    #[test]
    fn clone() {
        let mut t = Toolbar::default();
        t.toggle();
        let c = t.clone();
        assert_eq!(c.visible, t.visible);
    }
}
