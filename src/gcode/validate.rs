use crate::gcode::toolpath::Toolpath;

#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub ok: bool,
    pub errors: Vec<String>,
}

impl ValidationResult {
    pub fn valid() -> Self {
        Self {
            ok: true,
            errors: Vec::new(),
        }
    }
}

pub fn validate_toolpaths(
    toolpaths: &[Toolpath],
    max_feed: f64,
    max_power: f64,
) -> ValidationResult {
    let mut errors = Vec::new();
    for (i, tp) in toolpaths.iter().enumerate() {
        if tp.speed <= 0.0 || tp.speed > max_feed {
            errors.push(format!("operation {i}: feed rate out of range"));
        }
        if tp.power < 0.0 || tp.power > max_power {
            errors.push(format!("operation {i}: power out of range"));
        }
        if tp.moves.is_empty() {
            errors.push(format!("operation {i}: no geometry"));
        }
    }
    let ok = errors.is_empty();
    ValidationResult { ok, errors }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gcode::toolpath::{MoveKind, Toolpath};

    #[test]
    fn valid() {
        let mut tp = Toolpath::new(1000.0, 50.0, 1);
        tp.push(MoveKind::Cut, 1.0, 1.0);
        let r = validate_toolpaths(&[tp], 2000.0, 100.0);
        assert!(r.ok);
        assert_eq!(r.errors, [] as [String; 0]);
    }

    #[test]
    fn feed_out_of_range() {
        let mut tp = Toolpath::new(0.0, 50.0, 1);
        tp.push(MoveKind::Cut, 1.0, 1.0);
        let r = validate_toolpaths(&[tp], 2000.0, 100.0);
        assert!(!r.ok);
        assert_ne!(r.errors, [] as [String; 0]);
    }

    #[test]
    fn power_out_of_range() {
        let mut tp = Toolpath::new(1000.0, 200.0, 1);
        tp.push(MoveKind::Cut, 1.0, 1.0);
        let r = validate_toolpaths(&[tp], 2000.0, 100.0);
        assert!(!r.ok);
    }

    #[test]
    fn empty_geometry() {
        let tp = Toolpath::new(1000.0, 50.0, 1);
        let r = validate_toolpaths(&[tp], 2000.0, 100.0);
        assert!(!r.ok);
    }
}
