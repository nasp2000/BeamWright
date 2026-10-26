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

/// Point-in-polygon test (ray casting).
pub fn point_in_polygon(px: f64, py: f64, poly: &[(f64, f64)]) -> bool {
    let n = poly.len();
    if n < 3 {
        return false;
    }
    let mut inside = false;
    let mut j = n - 1;
    for i in 0..n {
        let (xi, yi) = poly[i];
        let (xj, yj) = poly[j];
        if ((yi > py) != (yj > py)) && (px < (xj - xi) * (py - yi) / (yj - yi) + xi) {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// Signed area of a polygon (shoelace).
pub fn polygon_area(poly: &[(f64, f64)]) -> f64 {
    let n = poly.len();
    if n < 3 {
        return 0.0;
    }
    let mut sum = 0.0;
    for i in 0..n {
        let (x1, y1) = poly[i];
        let (x2, y2) = poly[(i + 1) % n];
        sum += x1 * y2 - x2 * y1;
    }
    sum / 2.0
}

/// Convex hull (monotone chain) of a set of points.
pub fn convex_hull(points: &[(f64, f64)]) -> Vec<(f64, f64)> {
    if points.len() <= 1 {
        return points.to_vec();
    }
    let mut pts = points.to_vec();
    pts.sort_by(|a, b| {
        a.0.partial_cmp(&b.0)
            .unwrap()
            .then(a.1.partial_cmp(&b.1).unwrap())
    });
    let mut lower: Vec<(f64, f64)> = Vec::new();
    for p in &pts {
        while lower.len() >= 2 && cross(lower[lower.len() - 2], lower[lower.len() - 1], *p) <= 0.0 {
            lower.pop();
        }
        lower.push(*p);
    }
    let mut upper: Vec<(f64, f64)> = Vec::new();
    for p in pts.iter().rev() {
        while upper.len() >= 2 && cross(upper[upper.len() - 2], upper[upper.len() - 1], *p) <= 0.0 {
            upper.pop();
        }
        upper.push(*p);
    }
    lower.pop();
    upper.pop();
    lower.extend(upper);
    lower
}

fn cross(o: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 {
    (a.0 - o.0) * (b.1 - o.1) - (a.1 - o.1) * (b.0 - o.0)
}

/// Sample points inside `poly` on a grid (approximation of area).
pub fn sample_points(poly: &[(f64, f64)], step: f64) -> Vec<(f64, f64)> {
    if poly.is_empty() || step <= 0.0 {
        return Vec::new();
    }
    let mut xs: Vec<f64> = poly.iter().map(|p| p.0).collect();
    let mut ys: Vec<f64> = poly.iter().map(|p| p.1).collect();
    xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
    ys.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let (xmin, xmax) = (xs[0], xs[xs.len() - 1]);
    let (ymin, ymax) = (ys[0], ys[ys.len() - 1]);
    let mut out = Vec::new();
    let mut x = xmin;
    while x <= xmax {
        let mut y = ymin;
        while y <= ymax {
            if point_in_polygon(x, y, poly) {
                out.push((x, y));
            }
            y += step;
        }
        x += step;
    }
    out
}

/// Union of two polygons approximated by sampling.
pub fn union_sample(a: &[(f64, f64)], b: &[(f64, f64)], step: f64) -> Vec<(f64, f64)> {
    let mut pts = sample_points(a, step);
    pts.extend(sample_points(b, step));
    convex_hull(&pts)
}

/// Intersection of two polygons approximated by sampling.
pub fn intersection_sample(a: &[(f64, f64)], b: &[(f64, f64)], step: f64) -> Vec<(f64, f64)> {
    let sa = sample_points(a, step);
    let mut out = Vec::new();
    for p in sa {
        if point_in_polygon(p.0, p.1, b) {
            out.push(p);
        }
    }
    convex_hull(&out)
}

/// Difference (a - b) approximated by sampling.
pub fn difference_sample(a: &[(f64, f64)], b: &[(f64, f64)], step: f64) -> Vec<(f64, f64)> {
    let sa = sample_points(a, step);
    let mut out = Vec::new();
    for p in sa {
        if !point_in_polygon(p.0, p.1, b) {
            out.push(p);
        }
    }
    convex_hull(&out)
}

/// Xor of two polygons approximated by sampling.
pub fn xor_sample(a: &[(f64, f64)], b: &[(f64, f64)], step: f64) -> Vec<(f64, f64)> {
    let sa = sample_points(a, step);
    let sb = sample_points(b, step);
    let mut out = Vec::new();
    for p in &sa {
        if !point_in_polygon(p.0, p.1, b) {
            out.push(*p);
        }
    }
    for p in &sb {
        if !point_in_polygon(p.0, p.1, a) {
            out.push(*p);
        }
    }
    convex_hull(&out)
}

/// Sutherland-Hodgman: clip `subject` polygon against a single half-plane
/// defined by edge from `a` to `b` (keep points left of edge).
fn clip_half_plane(subject: &[(f64, f64)], a: (f64, f64), b: (f64, f64)) -> Vec<(f64, f64)> {
    let mut out = Vec::new();
    let n = subject.len();
    if n == 0 {
        return out;
    }
    for i in 0..n {
        let curr = subject[i];
        let prev = subject[(i + n - 1) % n];
        let fc = (b.0 - a.0) * (curr.1 - a.1) - (b.1 - a.1) * (curr.0 - a.0);
        let fp = (b.0 - a.0) * (prev.1 - a.1) - (b.1 - a.1) * (prev.0 - a.0);
        if fc >= 0.0 {
            if fp < 0.0 {
                if let Some(p) = intersect_segments(prev, curr, a, b) {
                    out.push(p);
                }
            }
            out.push(curr);
        } else if fp >= 0.0 {
            if let Some(p) = intersect_segments(prev, curr, a, b) {
                out.push(p);
            }
        }
    }
    out
}

fn intersect_segments(
    p1: (f64, f64),
    p2: (f64, f64),
    p3: (f64, f64),
    p4: (f64, f64),
) -> Option<(f64, f64)> {
    let d1 = (p2.0 - p1.0, p2.1 - p1.1);
    let d2 = (p4.0 - p3.0, p4.1 - p3.1);
    let cross = d1.0 * d2.1 - d1.1 * d2.0;
    if cross.abs() < 1e-12 {
        return None;
    }
    let t = ((p3.0 - p1.0) * d2.1 - (p3.1 - p1.1) * d2.0) / cross;
    Some((p1.0 + t * d1.0, p1.1 + t * d1.1))
}

/// Clip subject polygon to convex clip polygon (Sutherland-Hodgman intersection).
pub fn clip_polygon(subject: &[(f64, f64)], clip: &[(f64, f64)]) -> Vec<(f64, f64)> {
    let mut out = subject.to_vec();
    let n = clip.len();
    if n < 3 {
        return out;
    }
    for i in 0..n {
        let a = clip[i];
        let b = clip[(i + 1) % n];
        out = clip_half_plane(&out, a, b);
        if out.is_empty() {
            break;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn point_in_triangle() {
        let tri = [(0.0, 0.0), (10.0, 0.0), (5.0, 10.0)];
        assert!(point_in_polygon(5.0, 3.0, &tri));
        assert!(!point_in_polygon(20.0, 20.0, &tri));
    }

    #[test]
    fn point_in_square() {
        let sq = [(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)];
        assert!(point_in_polygon(5.0, 5.0, &sq));
        assert!(!point_in_polygon(15.0, 5.0, &sq));
    }

    #[test]
    fn area_triangle() {
        let tri = [(0.0, 0.0), (10.0, 0.0), (5.0, 10.0)];
        assert!((polygon_area(&tri) - 50.0).abs() < 1e-9);
    }

    #[test]
    fn area_square() {
        let sq = [(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)];
        assert!((polygon_area(&sq) - 100.0).abs() < 1e-9);
    }

    #[test]
    fn convex_hull_basic() {
        let pts = [(0.0, 0.0), (1.0, 1.0), (2.0, 0.0), (1.0, 0.5)];
        let h = convex_hull(&pts);
        assert!(h.len() >= 3);
    }

    #[test]
    fn convex_hull_single() {
        let pts = [(1.0, 2.0)];
        let h = convex_hull(&pts);
        assert_eq!(h.len(), 1);
    }

    #[test]
    fn sample_points_square() {
        let sq = [(0.0, 0.0), (2.0, 0.0), (2.0, 2.0), (0.0, 2.0)];
        let pts = sample_points(&sq, 1.0);
        assert_ne!(pts, [] as [(f64, f64); 0]);
    }

    #[test]
    fn union_sample_nonempty() {
        let a = [(0.0, 0.0), (2.0, 0.0), (2.0, 2.0), (0.0, 2.0)];
        let b = [(1.0, 1.0), (3.0, 1.0), (3.0, 3.0), (1.0, 3.0)];
        let r = union_sample(&a, &b, 1.0);
        assert_ne!(r, [] as [(f64, f64); 0]);
    }

    #[test]
    fn intersection_sample_basic() {
        let a = [(0.0, 0.0), (2.0, 0.0), (2.0, 2.0), (0.0, 2.0)];
        let b = [(1.0, 1.0), (3.0, 1.0), (3.0, 3.0), (1.0, 3.0)];
        let r = intersection_sample(&a, &b, 1.0);
        // overlap region exists
        assert_ne!(r, [] as [(f64, f64); 0]);
    }

    #[test]
    fn clip_polygon_basic() {
        let subject = [(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)];
        let clip = [(2.0, 2.0), (8.0, 2.0), (8.0, 8.0), (2.0, 8.0)];
        let r = clip_polygon(&subject, &clip);
        assert_ne!(r, [] as [(f64, f64); 0]);
    }

    #[test]
    fn clip_polygon_no_overlap() {
        let subject = [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)];
        let clip = [(5.0, 5.0), (6.0, 5.0), (6.0, 6.0), (5.0, 6.0)];
        let r = clip_polygon(&subject, &clip);
        assert_eq!(r, [] as [(f64, f64); 0]);
    }

    #[test]
    fn clip_polygon_full_contain() {
        let subject = [(1.0, 1.0), (2.0, 1.0), (2.0, 2.0), (1.0, 2.0)];
        let clip = [(0.0, 0.0), (5.0, 0.0), (5.0, 5.0), (0.0, 5.0)];
        let r = clip_polygon(&subject, &clip);
        assert_eq!(r.len(), 4);
    }

    #[test]
    fn difference_sample_basic() {
        let a = [(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0)];
        let b = [(1.0, 1.0), (3.0, 1.0), (3.0, 3.0), (1.0, 3.0)];
        let r = difference_sample(&a, &b, 1.0);
        let _ = r;
    }

    #[test]
    fn xor_sample_basic() {
        let a = [(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0)];
        let b = [(2.0, 2.0), (6.0, 2.0), (6.0, 6.0), (2.0, 6.0)];
        let r = xor_sample(&a, &b, 1.0);
        assert_ne!(r, [] as [(f64, f64); 0]);
    }
}
