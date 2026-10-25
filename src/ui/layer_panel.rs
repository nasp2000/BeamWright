#[derive(Debug, Clone, Default)]
pub struct LayerPanel {
    pub visible: bool,
}

impl LayerPanel {
    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }
}

#[cfg(test)]
mod tests {
    use super::LayerPanel;

    #[test]
    fn toggle() {
        let mut p = LayerPanel::default();
        assert!(!p.visible);
        p.toggle();
        assert!(p.visible);
        p.toggle();
        assert!(!p.visible);
    }

    #[test]
    fn default_hidden() {
        assert!(!LayerPanel::default().visible);
    }

    #[test]
    fn clone() {
        let mut p = LayerPanel::default();
        p.toggle();
        let c = p.clone();
        assert_eq!(c.visible, p.visible);
    }
}
