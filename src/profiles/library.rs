use std::path::{Path, PathBuf};

use crate::errors::{BeamError, Result};
use crate::profiles::model::{PROFILE_FORMAT_VERSION, ProfileLibrary};

/// File name used inside the application data directory.
pub const LIBRARY_FILE: &str = "profiles.json";

/// Default location of the profile library.
pub fn default_path() -> PathBuf {
    crate::config::paths::data_dir().join(LIBRARY_FILE)
}

/// Load the library from `path`, or built-ins when the file does not exist.
pub fn load_or_default(path: &Path) -> Result<ProfileLibrary> {
    if !path.exists() {
        return Ok(ProfileLibrary::with_builtins());
    }
    let text = std::fs::read_to_string(path)?;
    let library: ProfileLibrary = serde_json::from_str(&text)?;
    library
        .validate()
        .map_err(|error| BeamError::InvalidInput(format!("invalid profile library: {error}")))?;
    if library.format_version != PROFILE_FORMAT_VERSION {
        return Err(BeamError::InvalidInput(format!(
            "unsupported profile format version {}",
            library.format_version
        )));
    }
    Ok(library)
}

/// Save the library atomically.
pub fn save(library: &ProfileLibrary, path: &Path) -> Result<()> {
    library
        .validate()
        .map_err(|error| BeamError::InvalidInput(format!("invalid profile library: {error}")))?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(library)?;
    let temporary = path.with_extension("json.tmp");
    std::fs::write(&temporary, text)?;
    std::fs::rename(&temporary, path)?;
    Ok(())
}

/// Export one library to a user-chosen file.
pub fn export(library: &ProfileLibrary, path: &Path) -> Result<()> {
    save(library, path)
}

/// Import a library file and merge it into `library`.
///
/// Imported profiles keep their identifiers; collisions are reported instead of
/// silently overwriting existing profiles.
pub fn import_into(library: &mut ProfileLibrary, path: &Path) -> Result<usize> {
    let text = std::fs::read_to_string(path)?;
    let imported: ProfileLibrary = serde_json::from_str(&text)?;
    imported
        .validate()
        .map_err(|error| BeamError::InvalidInput(format!("invalid profile library: {error}")))?;
    let mut added = 0;
    for material in imported.materials {
        if library.material(&material.id).is_none() {
            library.materials.push(material);
            added += 1;
        }
    }
    for operation in imported.operations {
        if library.operation(&operation.id).is_none() {
            library.operations.push(operation);
            added += 1;
        }
    }
    Ok(added)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profiles::model::{MaterialProfile, OperationProfile};

    fn temp_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(name)
    }

    #[test]
    fn missing_file_returns_builtins() {
        let path = temp_path("bw_profiles_missing.json");
        let _ = std::fs::remove_file(&path);
        let library = load_or_default(&path).unwrap();
        assert!(library.materials.iter().any(|profile| profile.builtin));
    }

    #[test]
    fn save_and_load_round_trip() {
        let path = temp_path("bw_profiles_roundtrip.json");
        let _ = std::fs::remove_file(&path);
        let mut library = ProfileLibrary::with_builtins();
        library.add_material(MaterialProfile::new("Birch 5", "Birch", 5.0));
        library.add_operation(OperationProfile::new(
            "Score",
            crate::operations::OperationType::Line,
        ));
        save(&library, &path).unwrap();
        let loaded = load_or_default(&path).unwrap();
        assert_eq!(loaded, library);
        assert!(!path.with_extension("json.tmp").exists());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn invalid_file_is_rejected() {
        let path = temp_path("bw_profiles_invalid.json");
        std::fs::write(&path, "{not json").unwrap();
        assert!(load_or_default(&path).is_err());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn import_merges_without_overwriting() {
        let path = temp_path("bw_profiles_import.json");
        let mut source = ProfileLibrary::empty();
        source
            .materials
            .push(MaterialProfile::new("Imported", "MDF", 6.0));
        source.operations.push(OperationProfile::new(
            "Imported cut",
            crate::operations::OperationType::Fill,
        ));
        save(&source, &path).unwrap();

        let mut target = ProfileLibrary::with_builtins();
        let added = import_into(&mut target, &path).unwrap();
        assert_eq!(added, 2);
        // Importing again adds nothing.
        assert_eq!(import_into(&mut target, &path).unwrap(), 0);
        assert!(
            target
                .materials
                .iter()
                .any(|profile| profile.name == "Imported")
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn save_rejects_invalid_library() {
        let path = temp_path("bw_profiles_bad.json");
        let mut library = ProfileLibrary::empty();
        library.materials.push(MaterialProfile {
            id: "x".to_string(),
            name: String::new(),
            material: "Wood".to_string(),
            thickness_mm: 3.0,
            notes: String::new(),
            builtin: false,
        });
        assert!(save(&library, &path).is_err());
        let _ = std::fs::remove_file(&path);
    }
}
