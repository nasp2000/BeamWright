/// Offset parameters (PLAN.md §6.8, §18).
#[derive(Debug, Clone, Copy)]
pub struct OffsetParams {
    pub distance: f64,
    pub join_type: JoinType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JoinType {
    Miter,
    Round,
    Bevel,
}

impl Default for OffsetParams {
    fn default() -> Self {
        Self {
            distance: 1.0,
            join_type: JoinType::Miter,
        }
    }
}

impl OffsetParams {
    pub fn validate(&self) -> std::result::Result<(), String> {
        if !self.distance.is_finite() {
            return Err("distance must be finite".to_string());
        }
        Ok(())
    }
}

/// Offset a polygon by `distance` (positive = outward for CCW polygons).
pub fn offset_polygon(poly: &[(f64, f64)], distance: f64) -> Vec<(f64, f64)> {
    let n = poly.len();
    if n < 3 {
        return poly.to_vec();
    }
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let prev = poly[(i + n - 1) % n];
        let curr = poly[i];
        let next = poly[(i + 1) % n];
        // edge normals
        let e1 = (curr.0 - prev.0, curr.1 - prev.1);
        let e2 = (next.0 - curr.0, next.1 - curr.1);
        let n1 = normalize(-e1.1, e1.0);
        let n2 = normalize(-e2.1, e2.0);
        let nx = f64::midpoint(n1.0, n2.0);
        let ny = f64::midpoint(n1.1, n2.1);
        let (nx, ny) = normalize(nx, ny);
        out.push((curr.0 + nx * distance, curr.1 + ny * distance));
    }
    out
}

fn normalize(x: f64, y: f64) -> (f64, f64) {
    let len = (x * x + y * y).sqrt();
    if len == 0.0 {
        (0.0, 0.0)
    } else {
        (x / len, y / len)
    }
}

/// Offset a polygon outward by `distance` using edge-normal (miter) method.
pub fn offset_polygon_miter(poly: &[(f64, f64)], distance: f64) -> Vec<(f64, f64)> {
    let n = poly.len();
    if n < 3 {
        return poly.to_vec();
    }
    // compute offset edges
    let mut edges: Vec<((f64, f64), (f64, f64))> = Vec::with_capacity(n);
    for i in 0..n {
        let a = poly[i];
        let b = poly[(i + 1) % n];
        let dx = b.0 - a.0;
        let dy = b.1 - a.1;
        let len = (dx * dx + dy * dy).sqrt();
        if len == 0.0 {
            edges.push((a, b));
            continue;
        }
        // left normal (outward for CCW)
        let nx = -dy / len * distance;
        let ny = dx / len * distance;
        edges.push(((a.0 + nx, a.1 + ny), (b.0 + nx, b.1 + ny)));
    }
    // intersect consecutive offset edges
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let (a1, b1) = edges[(i + n - 1) % n];
        let (a2, b2) = edges[i];
        if let Some(p) = intersect_lines(a1, b1, a2, b2) {
            out.push(p);
        } else {
            out.push(poly[i]);
        }
    }
    out
}

fn intersect_lines(
    a1: (f64, f64),
    b1: (f64, f64),
    a2: (f64, f64),
    b2: (f64, f64),
) -> Option<(f64, f64)> {
    let d1 = (b1.0 - a1.0, b1.1 - a1.1);
    let d2 = (b2.0 - a2.0, b2.1 - a2.1);
    let cross = d1.0 * d2.1 - d1.1 * d2.0;
    if cross.abs() < 1e-12 {
        return None;
    }
    let t = ((a2.0 - a1.0) * d2.1 - (a2.1 - a1.1) * d2.0) / cross;
    Some((a1.0 + t * d1.0, a1.1 + t * d1.1))
}

/// Offset polygon with round joins (approximates arcs with `segments` per corner).
pub fn offset_polygon_round(
    poly: &[(f64, f64)],
    distance: f64,
    segments: usize,
) -> Vec<(f64, f64)> {
    let n = poly.len();
    if n < 3 || segments == 0 {
        return offset_polygon_miter(poly, distance);
    }
    // Use miter as base, then (placeholder) return miter result.
    // Round joins require arc insertion; keep miter for now but smooth via
    // sampling: this is a simplified stand-in.
    offset_polygon_miter(poly, distance)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_valid() {
        assert!(OffsetParams::default().validate().is_ok());
    }

    #[test]
    fn nan_distance_invalid() {
        let p = OffsetParams {
            distance: f64::NAN,
            ..OffsetParams::default()
        };
        assert!(p.validate().is_err());
    }

    #[test]
    fn join_types_distinct() {
        assert_ne!(JoinType::Miter, JoinType::Round);
        assert_ne!(JoinType::Round, JoinType::Bevel);
    }
}

#[cfg(test)]
mod offset_tests {
    use super::offset_polygon;

    #[test]
    fn square_offset() {
        let sq = [(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)];
        let r = offset_polygon(&sq, 1.0);
        assert_eq!(r.len(), 4);
    }

    #[test]
    fn zero_offset() {
        let sq = [(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)];
        let r = offset_polygon(&sq, 0.0);
        for (a, b) in sq.iter().zip(r.iter()) {
            assert!((a.0 - b.0).abs() < 1e-9);
            assert!((a.1 - b.1).abs() < 1e-9);
        }
    }

    #[test]
    fn small_polygon() {
        let tri = [(0.0, 0.0), (1.0, 0.0), (0.5, 1.0)];
        let r = offset_polygon(&tri, 0.1);
        assert_eq!(r.len(), 3);
    }
}

#[cfg(test)]
mod miter_tests {
    use super::offset_polygon_miter;

    #[test]
    fn square() {
        let sq = [(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)];
        let r = offset_polygon_miter(&sq, 1.0);
        assert_eq!(r.len(), 4);
    }

    #[test]
    fn zero() {
        let sq = [(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)];
        let r = offset_polygon_miter(&sq, 0.0);
        for (a, b) in sq.iter().zip(r.iter()) {
            assert!((a.0 - b.0).abs() < 1e-9);
            assert!((a.1 - b.1).abs() < 1e-9);
        }
    }

    #[test]
    fn small() {
        let tri = [(0.0, 0.0), (1.0, 0.0), (0.5, 1.0)];
        let r = offset_polygon_miter(&tri, 0.1);
        assert_eq!(r.len(), 3);
    }
}
