use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::operations::{OperationParams, OperationType};

/// Format version of the profile library file.
pub const PROFILE_FORMAT_VERSION: u32 = 1;

/// Reference material values. They are starting points, not tested recipes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MaterialProfile {
    pub id: String,
    pub name: String,
    pub material: String,
    pub thickness_mm: f64,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub builtin: bool,
}

impl MaterialProfile {
    pub fn new(name: impl Into<String>, material: impl Into<String>, thickness_mm: f64) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: name.into(),
            material: material.into(),
            thickness_mm,
            notes: String::new(),
            builtin: false,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.id.trim().is_empty() {
            return Err("material profile id must not be empty".to_string());
        }
        if self.name.trim().is_empty() {
            return Err("material profile name must not be empty".to_string());
        }
        if self.material.trim().is_empty() {
            return Err("material must not be empty".to_string());
        }
        if !self.thickness_mm.is_finite() || self.thickness_mm <= 0.0 {
            return Err("material thickness must be positive".to_string());
        }
        Ok(())
    }
}

/// Reusable operation settings that can be applied to objects.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OperationProfile {
    pub id: String,
    pub name: String,
    pub op_type: OperationType,
    pub params: OperationParams,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub builtin: bool,
}

impl OperationProfile {
    pub fn new(name: impl Into<String>, op_type: OperationType) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: name.into(),
            op_type,
            params: OperationParams::default(),
            notes: String::new(),
            builtin: false,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.id.trim().is_empty() {
            return Err("operation profile id must not be empty".to_string());
        }
        if self.name.trim().is_empty() {
            return Err("operation profile name must not be empty".to_string());
        }
        if !self.params.speed.is_finite() || self.params.speed <= 0.0 {
            return Err("operation speed must be positive".to_string());
        }
        if !self.params.power.is_finite() || !(0.0..=100.0).contains(&self.params.power) {
            return Err("operation power must be between 0 and 100 %".to_string());
        }
        if self.params.passes == 0 {
            return Err("operation passes must be at least 1".to_string());
        }
        if !self.params.line_interval_mm.is_finite() || self.params.line_interval_mm <= 0.0 {
            return Err("line interval must be positive".to_string());
        }
        Ok(())
    }
}

/// Material and operation profiles, built-in and user created.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProfileLibrary {
    pub format_version: u32,
    pub materials: Vec<MaterialProfile>,
    pub operations: Vec<OperationProfile>,
}

impl Default for ProfileLibrary {
    fn default() -> Self {
        Self::with_builtins()
    }
}

impl ProfileLibrary {
    pub fn empty() -> Self {
        Self {
            format_version: PROFILE_FORMAT_VERSION,
            materials: Vec::new(),
            operations: Vec::new(),
        }
    }

    /// Built-in reference profiles. They are marked as such and cannot be
    /// edited or deleted; duplicate them to customise.
    pub fn with_builtins() -> Self {
        let mut library = Self::empty();
        library.materials.push(MaterialProfile {
            id: "builtin-plywood-3".to_string(),
            name: "Plywood 3 mm".to_string(),
            material: "Plywood".to_string(),
            thickness_mm: 3.0,
            notes: "Reference values only; test on your own material.".to_string(),
            builtin: true,
        });
        library.materials.push(MaterialProfile {
            id: "builtin-acrylic-3".to_string(),
            name: "Acrylic 3 mm".to_string(),
            material: "Acrylic".to_string(),
            thickness_mm: 3.0,
            notes: "Reference values only; test on your own material.".to_string(),
            builtin: true,
        });
        library.materials.push(MaterialProfile {
            id: "builtin-cardboard-2".to_string(),
            name: "Cardboard 2 mm".to_string(),
            material: "Cardboard".to_string(),
            thickness_mm: 2.0,
            notes: "Reference values only; test on your own material.".to_string(),
            builtin: true,
        });
        library.operations.push(OperationProfile {
            id: "builtin-cut".to_string(),
            name: "Outline cut".to_string(),
            op_type: OperationType::OutlineCutting,
            params: OperationParams {
                speed: 600.0,
                power: 80.0,
                passes: 2,
                line_interval_mm: 0.1,
            },
            notes: "Reference values only; test on your own material.".to_string(),
            builtin: true,
        });
        library.operations.push(OperationProfile {
            id: "builtin-engrave".to_string(),
            name: "Line engraving".to_string(),
            op_type: OperationType::LineEngraving,
            params: OperationParams {
                speed: 3000.0,
                power: 25.0,
                passes: 1,
                line_interval_mm: 0.1,
            },
            notes: "Reference values only; test on your own material.".to_string(),
            builtin: true,
        });
        library
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.format_version != PROFILE_FORMAT_VERSION {
            return Err(format!(
                "unsupported profile format version {} (expected {})",
                self.format_version, PROFILE_FORMAT_VERSION
            ));
        }
        let mut ids = std::collections::HashSet::new();
        for material in &self.materials {
            material.validate()?;
            if !ids.insert(material.id.as_str()) {
                return Err(format!("duplicate profile id: {}", material.id));
            }
        }
        for operation in &self.operations {
            operation.validate()?;
            if !ids.insert(operation.id.as_str()) {
                return Err(format!("duplicate profile id: {}", operation.id));
            }
        }
        Ok(())
    }

    pub fn material(&self, id: &str) -> Option<&MaterialProfile> {
        self.materials.iter().find(|profile| profile.id == id)
    }

    pub fn operation(&self, id: &str) -> Option<&OperationProfile> {
        self.operations.iter().find(|profile| profile.id == id)
    }

