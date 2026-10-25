use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct RecoveryInfo {
    pub original: PathBuf,
    pub backup: PathBuf,
    pub recovered: bool,
}

pub fn recovery_backup_path(original: &Path) -> PathBuf {
    let mut p = original.to_path_buf();
    let name = p
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("project")
        .to_string();
    p.set_file_name(format!("{name}.bak"));
    p
}

pub fn has_backup(original: &Path) -> bool {
    recovery_backup_path(original).exists()
}

pub fn recover(original: &Path) -> std::io::Result<RecoveryInfo> {
    let backup = recovery_backup_path(original);
    if !backup.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "no backup",
        ));
    }
    std::fs::copy(&backup, original)?;
    Ok(RecoveryInfo {
        original: original.to_path_buf(),
        backup,
        recovered: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env::temp_dir;

    #[test]
    fn backup_path() {
        let p = temp_dir().join("proj.bwp");
        let b = recovery_backup_path(&p);
        assert!(b.to_string_lossy().contains(".bak"));
    }

    #[test]
    fn has_backup_false() {
        let p = temp_dir().join("nope.bwp");
        let _ = std::fs::remove_file(recovery_backup_path(&p));
        assert!(!has_backup(&p));
    }

    #[test]
    fn recover_ok() {
        let dir = temp_dir();
        let orig = dir.join("bw_rec.bwp");
        let bak = recovery_backup_path(&orig);
        std::fs::write(&bak, "data").unwrap();
        let info = recover(&orig).unwrap();
        assert!(info.recovered);
        assert_eq!(std::fs::read_to_string(&orig).unwrap(), "data");
        let _ = std::fs::remove_file(&orig);
        let _ = std::fs::remove_file(&bak);
    }

    #[test]
    fn recover_missing() {
        let p = temp_dir().join("bw_rec_missing.bwp");
        let _ = std::fs::remove_file(recovery_backup_path(&p));
        assert!(recover(&p).is_err());
    }
}
