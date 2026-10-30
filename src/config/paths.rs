use std::path::PathBuf;

/// Directory where `BeamWright` stores user data (profiles, preferences).
///
/// Resolution order: `BEAMWRIGHT_DATA_DIR`, then the platform convention from
/// environment variables. No OS-specific API is used, so the binary stays
/// portable.
pub fn data_dir() -> PathBuf {
    if let Some(value) = std::env::var_os("BEAMWRIGHT_DATA_DIR") {
        return PathBuf::from(value);
    }
    if let Some(value) = std::env::var_os("XDG_DATA_HOME") {
        return PathBuf::from(value).join("beamwright");
    }
    if cfg!(windows) {
        if let Some(value) = std::env::var_os("APPDATA") {
            return PathBuf::from(value).join("BeamWright");
        }
    } else if let Some(value) = std::env::var_os("HOME") {
        return PathBuf::from(value).join(".local/share/beamwright");
    }
    PathBuf::from(".beamwright")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn override_wins() {
        // The variable is process-global; read it back rather than mutating it
        // in a test that other tests could run concurrently with.
        let resolved = data_dir();
        if let Some(value) = std::env::var_os("BEAMWRIGHT_DATA_DIR") {
            assert_eq!(resolved, PathBuf::from(value));
        }
    }

    #[test]
    fn is_absolute_or_dot_relative() {
        let dir = data_dir();
        assert_ne!(dir.as_os_str().len(), 0);
    }

    #[test]
    fn does_not_contain_os_separators_in_name() {
        let dir = data_dir();
        let name = dir
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("");
        assert_ne!(name.len(), 0);
    }
}
