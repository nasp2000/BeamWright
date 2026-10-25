pub mod database;
pub mod migrations;
pub mod recovery;

pub fn backup_file(src: &std::path::Path, dst: &std::path::Path) -> crate::errors::Result<()> {
    use std::fs;
    if !src.exists() {
        return Err(crate::errors::BeamError::InvalidInput(
            "source does not exist".into(),
        ));
    }
    let tmp = dst.with_extension("bak.tmp");
    fs::copy(src, &tmp)?;
    fs::rename(&tmp, dst)?;
    Ok(())
}

pub fn restore_file(src: &std::path::Path, dst: &std::path::Path) -> crate::errors::Result<()> {
    use std::fs;
    if !src.exists() {
        return Err(crate::errors::BeamError::InvalidInput(
            "backup does not exist".into(),
        ));
    }
    fs::copy(src, dst)?;
    Ok(())
}

#[allow(unused_imports)]
pub use database::Database;
#[allow(unused_imports)]
pub use migrations::Migration;
#[allow(unused_imports)]
pub use migrations::MigrationRunner;

#[allow(unused_imports)]
pub use recovery::{RecoveryInfo, has_backup, recover, recovery_backup_path};

#[cfg(test)]
mod tests {
    use super::*;
    use std::env::temp_dir;

    #[test]
    fn backup_and_restore() {
        let dir = temp_dir();
        let src = dir.join("bw_src2.txt");
        let bak = dir.join("bw_bak2.txt");
        let restored = dir.join("bw_restored2.txt");
        std::fs::write(&src, "hello").unwrap();
        backup_file(&src, &bak).unwrap();
        assert!(bak.exists());
        restore_file(&bak, &restored).unwrap();
        assert_eq!(std::fs::read_to_string(&restored).unwrap(), "hello");
        let _ = std::fs::remove_file(&src);
        let _ = std::fs::remove_file(&bak);
        let _ = std::fs::remove_file(&restored);
    }

    #[test]
    fn backup_missing_source_errors() {
        let dir = temp_dir();
        let src = dir.join("bw_missing3.txt");
        let bak = dir.join("bw_bak3.txt");
        let _ = std::fs::remove_file(&src);
        assert!(backup_file(&src, &bak).is_err());
    }

    #[test]
    fn restore_missing_backup_errors() {
        let dir = temp_dir();
        let src = dir.join("bw_missing4.txt");
        let dst = dir.join("bw_dst4.txt");
        let _ = std::fs::remove_file(&src);
        assert!(restore_file(&src, &dst).is_err());
    }
}
