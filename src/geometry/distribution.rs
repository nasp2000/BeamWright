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
