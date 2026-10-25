use crate::gcode::toolpath::Toolpath;

/// Estimate duration in seconds for a set of toolpaths.
/// Assumes constant speed along cut moves; travel moves use 3x speed.
pub fn estimate_duration(toolpaths: &[Toolpath]) -> f64 {
    let mut total = 0.0;
    for tp in toolpaths {
        let pass_len = tp.total_length();
        let cut_time = if tp.speed > 0.0 {
            pass_len / tp.speed
        } else {
            0.0
        };
        total += cut_time * f64::from(tp.passes);
        // travel overhead: 10% of cut time
        total += cut_time * f64::from(tp.passes) * 0.1;
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gcode::toolpath::{MoveKind, Toolpath};

    #[test]
    fn empty() {
        assert_eq!(estimate_duration(&[]), 0.0);
    }

    #[test]
    fn single() {
        let mut tp = Toolpath::new(10.0, 50.0, 1);
        tp.push(MoveKind::Travel, 0.0, 0.0);
        tp.push(MoveKind::Cut, 10.0, 0.0);
        let d = estimate_duration(&[tp]);
        // length 10, speed 10 => 1s + 0.1s overhead = 1.1s
        assert!((d - 1.1).abs() < 1e-9);
    }

    #[test]
    fn passes_multiply() {
        let mut tp = Toolpath::new(10.0, 50.0, 2);
        tp.push(MoveKind::Travel, 0.0, 0.0);
        tp.push(MoveKind::Cut, 10.0, 0.0);
        let d = estimate_duration(&[tp]);
        assert!((d - 2.2).abs() < 1e-9);
    }

    #[test]
    fn zero_speed_safe() {
        let mut tp = Toolpath::new(0.0, 50.0, 1);
        tp.push(MoveKind::Cut, 10.0, 0.0);
        let d = estimate_duration(&[tp]);
        assert_eq!(d, 0.0);
    }
}
