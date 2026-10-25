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
}
