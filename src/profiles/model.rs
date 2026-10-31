use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::operations::{OperationParams, OperationType};

/// Format version of the profile library file.
///
/// Version 2 adds optional metadata (category, tags, favourite and timestamps).
/// Version 1 files load unchanged because every new field has a default.
pub const PROFILE_FORMAT_VERSION: u32 = 2;

/// Version written by earlier releases and still accepted when loading.
pub const LEGACY_PROFILE_FORMAT_VERSION: u32 = 1;

/// Initial material categories suggested by the interface.
pub const DEFAULT_CATEGORIES: [&str; 12] = [
    "Wood",
    "Plywood",
    "MDF",
    "Acrylic",
    "Paper",
    "Cardboard",
    "Leather",
    "Fabric",
    "Rubber",
    "Glass",
    "Coated metal",
    "Other",
];

/// Fixed timestamp used by the profiles shipped with the application, so the
/// built-in library is identical on every run.
const BUILTIN_TIMESTAMP: &str = "2024-01-01T00:00:00Z";

/// Metadata shared by every profile type (PLAN.md §11.3, §11.4, §11.7).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProfileMeta {
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub favorite: bool,
    /// RFC 3339 UTC creation time; empty for files written before metadata.
    #[serde(default)]
    pub created_at: String,
    /// RFC 3339 UTC time of the last edit; empty when never edited.
    #[serde(default)]
    pub modified_at: String,
}

impl Default for ProfileMeta {
    fn default() -> Self {
        Self::new()
    }
}

impl ProfileMeta {
    /// New metadata stamped with the current UTC time.
    pub fn new() -> Self {
        let now = now_utc();
        Self {
            category: String::new(),
            tags: Vec::new(),
            favorite: false,
            created_at: now.clone(),
            modified_at: now,
        }
    }

    /// Metadata with an explicit category, for built-in profiles.
    pub fn with_category(category: impl Into<String>) -> Self {
        Self {
            category: category.into(),
            ..Self::builtin()
        }
    }

    /// Metadata for a profile shipped with the application.
    pub fn builtin() -> Self {
        Self {
            category: String::new(),
            tags: Vec::new(),
            favorite: false,
            created_at: BUILTIN_TIMESTAMP.to_string(),
            modified_at: BUILTIN_TIMESTAMP.to_string(),
        }
    }

    /// Record the current time as the last modification.
    pub fn touch(&mut self) {
        self.modified_at = now_utc();
    }

    /// Tags as one comma-separated line for editing.
    pub fn tag_text(&self) -> String {
        self.tags.join(", ")
    }

    /// Replace the tags from a comma-separated line, trimming, dropping empty
    /// entries and ignoring case-insensitive duplicates.
    pub fn set_tag_text(&mut self, text: &str) {
        let mut tags: Vec<String> = Vec::new();
        for tag in text.split(',').map(str::trim).filter(|tag| !tag.is_empty()) {
            if tags
                .iter()
                .all(|existing| !existing.eq_ignore_ascii_case(tag))
            {
                tags.push(tag.to_string());
            }
        }
        self.tags = tags;
    }

    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags
            .iter()
            .any(|existing| existing.eq_ignore_ascii_case(tag.trim()))
    }

    /// Whether the category or any tag contains `query` (already lowercase).
    pub fn matches(&self, query: &str) -> bool {
        self.category.to_lowercase().contains(query)
            || self
                .tags
                .iter()
                .any(|tag| tag.to_lowercase().contains(query))
    }

    pub fn validate(&self) -> Result<(), String> {
        for tag in &self.tags {
            if tag.trim().is_empty() {
                return Err("profile tags must not be empty".to_string());
            }
            if tag.trim() != tag {
                return Err(format!("profile tag '{tag}' has surrounding spaces"));
            }
        }
        let mut seen = std::collections::HashSet::new();
        for tag in &self.tags {
            if !seen.insert(tag.to_lowercase()) {
                return Err(format!("duplicate profile tag: {tag}"));
            }
        }
        for timestamp in [&self.created_at, &self.modified_at] {
            if !timestamp.is_empty() && !is_utc_timestamp(timestamp) {
                return Err(format!("invalid profile timestamp: {timestamp}"));
            }
        }
        Ok(())
    }
}

