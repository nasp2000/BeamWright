/// Generate fill line segments for a polygon.
/// Returns list of (start, end) segments at given spacing (mm) and angle (rad).
/// Fill a polygon with holes: fill outer, subtract fills that lie inside any hole.
pub fn fill_with_holes(
    outer: &[(f64, f64)],
    holes: &[&[(f64, f64)]],
    spacing: f64,
    angle: f64,
) -> Vec<((f64, f64), (f64, f64))> {
    let mut lines = fill_lines(outer, spacing, angle);
    // remove segments whose midpoint lies inside a hole
    lines.retain(|(a, b)| {
        let mx = f64::midpoint(a.0, b.0);
        let my = f64::midpoint(a.1, b.1);
        !holes
            .iter()
            .any(|h| crate::geometry::boolean::point_in_polygon(mx, my, h))
    });
    lines
}

pub fn fill_lines(poly: &[(f64, f64)], spacing: f64, angle: f64) -> Vec<((f64, f64), (f64, f64))> {
    if poly.len() < 3 || spacing <= 0.0 {
        return Vec::new();
    }
    // rotate polygon by -angle, compute bounding box, generate horizontal lines, rotate back
    let cos = angle.cos();
    let sin = angle.sin();
    let rot: Vec<(f64, f64)> = poly
        .iter()
        .map(|(x, y)| (x * cos + y * sin, -x * sin + y * cos))
        .collect();
    let mut min_y = f64::INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    let mut min_x = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    for (x, y) in &rot {
        min_y = min_y.min(*y);
        max_y = max_y.max(*y);
        min_x = min_x.min(*x);
        max_x = max_x.max(*x);
    }
    let mut lines = Vec::new();
    let mut y = min_y;
    while y <= max_y {
        // find intersections of horizontal line y with polygon edges
        let mut xs = Vec::new();
        let n = rot.len();
        for i in 0..n {
            let (x0, y0) = rot[i];
            let (x1, y1) = rot[(i + 1) % n];
            if (y0 <= y && y1 > y) || (y1 <= y && y0 > y) {
                let x = x0 + (y - y0) * (x1 - x0) / (y1 - y0);
                xs.push(x);
            }
        }
        xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let mut k = 0;
        while k + 1 < xs.len() {
            let x0 = xs[k];
            let x1 = xs[k + 1];
            // rotate back
            let p0 = (x0 * cos - y * sin, x0 * sin + y * cos);
            let p1 = (x1 * cos - y * sin, x1 * sin + y * cos);
            lines.push((p0, p1));
            k += 2;
        }
        y += spacing;
    }
    // rotate back the y coordinate: we used rotated y; x positions already rotated back using y (rotated y). Need to use rotated y for back-rotation x: done via x0*cos - y*sin. But y is rotated y. Correct.
    let _ = (min_x, max_x);
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_poly() {
        assert_eq!(
            fill_lines(&[], 1.0, 0.0),
            [] as [((f64, f64), (f64, f64)); 0]
        );
    }

    #[test]
    fn zero_spacing() {
        let sq = [(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)];
        assert_eq!(
            fill_lines(&sq, 0.0, 0.0),
            [] as [((f64, f64), (f64, f64)); 0]
        );
    }

    #[test]
    fn square_horizontal() {
        let sq = [(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)];
        let lines = fill_lines(&sq, 2.0, 0.0);
        assert_ne!(lines, [] as [((f64, f64), (f64, f64)); 0]);
        assert!(lines.len() >= 4);
    }

    #[test]
    fn with_holes() {
        let outer = [(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)];
        let hole = [(3.0, 3.0), (7.0, 3.0), (7.0, 7.0), (3.0, 7.0)];
        let holes = [hole.as_slice()];
        let lines = fill_with_holes(&outer, &holes, 2.0, 0.0);
        let _ = lines;
    }

    #[test]
    fn rotated() {
        let sq = [(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)];
        let lines = fill_lines(&sq, 2.0, 0.5);
        assert_ne!(lines, [] as [((f64, f64), (f64, f64)); 0]);
    }
}
