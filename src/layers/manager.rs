use crate::layers::Layer;

#[derive(Debug, Default)]
pub struct LayerStack {
    pub layers: Vec<Layer>,
}

impl LayerStack {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn add(&mut self, layer: Layer) {
        self.layers.push(layer);
        self.reorder();
    }
    pub fn remove(&mut self, id: &str) -> Option<Layer> {
        if let Some(idx) = self.layers.iter().position(|l| l.id == id) {
            Some(self.layers.remove(idx))
        } else {
            None
        }
    }
    pub fn find(&self, id: &str) -> Option<&Layer> {
        self.layers.iter().find(|l| l.id == id)
    }
    pub fn find_mut(&mut self, id: &str) -> Option<&mut Layer> {
        self.layers.iter_mut().find(|l| l.id == id)
    }
    pub fn reorder(&mut self) {
        self.layers.sort_by_key(|l| l.order);
    }
    pub fn visible_layers(&self) -> Vec<&Layer> {
        self.layers.iter().filter(|l| l.visible).collect()
    }
    pub fn len(&self) -> usize {
        self.layers.len()
    }
    pub fn is_empty(&self) -> bool {
        self.layers.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layers::Layer;

    fn mk(id: &str, order: u32) -> Layer {
        Layer {
            id: id.to_string(),
            order,
            ..Layer::default()
        }
    }

    #[test]
    fn add_and_find() {
        let mut s = LayerStack::new();
        s.add(mk("a", 1));
        assert_eq!(s.len(), 1);
        assert!(s.find("a").is_some());
    }

    #[test]
    fn remove() {
        let mut s = LayerStack::new();
        s.add(mk("a", 1));
        let r = s.remove("a");
        assert!(r.is_some());
        assert!(s.is_empty());
        assert!(s.remove("a").is_none());
    }

    #[test]
    fn reorder_by_order() {
        let mut s = LayerStack::new();
        s.add(mk("b", 2));
        s.add(mk("a", 1));
        assert_eq!(s.layers[0].id, "a");
        assert_eq!(s.layers[1].id, "b");
    }

    #[test]
    fn visible_filter() {
        let mut s = LayerStack::new();
        let mut l = mk("a", 1);
        l.visible = false;
        s.add(l);
        s.add(mk("b", 2));
        assert_eq!(s.visible_layers().len(), 1);
    }
}
