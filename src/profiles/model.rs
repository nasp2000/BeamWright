use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProfileType {
    Material,
    Operation,
    Image,
    Project,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub kind: ProfileType,
    pub schema_version: u32,
    pub tags: Vec<String>,
}

impl Profile {
    pub fn new(name: String, profile_type: ProfileType) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name,
            kind: profile_type,
            schema_version: 1,
            tags: Vec::new(),
        }
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.name.trim().is_empty() {
            return Err("profile name must not be empty".to_string());
        }
        if self.id.is_empty() {
            return Err("profile id must not be empty".to_string());
        }
        if self.schema_version == 0 {
            return Err("schema version must be >= 1".to_string());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_valid() {
        let p = Profile::new("Test".to_string(), ProfileType::Material);
        assert!(p.validate().is_ok());
        assert_ne!(p.id, "");
    }

    #[test]
    fn reject_empty_name() {
        let mut p = Profile::new("Test".to_string(), ProfileType::Material);
        p.name = String::new();
        assert!(p.validate().is_err());
    }

    #[test]
    fn reject_empty_id() {
        let mut p = Profile::new("Test".to_string(), ProfileType::Material);
        p.id = String::new();
        assert!(p.validate().is_err());
    }

    #[test]
    fn reject_zero_schema() {
        let mut p = Profile::new("Test".to_string(), ProfileType::Material);
        p.schema_version = 0;
        assert!(p.validate().is_err());
    }

    #[test]
    fn profile_types_distinct() {
        assert_ne!(ProfileType::Material, ProfileType::Operation);
        assert_ne!(ProfileType::Image, ProfileType::Project);
    }
}