/// Format a Unix timestamp as `YYYY-MM-DDTHH:MM:SSZ`.
pub fn format_utc(seconds: u64) -> String {
    let days = i64::try_from(seconds / 86_400).unwrap_or(i64::MAX);
    let remainder = seconds % 86_400;
    let (year, month, day) = civil_from_days(days);
    let hour = remainder / 3_600;
    let minute = (remainder % 3_600) / 60;
    let second = remainder % 60;
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

/// Current time in the same format.
pub fn now_utc() -> String {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs());
    format_utc(seconds)
}

/// Convert days since 1970-01-01 to a civil date (Howard Hinnant's algorithm).
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = u32::try_from(day_of_year - (153 * month_prime + 2) / 5 + 1).unwrap_or(1);
    let month = u32::try_from(if month_prime < 10 {
        month_prime + 3
    } else {
        month_prime - 9
    })
    .unwrap_or(1);
    (if month <= 2 { year + 1 } else { year }, month, day)
}

/// Whether `value` looks like `YYYY-MM-DDTHH:MM:SSZ`.
fn is_utc_timestamp(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() != 20 {
        return false;
    }
    let digits = [0, 1, 2, 3, 5, 6, 8, 9, 11, 12, 14, 15, 17, 18];
    let separators = [
        (4, b'-'),
        (7, b'-'),
        (10, b'T'),
        (13, b':'),
        (16, b':'),
        (19, b'Z'),
    ];
    digits.iter().all(|index| bytes[*index].is_ascii_digit())
        && separators
            .iter()
            .all(|(index, expected)| bytes[*index] == *expected)
}

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
    #[serde(flatten)]
    pub meta: ProfileMeta,
    /// Operation profiles recommended for this material.
    #[serde(default)]
    pub recommended_operations: Vec<String>,
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
            meta: ProfileMeta::new(),
            recommended_operations: Vec::new(),
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        self.meta.validate()?;
        for (index, operation) in self.recommended_operations.iter().enumerate() {
            if operation.trim().is_empty() {
                return Err("recommended operation ids must not be empty".to_string());
            }
            if self.recommended_operations[..index].contains(operation) {
                return Err(format!("duplicate recommended operation: {operation}"));
            }
        }
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
    #[serde(flatten)]
    pub meta: ProfileMeta,
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
            meta: ProfileMeta::new(),
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        self.meta.validate()?;
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

/// Reusable image preparation settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImageProfile {
    pub id: String,
    pub name: String,
    pub settings: crate::objects::ImageSettings,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub builtin: bool,
    #[serde(flatten)]
    pub meta: ProfileMeta,
}

impl ImageProfile {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: name.into(),
            settings: crate::objects::ImageSettings::default(),
            notes: String::new(),
            builtin: false,
            meta: ProfileMeta::new(),
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        self.meta.validate()?;
        if self.id.trim().is_empty() {
            return Err("image profile id must not be empty".to_string());
        }
        if self.name.trim().is_empty() {
            return Err("image profile name must not be empty".to_string());
        }
        self.settings.validate()
    }
}

/// Reusable project defaults (work area, grid and snap).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectProfile {
    pub id: String,
    pub name: String,
    pub width_mm: f64,
    pub height_mm: f64,
    pub grid_spacing_mm: f64,
    pub snap_mm: f64,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub builtin: bool,
    #[serde(flatten)]
    pub meta: ProfileMeta,
}

