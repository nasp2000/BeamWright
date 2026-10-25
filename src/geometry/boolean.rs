/// Boolean operation kinds (PLAN.md §6.8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BooleanOp {
    Union,
    Difference,
    Intersection,
    Xor,
}

impl BooleanOp {
    pub fn label(self) -> &'static str {
        match self {
            BooleanOp::Union => "Union",
            BooleanOp::Difference => "Difference",
            BooleanOp::Intersection => "Intersection",
            BooleanOp::Xor => "Xor",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::BooleanOp;

    #[test]
    fn labels() {
        assert_eq!(BooleanOp::Union.label(), "Union");
        assert_eq!(BooleanOp::Difference.label(), "Difference");
        assert_eq!(BooleanOp::Intersection.label(), "Intersection");
        assert_eq!(BooleanOp::Xor.label(), "Xor");
    }

    #[test]
    fn equality() {
        assert_eq!(BooleanOp::Union, BooleanOp::Union);
        assert_ne!(BooleanOp::Union, BooleanOp::Xor);
    }

    #[test]
    fn copy() {
        let a = BooleanOp::Intersection;
        let b = a;
        assert_eq!(a, b);
    }
}
