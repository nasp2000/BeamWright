use std::fs;
use std::path::Path;

use crate::errors::{BeamError, Result};

/// Copy `src` to `dst` atomically (best-effort backup).
pub fn backup_file(src: &Path, dst: &Path) -> Result<()> {
    if !src.exists() {
        return Err(BeamError::InvalidInput("source does not exist".into()));
    }
    let tmp = dst.with_extension("bak.tmp");
    fs::copy(src, &tmp)?;
    fs::rename(&tmp, dst)?;
    Ok(())
}

/// Restore `dst` from `src` if `src` exists.
pub fn restore_file(src: &Path, dst: &Path) -> Result<()> {
    if !src.exists() {
        return Err(BeamError::InvalidInput("backup does not exist".into()));
    }
    fs::copy(src, dst)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env::temp_dir;

    #[test]
    fn backup_and_restore() {
        let dir = temp_dir();
        let src = dir.join("bw_src.txt");
        let bak = dir.join("bw_bak.txt");
        let restored = dir.join("bw_restored.txt");
        fs::write(&src, "hello").unwrap();
        backup_file(&src, &bak).unwrap();
        assert!(bak.exists());
        restore_file(&bak, &restored).unwrap();
        assert_eq!(fs::read_to_string(&restored).unwrap(), "hello");
        let _ = fs::remove_file(&src);
        let _ = fs::remove_file(&bak);
        let _ = fs::remove_file(&restored);
    }

    #[test]
    fn backup_missing_source_errors() {
        let dir = temp_dir();
        let src = dir.join("bw_missing.txt");
        let bak = dir.join("bw_bak2.txt");
        let _ = fs::remove_file(&src);
        assert!(backup_file(&src, &bak).is_err());
    }

    #[test]
    fn restore_missing_backup_errors() {
        let dir = temp_dir();
        let src = dir.join("bw_missing2.txt");
        let dst = dir.join("bw_dst.txt");
        let _ = fs::remove_file(&src);
        assert!(restore_file(&src, &dst).is_err());
    }
}
