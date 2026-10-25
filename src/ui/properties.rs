#[derive(Debug, Clone, Default)]
pub struct PropertiesPanel {
    pub visible: bool,
}

impl PropertiesPanel {
    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }
}

#[cfg(test)]
mod tests {
    use super::PropertiesPanel;

    #[test]
    fn toggle() {
        let mut p = PropertiesPanel::default();
        p.toggle();
        assert!(p.visible);
    }

    #[test]
    fn default_hidden() {
        assert!(!PropertiesPanel::default().visible);
    }

    #[test]
    fn clone() {
        let p = PropertiesPanel::default();
        let c = p.clone();
        assert_eq!(c.visible, p.visible);
    }
}
