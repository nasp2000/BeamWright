#[derive(Debug, Clone, Default)]
pub struct Group {
    pub name: String,
    pub children: Vec<String>,
}

impl Group {
    pub fn is_empty(&self) -> bool {
        self.children.is_empty()
    }
    pub fn add(&mut self, id: String) {
        if !self.children.contains(&id) {
            self.children.push(id);
        }
    }
    pub fn remove(&mut self, id: &str) {
        self.children.retain(|x| x != id);
    }
}

#[cfg(test)]
mod tests {
    use super::Group;

    #[test]
    fn empty() {
        let g = Group::default();
        assert!(g.is_empty());
    }

    #[test]
    fn add_remove() {
        let mut g = Group::default();
        g.add("a".to_string());
        assert!(!g.is_empty());
        assert_eq!(g.children.len(), 1);
        g.add("a".to_string());
        assert_eq!(g.children.len(), 1);
        g.remove("a");
        assert!(g.is_empty());
    }

    #[test]
    fn name() {
        let g = Group {
            name: "G1".to_string(),
            ..Group::default()
        };
        assert_eq!(g.name, "G1");
    }
}
