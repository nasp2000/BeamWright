/// Generate an Archimedean spiral as a list of points.
#[allow(clippy::cast_precision_loss)]
pub fn archimedean_spiral(turns: usize, points_per_turn: usize, a: f64, b: f64) -> Vec<(f64, f64)> {
    let total = turns * points_per_turn;
    let mut out = Vec::with_capacity(total + 1);
    out.push((0.0, 0.0));
    for i in 1..=total {
        #[allow(clippy::cast_precision_loss)]
        let t = i as f64 / points_per_turn as f64;
        let theta = t * 2.0 * std::f64::consts::PI;
        let r = a + b * theta;
        out.push((r * theta.cos(), r * theta.sin()));
    }
    out
}

/// Generate a rectangular spiral.
#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]
pub fn rectangular_spiral(turns: usize, step: f64) -> Vec<(f64, f64)> {
    let mut out = Vec::new();
    let mut x = 0.0;
    let mut y = 0.0;
    out.push((x, y));
    let mut dx = step;
    let mut dy = 0.0;
    let mut seg_len = step;
    let mut count = 0;
    for _ in 0..turns * 4 {
        #[allow(
            clippy::cast_precision_loss,
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss
        )]
        for _ in 0..(seg_len as usize / step as usize).max(1) {
            x += dx;
            y += dy;
            out.push((x, y));
        }
        // turn left
        let (ndx, ndy) = (-dy, dx);
        dx = ndx;
        dy = ndy;
        count += 1;
        if count % 2 == 0 {
            seg_len += step;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn archimedean_nonempty() {
        let s = archimedean_spiral(2, 8, 0.0, 1.0);
        assert_ne!(s, [] as [(f64, f64); 0]);
        assert!(s.len() > 10);
    }

    #[test]
    fn archimedean_zero_turns() {
        let s = archimedean_spiral(0, 8, 0.0, 1.0);
        assert_eq!(s.len(), 1);
    }

    #[test]
    fn rectangular_nonempty() {
        let s = rectangular_spiral(2, 1.0);
        assert_ne!(s, [] as [(f64, f64); 0]);
    }

    #[test]
    fn rectangular_zero() {
        let s = rectangular_spiral(0, 1.0);
        assert_eq!(s.len(), 1);
    }
}
