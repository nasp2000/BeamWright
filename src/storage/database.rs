use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::errors::{BeamError, Result};

/// Simple file-backed key/value store (SQLite substitute until rusqlite
/// is available in the build environment).
#[derive(Debug, Default)]
pub struct Database {
    pub path: String,
    data: HashMap<String, String>,
    loaded: bool,
}

impl Database {
    pub fn open(path: &str) -> Self {
        let mut db = Self {
            path: path.to_string(),
            data: HashMap::new(),
            loaded: false,
        };
        db.load();
        db
    }
    pub fn is_open(&self) -> bool {
        !self.path.is_empty()
    }
    fn load(&mut self) {
        if self.path.is_empty() {
            return;
        }
        if let Ok(s) = fs::read_to_string(&self.path) {
            if let Ok(m) = serde_json::from_str::<HashMap<String, String>>(&s) {
                self.data = m;
            }
        }
        self.loaded = true;
    }
    pub fn get(&self, key: &str) -> Option<&String> {
        self.data.get(key)
    }
    pub fn set(&mut self, key: String, value: String) -> Result<()> {
        self.data.insert(key, value);
        self.flush()
    }
    pub fn remove(&mut self, key: &str) -> Result<()> {
        self.data.remove(key);
        self.flush()
    }
    pub fn flush(&self) -> Result<()> {
        if self.path.is_empty() {
            return Ok(());
        }
        let s = serde_json::to_string(&self.data).map_err(BeamError::Serde)?;
        fs::write(Path::new(&self.path), s)?;
        Ok(())
    }
    pub fn len(&self) -> usize {
        self.data.len()
    }
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::Database;
    use std::env::temp_dir;

    #[test]
    fn open_sets_path() {
        let db = Database::open("test.db");
        assert!(db.is_open());
        assert_eq!(db.path, "test.db");
    }

    #[test]
    fn default_closed() {
        let db = Database::default();
        assert!(!db.is_open());
    }

    #[test]
    fn empty_path_closed() {
        let db = Database::open("");
        assert!(!db.is_open());
    }

    #[test]
    fn set_get_round_trip() {
        let path = temp_dir().join("bw_kv.json");
        let _ = std::fs::remove_file(&path);
        let mut db = Database::open(path.to_str().unwrap());
        db.set("k".to_string(), "v".to_string()).unwrap();
        assert_eq!(db.get("k"), Some(&"v".to_string()));
        let db2 = Database::open(path.to_str().unwrap());
        assert_eq!(db2.get("k"), Some(&"v".to_string()));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn remove() {
        let path = temp_dir().join("bw_kv2.json");
        let _ = std::fs::remove_file(&path);
        let mut db = Database::open(path.to_str().unwrap());
        db.set("k".to_string(), "v".to_string()).unwrap();
        db.remove("k").unwrap();
        assert!(db.get("k").is_none());
        let _ = std::fs::remove_file(&path);
    }
}
