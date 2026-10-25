/// Offset parameters (PLAN.md §6.8, §18).
#[derive(Debug, Clone, Copy)]
pub struct OffsetParams {
    pub distance: f64,
    pub join_type: JoinType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JoinType {
    Miter,
    Round,
    Bevel,
}

impl Default for OffsetParams {
    fn default() -> Self {
        Self {
            distance: 1.0,
            join_type: JoinType::Miter,
        }
    }
}

impl OffsetParams {
    pub fn validate(&self) -> std::result::Result<(), String> {
        if !self.distance.is_finite() {
            return Err("distance must be finite".to_string());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_valid() {
        assert!(OffsetParams::default().validate().is_ok());
    }

    #[test]
    fn nan_distance_invalid() {
        let p = OffsetParams {
            distance: f64::NAN,
            ..OffsetParams::default()
        };
        assert!(p.validate().is_err());
    }

    #[test]
    fn join_types_distinct() {
        assert_ne!(JoinType::Miter, JoinType::Round);
        assert_ne!(JoinType::Round, JoinType::Bevel);
    }
}
