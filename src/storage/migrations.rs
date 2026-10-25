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