impl ProjectProfile {
    pub fn new(name: impl Into<String>, width_mm: f64, height_mm: f64) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: name.into(),
            width_mm,
            height_mm,
            grid_spacing_mm: 10.0,
            snap_mm: 1.0,
            notes: String::new(),
            builtin: false,
            meta: ProfileMeta::new(),
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        self.meta.validate()?;
        if self.id.trim().is_empty() {
            return Err("project profile id must not be empty".to_string());
        }
        if self.name.trim().is_empty() {
            return Err("project profile name must not be empty".to_string());
        }
        if !self.width_mm.is_finite() || self.width_mm <= 0.0 {
            return Err("project width must be positive".to_string());
        }
        if !self.height_mm.is_finite() || self.height_mm <= 0.0 {
            return Err("project height must be positive".to_string());
        }
        if !self.grid_spacing_mm.is_finite() || self.grid_spacing_mm <= 0.0 {
            return Err("grid spacing must be positive".to_string());
        }
        if !self.snap_mm.is_finite() || self.snap_mm <= 0.0 {
            return Err("snap step must be positive".to_string());
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
    #[serde(default)]
    pub images: Vec<ImageProfile>,
    #[serde(default)]
    pub projects: Vec<ProjectProfile>,
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
            images: Vec::new(),
            projects: Vec::new(),
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
            meta: ProfileMeta::with_category("Plywood"),
            recommended_operations: Vec::new(),
        });
        library.materials.push(MaterialProfile {
            id: "builtin-acrylic-3".to_string(),
            name: "Acrylic 3 mm".to_string(),
            material: "Acrylic".to_string(),
            thickness_mm: 3.0,
            notes: "Reference values only; test on your own material.".to_string(),
            builtin: true,
            meta: ProfileMeta::with_category("Acrylic"),
            recommended_operations: Vec::new(),
        });
        library.materials.push(MaterialProfile {
            id: "builtin-cardboard-2".to_string(),
            name: "Cardboard 2 mm".to_string(),
            material: "Cardboard".to_string(),
            thickness_mm: 2.0,
            notes: "Reference values only; test on your own material.".to_string(),
            builtin: true,
            meta: ProfileMeta::with_category("Cardboard"),
            recommended_operations: Vec::new(),
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
                ..OperationParams::default()
            },
            notes: "Reference values only; test on your own material.".to_string(),
            builtin: true,
            meta: ProfileMeta::with_category("Cutting"),
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
                ..OperationParams::default()
            },
            notes: "Reference values only; test on your own material.".to_string(),
            builtin: true,
            meta: ProfileMeta::with_category("Engraving"),
        });
        library.images.push(ImageProfile {
            id: "builtin-photo".to_string(),
            name: "Photo engraving".to_string(),
            settings: crate::objects::ImageSettings {
                mode: crate::objects::ImageMode::FloydSteinberg,
                threshold: 128,
                brightness: 0,
                contrast: 1.1,
                gamma: 1.0,
                invert: false,
                line_interval_mm: 0.1,
                ..crate::objects::ImageSettings::default()
            },
            notes: "Reference values only; test on your own material.".to_string(),
            builtin: true,
            meta: ProfileMeta::with_category("Photo"),
        });
        library.projects.push(ProjectProfile {
            id: "builtin-a3".to_string(),
            name: "A3 landscape (420 × 297)".to_string(),
            width_mm: 420.0,
            height_mm: 297.0,
            grid_spacing_mm: 10.0,
            snap_mm: 1.0,
            notes: String::new(),
            builtin: true,
            meta: ProfileMeta::with_category("Work area"),
        });
        library
    }

    pub fn validate(&self) -> Result<(), String> {
        if !(LEGACY_PROFILE_FORMAT_VERSION..=PROFILE_FORMAT_VERSION).contains(&self.format_version)
        {
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
        for image in &self.images {
            image.validate()?;
            if !ids.insert(image.id.as_str()) {
                return Err(format!("duplicate profile id: {}", image.id));
            }
        }
        for project in &self.projects {
            project.validate()?;
            if !ids.insert(project.id.as_str()) {
                return Err(format!("duplicate profile id: {}", project.id));
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

    pub fn image(&self, id: &str) -> Option<&ImageProfile> {
        self.images.iter().find(|profile| profile.id == id)
    }

    pub fn project(&self, id: &str) -> Option<&ProjectProfile> {
        self.projects.iter().find(|profile| profile.id == id)
    }

    /// Metadata of any profile type, for editing category, tags or favourite.
    pub fn meta_mut(&mut self, id: &str) -> Option<&mut ProfileMeta> {
        if let Some(profile) = self.materials.iter_mut().find(|profile| profile.id == id) {
            return Some(&mut profile.meta);
        }
        if let Some(profile) = self.operations.iter_mut().find(|profile| profile.id == id) {
            return Some(&mut profile.meta);
        }
        if let Some(profile) = self.images.iter_mut().find(|profile| profile.id == id) {
            return Some(&mut profile.meta);
        }
        self.projects
            .iter_mut()
            .find(|profile| profile.id == id)
            .map(|profile| &mut profile.meta)
    }

    /// Whether any profile carries this identifier.
    pub fn contains(&self, id: &str) -> bool {
        self.material(id).is_some()
            || self.operation(id).is_some()
            || self.image(id).is_some()
            || self.project(id).is_some()
    }

    /// Mark a profile as a favourite (or clear the mark).
    pub fn set_favorite(&mut self, id: &str, favorite: bool) -> Result<(), String> {
        let meta = self
            .meta_mut(id)
            .ok_or_else(|| format!("unknown profile: {id}"))?;
        if meta.favorite != favorite {
            meta.favorite = favorite;
            meta.touch();
        }
        Ok(())
    }

    /// Identifiers of every profile marked as a favourite.
    pub fn favorites(&self) -> Vec<String> {
        let mut ids: Vec<String> = Vec::new();
        ids.extend(
            self.materials
                .iter()
                .filter(|profile| profile.meta.favorite)
                .map(|profile| profile.id.clone()),
        );
        ids.extend(
            self.operations
                .iter()
                .filter(|profile| profile.meta.favorite)
                .map(|profile| profile.id.clone()),
        );
        ids.extend(
            self.images
                .iter()
                .filter(|profile| profile.meta.favorite)
                .map(|profile| profile.id.clone()),
        );
        ids.extend(
            self.projects
                .iter()
                .filter(|profile| profile.meta.favorite)
                .map(|profile| profile.id.clone()),
        );
        ids
    }

    /// Distinct categories in use, sorted for display.
    pub fn categories(&self) -> Vec<String> {
        let mut categories: Vec<String> = self
            .materials
            .iter()
            .map(|profile| &profile.meta.category)
            .chain(self.operations.iter().map(|profile| &profile.meta.category))
            .chain(self.images.iter().map(|profile| &profile.meta.category))
            .chain(self.projects.iter().map(|profile| &profile.meta.category))
            .filter(|category| !category.trim().is_empty())
            .cloned()
            .collect();
        categories.sort_by_key(|category| category.to_lowercase());
        categories.dedup();
        categories
    }

    pub fn add_image(&mut self, mut profile: ImageProfile) -> &ImageProfile {
        profile.builtin = false;
        self.images.push(profile);
        self.images.last().expect("profile was just pushed")
    }

    pub fn add_project(&mut self, mut profile: ProjectProfile) -> &ProjectProfile {
        profile.builtin = false;
        self.projects.push(profile);
        self.projects.last().expect("profile was just pushed")
    }

    pub fn remove_image(&mut self, id: &str) -> Result<(), String> {
        let index = self
            .images
            .iter()
            .position(|profile| profile.id == id)
            .ok_or_else(|| format!("unknown image profile: {id}"))?;
        if self.images[index].builtin {
            return Err("built-in profiles cannot be removed".to_string());
        }
        self.images.remove(index);
        Ok(())
    }

    pub fn remove_project(&mut self, id: &str) -> Result<(), String> {
        let index = self
            .projects
            .iter()
            .position(|profile| profile.id == id)
            .ok_or_else(|| format!("unknown project profile: {id}"))?;
        if self.projects[index].builtin {
            return Err("built-in profiles cannot be removed".to_string());
        }
        self.projects.remove(index);
        Ok(())
    }

    pub fn duplicate_image(&mut self, id: &str) -> Option<String> {
        let mut copy = self.image(id)?.clone();
        copy.id = Uuid::new_v4().to_string();
        copy.name = format!("{} copy", copy.name);
        copy.builtin = false;
        let new_id = copy.id.clone();
        self.images.push(copy);
        Some(new_id)
    }

    pub fn duplicate_project(&mut self, id: &str) -> Option<String> {
        let mut copy = self.project(id)?.clone();
        copy.id = Uuid::new_v4().to_string();
        copy.name = format!("{} copy", copy.name);
        copy.builtin = false;
        let new_id = copy.id.clone();
        self.projects.push(copy);
        Some(new_id)
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
                    || profile.meta.matches(&query)
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
                        || profile.meta.matches(&query)
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
            meta: ProfileMeta::default(),
            recommended_operations: Vec::new(),
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
    fn image_and_project_profiles_are_validated() {
        let mut library = ProfileLibrary::empty();
        library.images.push(ImageProfile::new("Dither"));
        library
            .projects
            .push(ProjectProfile::new("Small", 100.0, 80.0));
        assert!(library.validate().is_ok());
        library.projects[0].width_mm = 0.0;
        assert!(library.validate().is_err());
    }

    #[test]
    fn image_profile_settings_are_checked() {
        let mut library = ProfileLibrary::empty();
        let mut profile = ImageProfile::new("Photo");
        profile.settings.gamma = 0.0;
        library.images.push(profile);
        assert!(library.validate().is_err());
    }

    #[test]
    fn image_and_project_profiles_can_be_duplicated() {
        let mut library = ProfileLibrary::with_builtins();
        let image_id = library.images[0].id.clone();
        let copy = library.duplicate_image(&image_id).unwrap();
        assert!(!library.image(&copy).unwrap().builtin);
        let project_id = library.projects[0].id.clone();
        let copy = library.duplicate_project(&project_id).unwrap();
        assert!(!library.project(&copy).unwrap().builtin);
        assert!(library.remove_image(&image_id).is_err());
        assert!(library.remove_project(&project_id).is_err());
    }

    #[test]
    fn timestamps_format_known_instants() {
        assert_eq!(format_utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(format_utc(951_782_400), "2000-02-29T00:00:00Z");
        assert_eq!(format_utc(1_700_000_000), "2023-11-14T22:13:20Z");
    }

    #[test]
    fn new_profiles_are_stamped() {
        let profile = MaterialProfile::new("Birch", "Birch", 3.0);
        assert!(is_utc_timestamp(&profile.meta.created_at));
        assert_eq!(profile.meta.created_at, profile.meta.modified_at);
        assert!(is_utc_timestamp(&now_utc()));
    }

    #[test]
    fn tags_are_parsed_and_deduplicated() {
        let mut meta = ProfileMeta::default();
        meta.set_tag_text(" wood, MDF , wood, , 3 mm ");
        assert_eq!(meta.tags, vec!["wood", "MDF", "3 mm"]);
        assert!(meta.has_tag("mdf"));
        assert!(!meta.has_tag("paper"));
        assert_eq!(meta.tag_text(), "wood, MDF, 3 mm");
        assert!(meta.validate().is_ok());
    }

    #[test]
    fn invalid_metadata_is_rejected() {
        let spaced = ProfileMeta {
            tags: vec!["ok".to_string(), " ok ".to_string()],
            ..ProfileMeta::default()
        };
        assert!(spaced.validate().is_err());
        let duplicated = ProfileMeta {
            tags: vec!["ok".to_string(), "OK".to_string()],
            ..ProfileMeta::default()
        };
        assert!(duplicated.validate().is_err());
        let bad_time = ProfileMeta {
            tags: vec!["ok".to_string()],
            created_at: "yesterday".to_string(),
            ..ProfileMeta::default()
        };
        assert!(bad_time.validate().is_err());
    }

    #[test]
    fn favourites_are_stored_and_listed() {
        let mut library = ProfileLibrary::with_builtins();
        let material = library.materials[0].id.clone();
        let operation = library.operations[0].id.clone();
        library.set_favorite(&material, true).unwrap();
        library.set_favorite(&operation, true).unwrap();
        let favourites = library.favorites();
        assert_eq!(favourites.len(), 2);
        assert!(favourites.contains(&material));
        assert!(favourites.contains(&operation));
        library.set_favorite(&material, false).unwrap();
        assert_eq!(library.favorites(), vec![operation]);
        assert!(library.set_favorite("missing", true).is_err());
    }

    #[test]
    fn categories_are_unique_and_sorted() {
        let library = ProfileLibrary::with_builtins();
        let categories = library.categories();
        assert!(categories.contains(&"Plywood".to_string()));
        assert!(categories.contains(&"Acrylic".to_string()));
        let mut sorted = categories.clone();
        sorted.sort_by_key(|category| category.to_lowercase());
        assert_eq!(categories, sorted);
        assert_eq!(
            categories
                .iter()
                .filter(|category| *category == "Plywood")
                .count(),
            1
        );
    }

    #[test]
    fn search_matches_tags_and_category() {
        let mut library = ProfileLibrary::empty();
        let mut profile = MaterialProfile::new("Board", "Pine", 4.0);
        profile.meta.category = "Signage".to_string();
        profile.meta.tags = vec!["stencil".to_string()];
        library.add_material(profile);
        assert_eq!(library.search("signage").len(), 1);
        assert_eq!(library.search("stencil").len(), 1);
        assert_eq!(library.search("missing").len(), 0);
    }

    #[test]
    fn recommended_operations_round_trip_and_validate() {
        let mut profile = MaterialProfile::new("Birch", "Birch", 3.0);
        assert_eq!(profile.recommended_operations.len(), 0);
        profile.recommended_operations = vec!["builtin-cut".to_string()];
        assert!(profile.validate().is_ok());
        let json = serde_json::to_string(&profile).unwrap();
        let loaded: MaterialProfile = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.recommended_operations, vec!["builtin-cut"]);

        profile.recommended_operations = vec![String::new()];
        assert!(profile.validate().is_err());
        profile.recommended_operations = vec!["a".to_string(), "a".to_string()];
        assert!(profile.validate().is_err());

        let legacy: MaterialProfile = serde_json::from_str(
            r#"{"id":"x","name":"Old","material":"Wood","thickness_mm":3.0,"notes":"","builtin":false}"#,
        )
        .unwrap();
        assert_eq!(legacy.recommended_operations.len(), 0);
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
