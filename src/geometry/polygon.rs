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

/// Ramer-Douglas-Peucker polygon simplification.
pub fn simplify(poly: &[(f64, f64)], epsilon: f64) -> Vec<(f64, f64)> {
    if poly.len() <= 2 {
        return poly.to_vec();
    }
    // find point with max distance from line between first and last
    let first = poly[0];
    let last = poly[poly.len() - 1];
    let mut max_dist = 0.0;
    let mut max_idx = 0;
    for (i, p) in poly.iter().enumerate().skip(1).take(poly.len() - 2) {
        let d = point_line_distance(*p, first, last);
        if d > max_dist {
            max_dist = d;
            max_idx = i;
        }
    }
    if max_dist > epsilon {
        let mut left = simplify(&poly[..=max_idx], epsilon);
        let right = simplify(&poly[max_idx..], epsilon);
        left.pop();
        left.extend(right);
        left
    } else {
        vec![first, last]
    }
}

fn point_line_distance(p: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 {
    let dx = b.0 - a.0;
    let dy = b.1 - a.1;
    let len = (dx * dx + dy * dy).sqrt();
    if len == 0.0 {
        return ((p.0 - a.0).powi(2) + (p.1 - a.1).powi(2)).sqrt();
    }
    ((p.0 - a.0) * dy - (p.1 - a.1) * dx).abs() / len
}

/// Check if polygon is convex.
pub fn is_convex(poly: &[(f64, f64)]) -> bool {
    let n = poly.len();
    if n < 3 {
        return false;
    }
    let mut sign = 0i32;
    for i in 0..n {
        let (x0, y0) = poly[i];
        let (x1, y1) = poly[(i + 1) % n];
        let (x2, y2) = poly[(i + 2) % n];
        let cross = (x1 - x0) * (y2 - y0) - (y1 - y0) * (x2 - x0);
        if cross > 1e-9 {
            if sign == -1 {
                return false;
            }
            sign = 1;
        } else if cross < -1e-9 {
            if sign == 1 {
                return false;
            }
            sign = -1;
        }
    }
    true
}

#[cfg(test)]
mod simplify_tests {
    use super::*;

    #[test]
    fn simplify_reduces() {
        let poly = [(0.0, 0.0), (1.0, 0.1), (2.0, 0.0), (3.0, 0.1), (4.0, 0.0)];
        let r = simplify(&poly, 0.5);
        assert!(r.len() <= poly.len());
        assert!(r.len() >= 2);
    }

    #[test]
    fn simplify_small_epsilon() {
        let poly = [(0.0, 0.0), (1.0, 1.0), (2.0, 0.0)];
        let r = simplify(&poly, 0.001);
        assert_eq!(r.len(), 3);
    }

    #[test]
    fn is_convex_square() {
        let sq = [(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0)];
        assert!(is_convex(&sq));
    }

    #[test]
    fn is_convex_concave() {
        let poly = [(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (2.0, 2.0), (0.0, 4.0)];
        assert!(!is_convex(&poly));
    }

    #[test]
    fn is_convex_degenerate() {
        assert!(!is_convex(&[]));
        assert!(!is_convex(&[(0.0, 0.0), (1.0, 0.0)]));
    }
}

/// Check if polygon is simple (no self-intersections).
pub fn is_simple(poly: &[(f64, f64)]) -> bool {
    let n = poly.len();
    if n < 3 {
        return true;
    }
    for i in 0..n {
        let a1 = poly[i];
        let a2 = poly[(i + 1) % n];
        for j in (i + 2)..n {
            if j == n - 1 && i == 0 {
                continue; // adjacent edge
            }
            let b1 = poly[j];
            let b2 = poly[(j + 1) % n];
            if segments_intersect(a1, a2, b1, b2) {
                return false;
            }
        }
    }
    true
}

fn segments_intersect(a1: (f64, f64), a2: (f64, f64), b1: (f64, f64), b2: (f64, f64)) -> bool {
    let d1 = (a2.0 - a1.0, a2.1 - a1.1);
    let d2 = (b2.0 - b1.0, b2.1 - b1.1);
    let cross = d1.0 * d2.1 - d1.1 * d2.0;
    if cross.abs() < 1e-12 {
        return false;
    }
    let t = ((b1.0 - a1.0) * d2.1 - (b1.1 - a1.1) * d2.0) / cross;
    let u = ((b1.0 - a1.0) * d1.1 - (b1.1 - a1.1) * d1.0) / cross;
    (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u)
}

/// Ensure polygon is counter-clockwise; reverse if needed.
pub fn ensure_ccw(poly: &[(f64, f64)]) -> Vec<(f64, f64)> {
    let mut p = poly.to_vec();
    // compute signed area
    let n = p.len();
    if n < 3 {
        return p;
    }
    let mut area = 0.0;
    for i in 0..n {
        let (x0, y0) = p[i];
        let (x1, y1) = p[(i + 1) % n];
        area += x0 * y1 - x1 * y0;
    }
    if area < 0.0 {
        p.reverse();
    }
    p
}

#[cfg(test)]
mod simple_tests {
    use super::*;

    #[test]
    fn simple_square() {
        let sq = [(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0)];
        assert!(is_simple(&sq));
    }

    #[test]
    fn self_intersecting() {
        let bowtie = [(0.0, 0.0), (4.0, 4.0), (4.0, 0.0), (0.0, 4.0)];
        assert!(!is_simple(&bowtie));
    }

    #[test]
    fn ensure_ccw_reverses() {
        // clockwise square
        let cw = [(0.0, 0.0), (0.0, 4.0), (4.0, 4.0), (4.0, 0.0)];
        let r = ensure_ccw(&cw);
        // area should be positive
        let mut area = 0.0;
        for i in 0..r.len() {
            let (x0, y0) = r[i];
            let (x1, y1) = r[(i + 1) % r.len()];
            area += x0 * y1 - x1 * y0;
        }
        assert!(area > 0.0);
    }

    #[test]
    fn ensure_ccw_keeps() {
        let ccw = [(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0)];
        let r = ensure_ccw(&ccw);
        assert_eq!(r, ccw.to_vec());
    }
}

/// Area of a polygon (shoelace).
pub fn area(poly: &[(f64, f64)]) -> f64 {
    crate::geometry::boolean::polygon_area(poly)
}

/// Check if a point is inside a polygon (ray casting, reuses boolean logic).
pub fn contains_point(poly: &[(f64, f64)], px: f64, py: f64) -> bool {
    crate::geometry::boolean::point_in_polygon(px, py, poly)
}

#[cfg(test)]
mod contains_tests {
    use super::*;

    #[test]
    fn inside() {
        let sq = [(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0)];
        assert!(contains_point(&sq, 2.0, 2.0));
    }

    #[test]
    fn outside() {
        let sq = [(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0)];
        assert!(!contains_point(&sq, 5.0, 5.0));
    }

    #[test]
    fn empty() {
        assert!(!contains_point(&[], 0.0, 0.0));
    }
}

#[cfg(test)]
mod area_tests {
    use super::*;

    #[test]
    fn square() {
        let sq = [(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0)];
        assert!((area(&sq) - 16.0).abs() < 1e-9);
    }

    #[test]
    fn triangle() {
        let tri = [(0.0, 0.0), (6.0, 0.0), (0.0, 6.0)];
        assert!((area(&tri) - 18.0).abs() < 1e-9);
    }

    #[test]
    fn empty() {
        assert_eq!(area(&[]), 0.0);
    }
}
