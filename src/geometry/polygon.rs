#[allow(clippy::cast_precision_loss)]
pub fn polygon_centroid(poly: &[(f64, f64)]) -> (f64, f64) {
    let n = poly.len();
    if n == 0 {
        return (0.0, 0.0);
    }
    if n == 1 {
        return poly[0];
    }
    if n == 2 {
        return (
            f64::midpoint(poly[0].0, poly[1].0),
            f64::midpoint(poly[0].1, poly[1].1),
        );
    }
    let mut cx = 0.0;
    let mut cy = 0.0;
    let mut area = 0.0;
    for i in 0..n {
        let (x0, y0) = poly[i];
        let (x1, y1) = poly[(i + 1) % n];
        let cross = x0 * y1 - x1 * y0;
        area += cross;
        cx += (x0 + x1) * cross;
        cy += (y0 + y1) * cross;
    }
    area *= 0.5;
    if area.abs() < 1e-12 {
        // degenerate: average vertices
        let sx: f64 = poly.iter().map(|p| p.0).sum();
        let sy: f64 = poly.iter().map(|p| p.1).sum();
        return (sx / n as f64, sy / n as f64);
    }
    cx /= 6.0 * area;
    cy /= 6.0 * area;
    (cx, cy)
}

pub fn polygon_perimeter(poly: &[(f64, f64)]) -> f64 {
    let n = poly.len();
    if n < 2 {
        return 0.0;
    }
    if n == 2 {
        let (x0, y0) = poly[0];
        let (x1, y1) = poly[1];
        let dx = x1 - x0;
        let dy = y1 - y0;
        return (dx * dx + dy * dy).sqrt();
    }
    let mut total = 0.0;
    for i in 0..n {
        let (x0, y0) = poly[i];
        let (x1, y1) = poly[(i + 1) % n];
        let dx = x1 - x0;
        let dy = y1 - y0;
        total += (dx * dx + dy * dy).sqrt();
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn square_centroid() {
        let sq = [(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0)];
        let (cx, cy) = polygon_centroid(&sq);
        assert!((cx - 2.0).abs() < 1e-9);
        assert!((cy - 2.0).abs() < 1e-9);
    }

    #[test]
    fn triangle_centroid() {
        let tri = [(0.0, 0.0), (6.0, 0.0), (0.0, 6.0)];
        let (cx, cy) = polygon_centroid(&tri);
        assert!((cx - 2.0).abs() < 1e-9);
        assert!((cy - 2.0).abs() < 1e-9);
    }

    #[test]
    fn empty() {
        assert_eq!(polygon_centroid(&[]), (0.0, 0.0));
    }

    #[test]
    fn perimeter_square() {
        let sq = [(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0)];
        assert!((polygon_perimeter(&sq) - 16.0).abs() < 1e-9);
    }

    #[test]
    fn perimeter_degenerate() {
        assert_eq!(polygon_perimeter(&[]), 0.0);
        let p = [(0.0, 0.0), (3.0, 4.0)];
        assert!((polygon_perimeter(&p) - 5.0).abs() < 1e-9);
    }
}
