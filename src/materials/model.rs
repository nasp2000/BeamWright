use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Material {
    pub id: String,
    pub name: String,
    pub category: String,
    pub thickness_mm: f64,
    pub color: String,
    pub notes: String,
}

impl Material {
    pub fn validate(&self) -> Result<(), String> {
        if self.name.trim().is_empty() {
            return Err("material name must not be empty".to_string());
        }
        if self.id.is_empty() {
            return Err("material id must not be empty".to_string());
        }
        if !self.thickness_mm.is_finite() || self.thickness_mm <= 0.0 {
            return Err("thickness must be positive".to_string());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn sample() -> Material {
        Material {
            id: Uuid::new_v4().to_string(),
            name: "Plywood".to_string(),
            category: "Wood".to_string(),
            thickness_mm: 3.0,
            color: "#8B4513".to_string(),
            notes: String::new(),
        }
    }

    #[test]
    fn valid() {
        assert!(sample().validate().is_ok());
    }

    #[test]
    fn reject_empty_name() {
        let mut m = sample();
        m.name = String::new();
        assert!(m.validate().is_err());
    }

    #[test]
    fn reject_bad_thickness() {
        for t in [0.0_f64, -1.0, f64::NAN] {
            let mut m = sample();
            m.thickness_mm = t;
            assert!(m.validate().is_err());
        }
    }

    #[test]
    fn reject_empty_id() {
        let mut m = sample();
        m.id = String::new();
        assert!(m.validate().is_err());
    }
}
