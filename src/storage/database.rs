/// SQLite database wrapper (PLAN.md §20). rusqlite is not yet available in
/// the local cache; this is a stub that keeps the module shape stable.
#[derive(Debug, Default)]
pub struct Database {
    pub path: String,
}

impl Database {
    pub fn open(path: &str) -> Self {
        Self {
            path: path.to_string(),
        }
    }
    pub fn is_open(&self) -> bool {
        !self.path.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::Database;

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
}
