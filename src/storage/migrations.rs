/// Migration version tracking (PLAN.md §20.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Migration {
    pub version: u32,
    pub applied: bool,
}

impl Migration {
    pub fn new(version: u32) -> Self {
        Self {
            version,
            applied: false,
        }
    }
    pub fn apply(&mut self) {
        self.applied = true;
    }
}

#[derive(Debug, Default)]
pub struct MigrationRunner {
    pub migrations: Vec<Migration>,
}

impl MigrationRunner {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn add(&mut self, m: Migration) {
        self.migrations.push(m);
    }
    pub fn pending(&self) -> Vec<u32> {
        self.migrations
            .iter()
            .filter(|m| !m.applied)
            .map(|m| m.version)
            .collect()
    }
    pub fn apply_all(&mut self) {
        for m in &mut self.migrations {
            m.apply();
        }
    }
    pub fn current_version(&self) -> u32 {
        self.migrations
            .iter()
            .filter(|m| m.applied)
            .map(|m| m.version)
            .max()
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::Migration;

    #[test]
    fn new_not_applied() {
        let m = Migration::new(1);
        assert_eq!(m.version, 1);
        assert!(!m.applied);
    }

    #[test]
    fn apply_sets_flag() {
        let mut m = Migration::new(1);
        m.apply();
        assert!(m.applied);
    }

    #[test]
    fn versions_distinct() {
        let a = Migration::new(1);
        let b = Migration::new(2);
        assert_ne!(a.version, b.version);
    }
}

#[cfg(test)]
mod runner_tests {
    use super::{Migration, MigrationRunner};

    #[test]
    fn pending_and_apply() {
        let mut r = MigrationRunner::new();
        r.add(Migration::new(1));
        r.add(Migration::new(2));
        assert_eq!(r.pending(), vec![1, 2]);
        r.apply_all();
        assert_eq!(r.pending(), [] as [u32; 0]);
        assert_eq!(r.current_version(), 2);
    }

    #[test]
    fn empty_runner() {
        let r = MigrationRunner::new();
        assert_eq!(r.pending(), [] as [u32; 0]);
        assert_eq!(r.current_version(), 0);
    }

    #[test]
    fn partial() {
        let mut r = MigrationRunner::new();
        let mut m = Migration::new(1);
        m.apply();
        r.add(m);
        r.add(Migration::new(2));
        assert_eq!(r.pending(), vec![2]);
        assert_eq!(r.current_version(), 1);
    }
}
