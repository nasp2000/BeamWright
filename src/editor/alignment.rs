#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlignMode {
    Left,
    Right,
    Top,
    Bottom,
    CenterH,
    CenterV,
}

impl AlignMode {
    pub fn label(self) -> &'static str {
        match self {
            AlignMode::Left => "Left",
            AlignMode::Right => "Right",
            AlignMode::Top => "Top",
            AlignMode::Bottom => "Bottom",
            AlignMode::CenterH => "Center H",
            AlignMode::CenterV => "Center V",
        }
    }
}

#[allow(clippy::cast_precision_loss)]
pub fn align_value(mode: AlignMode, values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    match mode {
        AlignMode::Left | AlignMode::Top => *values
            .iter()
            .min_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap(),
        AlignMode::Right | AlignMode::Bottom => *values
            .iter()
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap(),
        AlignMode::CenterH | AlignMode::CenterV => values.iter().sum::<f64>() / values.len() as f64,
    }
}

#[cfg(test)]
mod tests {
    use super::{AlignMode, align_value};

    #[test]
    fn left() {
        assert!((align_value(AlignMode::Left, &[1.0, 5.0, 3.0]) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn right() {
        assert!((align_value(AlignMode::Right, &[1.0, 5.0, 3.0]) - 5.0).abs() < 1e-9);
    }

    #[test]
    fn center() {
        assert!((align_value(AlignMode::CenterH, &[0.0, 10.0]) - 5.0).abs() < 1e-9);
    }

    #[test]
    fn empty() {
        assert_eq!(align_value(AlignMode::Left, &[]), 0.0);
    }
}