    pub fn add_material(&mut self, mut profile: MaterialProfile) -> &MaterialProfile {
        profile.builtin = false;
        self.materials.push(profile);
        self.materials.last().expect("profile was just pushed")
    }

    pub fn add_operation(&mut self, mut profile: OperationProfile) -> &OperationProfile {
        profile.builtin = false;
        self.operations.push(profile);
        self.operations.last().expect("profile was just pushed")
    }

    /// Remove a user profile. Built-in profiles are kept.
    pub fn remove_material(&mut self, id: &str) -> Result<(), String> {
        let index = self
            .materials
            .iter()
            .position(|profile| profile.id == id)
            .ok_or_else(|| format!("unknown material profile: {id}"))?;
        if self.materials[index].builtin {
            return Err("built-in profiles cannot be removed".to_string());
        }
        self.materials.remove(index);
        Ok(())
    }

    pub fn remove_operation(&mut self, id: &str) -> Result<(), String> {
        let index = self
            .operations
            .iter()
            .position(|profile| profile.id == id)
            .ok_or_else(|| format!("unknown operation profile: {id}"))?;
        if self.operations[index].builtin {
            return Err("built-in profiles cannot be removed".to_string());
        }
        self.operations.remove(index);
        Ok(())
    }

    /// Duplicate a profile as an editable user profile.
    pub fn duplicate_material(&mut self, id: &str) -> Option<String> {
        let mut copy = self.material(id)?.clone();
        copy.id = Uuid::new_v4().to_string();
        copy.name = format!("{} copy", copy.name);
        copy.builtin = false;
        let new_id = copy.id.clone();
        self.materials.push(copy);
        Some(new_id)
    }

    pub fn duplicate_operation(&mut self, id: &str) -> Option<String> {
        let mut copy = self.operation(id)?.clone();
        copy.id = Uuid::new_v4().to_string();
        copy.name = format!("{} copy", copy.name);
        copy.builtin = false;
        let new_id = copy.id.clone();
        self.operations.push(copy);
        Some(new_id)
    }

    /// Case-insensitive search over names, materials and notes.
    pub fn search(&self, query: &str) -> Vec<String> {
        let query = query.trim().to_lowercase();
        if query.is_empty() {
            return Vec::new();
        }
        let mut matches: Vec<String> = self
            .materials
            .iter()
            .filter(|profile| {
                format!("{} {} {}", profile.name, profile.material, profile.notes)
                    .to_lowercase()
                    .contains(&query)
            })
            .map(|profile| profile.name.clone())
            .collect();
        matches.extend(
            self.operations
                .iter()
                .filter(|profile| {
                    format!("{} {}", profile.name, profile.notes)
                        .to_lowercase()
                        .contains(&query)
                })
                .map(|profile| profile.name.clone()),
        );
        matches
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtins_are_valid_and_marked() {
        let library = ProfileLibrary::with_builtins();
        assert!(library.validate().is_ok());
        assert_ne!(library.materials.len(), 0);
        assert_ne!(library.operations.len(), 0);
        assert!(library.materials.iter().all(|profile| profile.builtin));
        assert!(library.operations.iter().all(|profile| profile.builtin));
    }

    #[test]
    fn rejects_invalid_material() {
        let mut library = ProfileLibrary::empty();
        library.materials.push(MaterialProfile {
            id: "a".to_string(),
            name: String::new(),
            material: "Wood".to_string(),
            thickness_mm: 3.0,
            notes: String::new(),
            builtin: false,
        });
        assert!(library.validate().is_err());
    }

    #[test]
    fn rejects_duplicate_ids() {
        let mut library = ProfileLibrary::empty();
        let profile = MaterialProfile::new("A", "Wood", 3.0);
        library.materials.push(profile.clone());
        library.materials.push(profile);
        assert!(library.validate().is_err());
    }

    #[test]
    fn builtins_cannot_be_removed() {
        let mut library = ProfileLibrary::with_builtins();
        let id = library.materials[0].id.clone();
        assert!(library.remove_material(&id).is_err());
        assert!(library.materials.iter().any(|profile| profile.id == id));
    }

    #[test]
    fn user_profiles_can_be_added_and_removed() {
        let mut library = ProfileLibrary::with_builtins();
        let profile = library.add_material(MaterialProfile::new("Birch 4", "Birch", 4.0));
        let id = profile.id.clone();
        assert!(!profile.builtin);
        assert!(library.remove_material(&id).is_ok());
        assert!(library.remove_material(&id).is_err());
    }

    #[test]
    fn duplicate_creates_user_copy() {
        let mut library = ProfileLibrary::with_builtins();
        let id = library.materials[0].id.clone();
        let copy_id = library.duplicate_material(&id).unwrap();
        assert_ne!(copy_id, id);
        let copy = library.material(&copy_id).unwrap();
        assert!(!copy.builtin);
        assert!(copy.name.ends_with("copy"));
        assert!(library.duplicate_material("missing").is_none());
    }

    #[test]
    fn search_matches_material_and_operation() {
        let library = ProfileLibrary::with_builtins();
        assert_eq!(library.search("plywood").len(), 1);
        assert_eq!(library.search("cut").len(), 1);
        assert_eq!(library.search("   ").len(), 0);
        assert_eq!(library.search("nonexistent").len(), 0);
    }

    #[test]
    fn rejects_bad_operation_power() {
        let mut library = ProfileLibrary::empty();
        let mut profile = OperationProfile::new("Cut", OperationType::OutlineCutting);
        profile.params.power = 150.0;
        library.operations.push(profile);
        assert!(library.validate().is_err());
    }
}
