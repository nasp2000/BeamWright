/// Distribute `n` positions evenly between `start` and `end` (inclusive).
#[allow(clippy::cast_precision_loss)]
pub fn distribute(start: f64, end: f64, n: usize) -> Vec<f64> {
    if n == 0 {
        return Vec::new();
    }
    if n == 1 {
        return vec![start];
    }
    #[allow(clippy::cast_precision_loss)]
    let step = (end - start) / (n as f64 - 1.0);
    #[allow(clippy::cast_precision_loss)]
    (0..n).map(|i| start + step * i as f64).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_points() {
        let v = distribute(0.0, 10.0, 2);
        assert_eq!(v, vec![0.0, 10.0]);
    }

    #[test]
    fn three_points() {
        let v = distribute(0.0, 10.0, 3);
        assert_eq!(v, vec![0.0, 5.0, 10.0]);
    }

    #[test]
    fn single() {
        let v = distribute(5.0, 10.0, 1);
        assert_eq!(v, vec![5.0]);
    }

    #[test]
    fn zero() {
        assert_eq!(distribute(0.0, 10.0, 0), [] as [f64; 0]);
    }
}

/// Distribute points in 2D grid within a rectangle.
pub fn distribute_2d(x0: f64, y0: f64, x1: f64, y1: f64, nx: usize, ny: usize) -> Vec<(f64, f64)> {
    let xs = distribute(x0, x1, nx);
    let ys = distribute(y0, y1, ny);
    let mut out = Vec::new();
    for y in &ys {
        for x in &xs {
            out.push((*x, *y));
        }
    }
    out
}

#[cfg(test)]
mod d2_tests {
    use super::distribute_2d;

    #[test]
    fn grid_2x2() {
        let pts = distribute_2d(0.0, 0.0, 10.0, 10.0, 2, 2);
        assert_eq!(pts.len(), 4);
    }

    #[test]
    fn single() {
        let pts = distribute_2d(5.0, 5.0, 5.0, 5.0, 1, 1);
        assert_eq!(pts, vec![(5.0, 5.0)]);
    }

    #[test]
    fn zero() {
        assert_eq!(
            distribute_2d(0.0, 0.0, 10.0, 10.0, 0, 2),
            [] as [(f64, f64); 0]
        );
    }
}
