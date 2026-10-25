/// Generate grid positions for duplication.
#[allow(clippy::cast_precision_loss)]
pub fn grid_positions(cols: usize, rows: usize, spacing_x: f64, spacing_y: f64) -> Vec<(f64, f64)> {
    let mut out = Vec::new();
    for r in 0..rows {
        for c in 0..cols {
            out.push((c as f64 * spacing_x, r as f64 * spacing_y));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_cell() {
        let p = grid_positions(1, 1, 10.0, 10.0);
        assert_eq!(p, vec![(0.0, 0.0)]);
    }

    #[test]
    fn two_by_two() {
        let p = grid_positions(2, 2, 5.0, 5.0);
        assert_eq!(p.len(), 4);
        assert!(p.contains(&(0.0, 0.0)));
        assert!(p.contains(&(5.0, 5.0)));
    }

    #[test]
    fn zero_dims() {
        assert_eq!(grid_positions(0, 3, 1.0, 1.0), [] as [(f64, f64); 0]);
        assert_eq!(grid_positions(3, 0, 1.0, 1.0), [] as [(f64, f64); 0]);
    }

    #[test]
    fn zero_spacing() {
        let p = grid_positions(2, 2, 0.0, 0.0);
        // all positions coincide at origin
        assert_eq!(p.len(), 4);
        for (x, y) in &p {
            assert_eq!(*x, 0.0);
            assert_eq!(*y, 0.0);
        }
    }
}
