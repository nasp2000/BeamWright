use crate::profiles::Profile;

#[derive(Debug, Default)]
pub struct ProfileLibrary {
    pub profiles: Vec<Profile>,
}

impl ProfileLibrary {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn add(&mut self, p: Profile) {
        self.profiles.push(p);
    }
    pub fn remove(&mut self, id: &str) -> Option<Profile> {
        if let Some(i) = self.profiles.iter().position(|p| p.id == id) {
            Some(self.profiles.remove(i))
        } else {
            None
        }
    }
    pub fn find(&self, id: &str) -> Option<&Profile> {
        self.profiles.iter().find(|p| p.id == id)
    }
    pub fn duplicate(&mut self, id: &str) -> Option<Profile> {
        if let Some(p) = self.profiles.iter().find(|p| p.id == id).cloned() {
            let mut copy = p;
            copy.id = uuid::Uuid::new_v4().to_string();
            copy.name = format!("{} (copy)", copy.name);
            self.profiles.push(copy.clone());
            Some(copy)
        } else {
            None
        }
    }
    pub fn by_kind(&self, kind: crate::profiles::ProfileType) -> Vec<&Profile> {
        self.profiles.iter().filter(|p| p.kind == kind).collect()
    }
    pub fn len(&self) -> usize {
        self.profiles.len()
    }
    pub fn is_empty(&self) -> bool {
        self.profiles.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profiles::{Profile, ProfileType};

    #[test]
    fn add_find() {
        let mut lib = ProfileLibrary::new();
        let p = Profile::new("Op1".to_string(), ProfileType::Operation);
        let id = p.id.clone();
        lib.add(p);
        assert_eq!(lib.len(), 1);
        assert!(lib.find(&id).is_some());
    }

    #[test]
    fn duplicate() {
        let mut lib = ProfileLibrary::new();
        let p = Profile::new("Op1".to_string(), ProfileType::Operation);
        let id = p.id.clone();
        lib.add(p);
        let copy = lib.duplicate(&id).unwrap();
        assert_ne!(copy.id, id);
        assert!(copy.name.contains("copy"));
        assert_eq!(lib.len(), 2);
    }

    #[test]
    fn duplicate_missing() {
        let mut lib = ProfileLibrary::new();
        assert!(lib.duplicate("nope").is_none());
    }

    #[test]
    fn by_kind() {
        let mut lib = ProfileLibrary::new();
        lib.add(Profile::new("a".to_string(), ProfileType::Material));
        lib.add(Profile::new("b".to_string(), ProfileType::Operation));
        assert_eq!(lib.by_kind(ProfileType::Material).len(), 1);
    }
}
