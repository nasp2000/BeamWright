#[derive(Debug, Clone, Default)]
pub struct Selection {
    ids: Vec<String>,
}

impl Selection {
    pub fn new() -> Self {
        Self { ids: Vec::new() }
    }
    pub fn clear(&mut self) {
        self.ids.clear();
    }
    pub fn add(&mut self, id: String) {
        if !self.ids.contains(&id) {
            self.ids.push(id);
        }
    }
    pub fn remove(&mut self, id: &str) {
        self.ids.retain(|x| x != id);
    }
    pub fn toggle(&mut self, id: String) {
        if self.ids.contains(&id) {
            self.remove(&id);
        } else {
            self.add(id);
        }
    }
    pub fn contains(&self, id: &str) -> bool {
        self.ids.contains(&id.to_string())
    }
    pub fn len(&self) -> usize {
        self.ids.len()
    }
    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }
    pub fn ids(&self) -> &[String] {
        &self.ids
    }
}

#[cfg(test)]
mod tests {
    use super::Selection;

    #[test]
    fn empty_by_default() {
        let s = Selection::new();
        assert!(s.is_empty());
        assert_eq!(s.len(), 0);
    }

    #[test]
    fn add_and_contains() {
        let mut s = Selection::new();
        s.add("a".to_string());
        assert!(s.contains("a"));
        assert_eq!(s.len(), 1);
        // duplicate ignored
        s.add("a".to_string());
        assert_eq!(s.len(), 1);
    }

    #[test]
    fn remove() {
        let mut s = Selection::new();
        s.add("a".to_string());
        s.add("b".to_string());
        s.remove("a");
        assert!(!s.contains("a"));
        assert!(s.contains("b"));
        assert_eq!(s.len(), 1);
    }

    #[test]
    fn toggle() {
        let mut s = Selection::new();
        s.toggle("x".to_string());
        assert!(s.contains("x"));
        s.toggle("x".to_string());
        assert!(!s.contains("x"));
    }

    #[test]
    fn clear() {
        let mut s = Selection::new();
        s.add("a".to_string());
        s.clear();
        assert!(s.is_empty());
    }
}
