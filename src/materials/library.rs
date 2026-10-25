use crate::materials::Material;

#[derive(Debug, Default)]
pub struct MaterialLibrary {
    pub materials: Vec<Material>,
}

impl MaterialLibrary {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn add(&mut self, m: Material) {
        self.materials.push(m);
    }
    pub fn remove(&mut self, id: &str) -> Option<Material> {
        if let Some(i) = self.materials.iter().position(|m| m.id == id) {
            Some(self.materials.remove(i))
        } else {
            None
        }
    }
    pub fn find(&self, id: &str) -> Option<&Material> {
        self.materials.iter().find(|m| m.id == id)
    }
    pub fn search(&self, query: &str) -> Vec<&Material> {
        if query.trim().is_empty() {
            return self.materials.iter().collect();
        }
        let q = query.to_lowercase();
        self.materials
            .iter()
            .filter(|m| {
                m.name.to_lowercase().contains(&q) || m.category.to_lowercase().contains(&q)
            })
            .collect()
    }
    pub fn len(&self) -> usize {
        self.materials.len()
    }
    pub fn is_empty(&self) -> bool {
        self.materials.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::materials::Material;

    fn mk(name: &str) -> Material {
        Material {
            id: name.to_string(),
            name: name.to_string(),
            category: "Wood".to_string(),
            thickness_mm: 3.0,
            color: "#000".to_string(),
            notes: String::new(),
        }
    }

    #[test]
    fn add_find() {
        let mut lib = MaterialLibrary::new();
        lib.add(mk("ply"));
        assert_eq!(lib.len(), 1);
        assert!(lib.find("ply").is_some());
    }

    #[test]
    fn remove() {
        let mut lib = MaterialLibrary::new();
        lib.add(mk("ply"));
        assert!(lib.remove("ply").is_some());
        assert!(lib.is_empty());
    }

    #[test]
    fn search() {
        let mut lib = MaterialLibrary::new();
        lib.add(mk("plywood"));
        lib.add(mk("acrylic"));
        assert_eq!(lib.search("ply").len(), 1);
        assert_eq!(lib.search("").len(), 2);
        assert_eq!(lib.search("zzz").len(), 0);
    }
}
