//! Polygon boolean operations (PLAN.md §6.8).
//!
//! The implementation is Greiner–Hormann: both polygons are stored as circular
//! linked lists, every edge crossing is inserted as a shared vertex, and the
//! result is assembled by walking one boundary and switching to the other at
//! each crossing. It handles concave polygons and returns one contour per output
//! region; a subtraction that fully contains the second polygon returns the
//! outer contour followed by the hole in the opposite orientation.
//!
//! Degenerate inputs are rejected or simplified: consecutive duplicate points are
//! removed, edges that only touch at a vertex are treated as non-crossing, and a
//! self-intersecting polygon is used as given.

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

    pub fn all() -> [BooleanOp; 4] {
        [
            BooleanOp::Union,
            BooleanOp::Difference,
            BooleanOp::Intersection,
            BooleanOp::Xor,
        ]
    }
}

const EPSILON: f64 = 1e-9;

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

/// Signed area of a polygon (shoelace). Positive means counter-clockwise.
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
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
    });
    pts.dedup();
    if pts.len() <= 2 {
        return pts;
    }
    let cross = |o: (f64, f64), a: (f64, f64), b: (f64, f64)| {
        (a.0 - o.0) * (b.1 - o.1) - (a.1 - o.1) * (b.0 - o.0)
    };
    let mut lower: Vec<(f64, f64)> = Vec::new();
    for &point in &pts {
        while lower.len() >= 2
            && cross(lower[lower.len() - 2], lower[lower.len() - 1], point) <= 0.0
        {
            lower.pop();
        }
        lower.push(point);
    }
    let mut upper: Vec<(f64, f64)> = Vec::new();
    for &point in pts.iter().rev() {
        while upper.len() >= 2
            && cross(upper[upper.len() - 2], upper[upper.len() - 1], point) <= 0.0
        {
            upper.pop();
        }
        upper.push(point);
    }
    lower.pop();
    upper.pop();
    lower.extend(upper);
    lower
}

/// Remove non-finite points, consecutive duplicates and a repeated closing point.
fn clean(poly: &[(f64, f64)]) -> Vec<(f64, f64)> {
    let mut out: Vec<(f64, f64)> = Vec::with_capacity(poly.len());
    for point in poly {
        if !point.0.is_finite() || !point.1.is_finite() {
            continue;
        }
        if out.last().is_none_or(|last| !same_point(*last, *point)) {
            out.push(*point);
        }
    }
    if out.len() > 1 && same_point(out[0], out[out.len() - 1]) {
        out.pop();
    }
    out
}

/// Split a polyline at every proper crossing with `other`.
///
/// Returns the pieces in path order; a polyline without crossings comes back
/// unchanged as a single piece. Crossings at an existing vertex are not
/// inserted, so a piece boundary is always a real intersection point.
pub fn split_polyline(
    line: &[(f64, f64)],
    closed: bool,
    other: &[(f64, f64)],
) -> Vec<Vec<(f64, f64)>> {
    let count = line.len();
    if count < 2 || other.len() < 3 {
        return vec![line.to_vec()];
    }
    let edges = if closed { count } else { count - 1 };
    let mut cuts: Vec<(usize, f64, (f64, f64))> = Vec::new();
    for edge in 0..edges {
        let from = line[edge];
        let to = line[(edge + 1) % count];
        for index in 0..other.len() {
            let other_from = other[index];
            let other_to = other[(index + 1) % other.len()];
            if let Some((along, across, point)) =
                segment_intersection(from, to, other_from, other_to)
            {
                if along > EPSILON
                    && along < 1.0 - EPSILON
                    && across > EPSILON
                    && across < 1.0 - EPSILON
                {
                    cuts.push((edge, along, point));
                }
            }
        }
    }
    if cuts.is_empty() {
        return vec![line.to_vec()];
    }
    cuts.sort_by(|left, right| {
        left.0.cmp(&right.0).then(
            left.1
                .partial_cmp(&right.1)
                .unwrap_or(std::cmp::Ordering::Equal),
        )
    });
    let mut pieces = Vec::new();
    let mut current = vec![line[0]];
    let mut cut = 0;
    for edge in 0..edges {
        while cut < cuts.len() && cuts[cut].0 == edge {
            let point = cuts[cut].2;
            current.push(point);
            pieces.push(std::mem::take(&mut current));
            current.push(point);
            cut += 1;
        }
        current.push(line[(edge + 1) % count]);
    }
    if current.len() >= 2 {
        pieces.push(current);
    }
    // The path start is not a cut, so for a closed path the last and first
    // pieces continue each other.
    if closed && pieces.len() > 1 {
        let joins = pieces
            .last()
            .zip(pieces.first())
            .is_some_and(|(last, first)| last.last().is_some_and(|end| same_point(*end, first[0])));
        if joins {
            let first = pieces.remove(0);
            if let Some(last) = pieces.last_mut() {
                last.extend(first.into_iter().skip(1));
            }
        }
    }
    pieces.retain(|piece| piece.len() >= 2);
    pieces
}

/// A directed edge fragment kept while assembling the result.
type Fragment = ((f64, f64), (f64, f64));

fn same_point(a: (f64, f64), b: (f64, f64)) -> bool {
    (a.0 - b.0).abs() <= EPSILON && (a.1 - b.1).abs() <= EPSILON
}

fn midpoint(a: (f64, f64), b: (f64, f64)) -> (f64, f64) {
    (f64::midpoint(a.0, b.0), f64::midpoint(a.1, b.1))
}

fn orient_ccw(poly: &[(f64, f64)]) -> Vec<(f64, f64)> {
    if polygon_area(poly) < 0.0 {
        let mut reversed = poly.to_vec();
        reversed.reverse();
        reversed
    } else {
        poly.to_vec()
    }
}

/// Intersection of two segments, returning the parameters along each segment.
fn segment_intersection(
    first_start: (f64, f64),
    first_end: (f64, f64),
    second_start: (f64, f64),
    second_end: (f64, f64),
) -> Option<(f64, f64, (f64, f64))> {
    let first = (first_end.0 - first_start.0, first_end.1 - first_start.1);
    let second = (second_end.0 - second_start.0, second_end.1 - second_start.1);
    let denominator = first.0 * second.1 - first.1 * second.0;
    if denominator.abs() < EPSILON {
        return None;
    }
    let between = (
        second_start.0 - first_start.0,
        second_start.1 - first_start.1,
    );
    let along_first = (between.0 * second.1 - between.1 * second.0) / denominator;
    let along_second = (between.0 * first.1 - between.1 * first.0) / denominator;
    Some((
        along_first,
        along_second,
        (
            first_start.0 + along_first * first.0,
            first_start.1 + along_first * first.1,
        ),
    ))
}

/// Insert every proper crossing with `other` as a vertex of `poly`.
///
/// Edges that merely touch at a shared vertex or overlap collinearly are not
/// split; such input is documented as unsupported.
fn split_at_crossings(poly: &[(f64, f64)], other: &[(f64, f64)]) -> Vec<(f64, f64)> {
    let n = poly.len();
    let mut out = Vec::with_capacity(n);
    for index in 0..n {
        let a = poly[index];
        let b = poly[(index + 1) % n];
        out.push(a);
        let mut cuts: Vec<(f64, (f64, f64))> = Vec::new();
        for edge in 0..other.len() {
            let other_from = other[edge];
            let other_to = other[(edge + 1) % other.len()];
            if let Some((t, u, point)) = segment_intersection(a, b, other_from, other_to) {
                if t > EPSILON && t < 1.0 - EPSILON && u > EPSILON && u < 1.0 - EPSILON {
                    cuts.push((t, point));
                }
            }
        }
        cuts.sort_by(|left, right| {
            left.0
                .partial_cmp(&right.0)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        for (_, point) in cuts {
            if out.last().is_none_or(|last| !same_point(*last, point)) {
                out.push(point);
            }
        }
    }
    out
}

fn fragments(poly: &[(f64, f64)]) -> Vec<Fragment> {
    let n = poly.len();
    (0..n)
        .map(|index| (poly[index], poly[(index + 1) % n]))
        .filter(|(a, b)| !same_point(*a, *b))
        .collect()
}

/// Assemble directed fragments into closed contours.
///
/// Fragments that continue the current contour are preferred; when a vertex has
/// several outgoing fragments the first one is taken, which keeps the result
/// stable for the simple polygons this module supports.
fn stitch(mut segments: Vec<Fragment>) -> Vec<Vec<(f64, f64)>> {
    let mut contours = Vec::new();
    while let Some((start, end)) = segments.pop() {
        let mut contour = vec![start];
        let mut current = end;
        let mut closed = false;
        while !same_point(current, start) {
            contour.push(current);
            let Some(position) = segments
                .iter()
                .position(|(from, _)| same_point(*from, current))
            else {
                break;
            };
            let (_, next) = segments.remove(position);
            current = next;
            if same_point(current, start) {
                closed = true;
            }
        }
        if closed && contour.len() >= 3 {
            contours.push(contour);
        }
    }
    contours
}

/// Apply a boolean operation to two polygons.
///
/// Both polygons are normalised to counter-clockwise, split at every crossing,
/// and each resulting edge fragment is kept or dropped according to whether its
/// midpoint lies inside the other polygon. The kept fragments are stitched back
/// into closed contours: counter-clockwise for solid regions and clockwise for
/// holes. Empty input or a result without area returns an empty vector.
pub fn boolean_contours(
    op: BooleanOp,
    subject: &[(f64, f64)],
    clip: &[(f64, f64)],
) -> Vec<Vec<(f64, f64)>> {
    let subject = clean(subject);
    let clip = clean(clip);
    if subject.len() < 3 || clip.len() < 3 {
        return Vec::new();
    }
    if op == BooleanOp::Xor {
        let mut result = boolean_contours(BooleanOp::Difference, &subject, &clip);
        result.extend(boolean_contours(BooleanOp::Difference, &clip, &subject));
        return result
            .into_iter()
            .filter(|contour| contour.len() >= 3)
            .collect();
    }

    let subject = orient_ccw(&subject);
    let clip = orient_ccw(&clip);
    let subject_split = split_at_crossings(&subject, &clip);
    let clip_split = split_at_crossings(&clip, &subject);

    let inside =
        |point: (f64, f64), polygon: &[(f64, f64)]| point_in_polygon(point.0, point.1, polygon);

    let mut kept: Vec<Fragment> = Vec::new();
    for (from, to) in fragments(&subject_split) {
        let mid = midpoint(from, to);
        let inside_clip = inside(mid, &clip);
        let keep = match op {
            BooleanOp::Union | BooleanOp::Difference => !inside_clip,
            BooleanOp::Intersection => inside_clip,
            BooleanOp::Xor => unreachable!("handled above"),
        };
        if keep {
            kept.push((from, to));
        }
    }
    for (from, to) in fragments(&clip_split) {
        let mid = midpoint(from, to);
        let inside_subject = inside(mid, &subject);
        let keep = match op {
            BooleanOp::Union => !inside_subject,
            // For a difference the clip boundary becomes the hole wall.
            BooleanOp::Intersection | BooleanOp::Difference => inside_subject,
            BooleanOp::Xor => unreachable!("handled above"),
        };
        if keep {
            if op == BooleanOp::Difference {
                kept.push((to, from));
            } else {
                kept.push((from, to));
            }
        }
    }

    let mut contours: Vec<Vec<(f64, f64)>> = stitch(kept)
        .into_iter()
        .filter(|contour| contour.len() >= 3 && polygon_area(contour).abs() > EPSILON)
        .collect();
    // Solid regions first, holes last, so a caller can treat the first contour
    // as the outer boundary and the rest as holes.
    contours.sort_by(|left, right| {
        polygon_area(right)
            .partial_cmp(&polygon_area(left))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    contours
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

    fn square(x: f64, y: f64, size: f64) -> Vec<(f64, f64)> {
        vec![(x, y), (x + size, y), (x + size, y + size), (x, y + size)]
    }

    /// Signed area of the result: holes subtract from their outer contour.
    fn total_area(contours: &[Vec<(f64, f64)>]) -> f64 {
        contours.iter().map(|contour| polygon_area(contour)).sum()
    }

    #[test]
    fn labels_are_unique() {
        let labels: Vec<&str> = BooleanOp::all().iter().map(|op| op.label()).collect();
        let mut sorted = labels.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), labels.len());
    }

    #[test]
    fn point_in_triangle() {
        let tri = [(0.0, 0.0), (10.0, 0.0), (5.0, 10.0)];
        assert!(point_in_polygon(5.0, 3.0, &tri));
        assert!(!point_in_polygon(20.0, 20.0, &tri));
    }

    #[test]
    fn area_is_signed() {
        let ccw = square(0.0, 0.0, 10.0);
        assert!((polygon_area(&ccw) - 100.0).abs() < 1e-9);
        let mut cw = ccw;
        cw.reverse();
        assert!((polygon_area(&cw) + 100.0).abs() < 1e-9);
    }

    #[test]
    fn convex_hull_wraps_points() {
        let points = [(0.0, 0.0), (1.0, 1.0), (2.0, 0.0), (1.0, 0.5)];
        let hull = convex_hull(&points);
        assert_eq!(hull.len(), 3);
        assert!(polygon_area(&hull) > 0.0);
    }

    #[test]
    fn overlapping_squares_have_the_expected_areas() {
        let a = square(0.0, 0.0, 4.0);
        let b = square(2.0, 2.0, 4.0);
        let intersection = boolean_contours(BooleanOp::Intersection, &a, &b);
        assert_eq!(intersection.len(), 1);
        assert!((total_area(&intersection) - 4.0).abs() < 1e-6);
        let union = boolean_contours(BooleanOp::Union, &a, &b);
        assert_eq!(union.len(), 1);
        assert!((total_area(&union) - 28.0).abs() < 1e-6);
        let difference = boolean_contours(BooleanOp::Difference, &a, &b);
        assert_eq!(difference.len(), 1);
        assert!((total_area(&difference) - 12.0).abs() < 1e-6);
        let xor = boolean_contours(BooleanOp::Xor, &a, &b);
        assert_eq!(xor.len(), 2);
        assert!((total_area(&xor) - 24.0).abs() < 1e-6);
    }

    #[test]
    fn results_are_counter_clockwise() {
        let a = square(0.0, 0.0, 4.0);
        let b = square(2.0, 2.0, 4.0);
        for op in [
            BooleanOp::Union,
            BooleanOp::Intersection,
            BooleanOp::Difference,
        ] {
            for contour in boolean_contours(op, &a, &b) {
                assert!(
                    polygon_area(&contour) > 0.0,
                    "{op:?} produced a clockwise contour"
                );
            }
        }
    }

    #[test]
    fn disjoint_polygons_do_not_interact() {
        let a = square(0.0, 0.0, 4.0);
        let b = square(10.0, 0.0, 4.0);
        assert_eq!(boolean_contours(BooleanOp::Intersection, &a, &b).len(), 0);
        let union = boolean_contours(BooleanOp::Union, &a, &b);
        assert_eq!(union.len(), 2);
        assert!((total_area(&union) - 32.0).abs() < 1e-6);
        let difference = boolean_contours(BooleanOp::Difference, &a, &b);
        assert_eq!(difference.len(), 1);
        assert!((total_area(&difference) - 16.0).abs() < 1e-6);
        let xor = boolean_contours(BooleanOp::Xor, &a, &b);
        assert_eq!(xor.len(), 2);
    }

    #[test]
    fn contained_polygon_is_returned_as_a_hole() {
        let a = square(0.0, 0.0, 8.0);
        let b = square(2.0, 2.0, 2.0);
        let intersection = boolean_contours(BooleanOp::Intersection, &a, &b);
        assert_eq!(intersection.len(), 1);
        assert!((total_area(&intersection) - 4.0).abs() < 1e-6);
        let union = boolean_contours(BooleanOp::Union, &a, &b);
        assert_eq!(union.len(), 1);
        assert!((total_area(&union) - 64.0).abs() < 1e-6);
        let difference = boolean_contours(BooleanOp::Difference, &a, &b);
        assert_eq!(difference.len(), 2);
        assert!((total_area(&difference) - 60.0).abs() < 1e-6);
        assert!(
            polygon_area(&difference[1]) < 0.0,
            "the hole must be clockwise"
        );
        let xor = boolean_contours(BooleanOp::Xor, &a, &b);
        assert!((total_area(&xor) - 60.0).abs() < 1e-6);
    }

    #[test]
    fn concave_shapes_are_clipped() {
        // An L shape (8 × 8 minus its top-right 4 × 4 corner).
        let l_shape = vec![
            (0.0, 0.0),
            (8.0, 0.0),
            (8.0, 4.0),
            (4.0, 4.0),
            (4.0, 8.0),
            (0.0, 8.0),
        ];
        let bar = vec![(2.0, 3.0), (6.0, 3.0), (6.0, 5.0), (2.0, 5.0)];
        let intersection = boolean_contours(BooleanOp::Intersection, &l_shape, &bar);
        assert_eq!(intersection.len(), 1);
        // The bar covers 4 × 2; its right half crosses the notch.
        assert!((total_area(&intersection) - 6.0).abs() < 1e-6);
        let union = boolean_contours(BooleanOp::Union, &l_shape, &bar);
        assert!((total_area(&union) - (48.0 + 8.0 - 6.0)).abs() < 1e-6);
    }

    #[test]
    fn clockwise_input_is_normalised() {
        let mut a = square(0.0, 0.0, 4.0);
        a.reverse();
        let mut b = square(2.0, 2.0, 4.0);
        b.reverse();
        let union = boolean_contours(BooleanOp::Union, &a, &b);
        assert_eq!(union.len(), 1);
        assert!((total_area(&union) - 28.0).abs() < 1e-6);
        assert!(polygon_area(&union[0]) > 0.0);
        let intersection = boolean_contours(BooleanOp::Intersection, &a, &b);
        assert_eq!(intersection.len(), 1);
        assert!((total_area(&intersection) - 4.0).abs() < 1e-6);
    }

    #[test]
    fn subtraction_can_split_a_polygon_in_two() {
        let bar = vec![(0.0, 0.0), (10.0, 0.0), (10.0, 2.0), (0.0, 2.0)];
        let cutter = vec![(4.0, -1.0), (6.0, -1.0), (6.0, 3.0), (4.0, 3.0)];
        let pieces = boolean_contours(BooleanOp::Difference, &bar, &cutter);
        assert_eq!(pieces.len(), 2);
        assert!((total_area(&pieces) - 16.0).abs() < 1e-6);
        for piece in &pieces {
            assert!(polygon_area(piece) > 0.0);
        }
        let intersection = boolean_contours(BooleanOp::Intersection, &bar, &cutter);
        assert_eq!(intersection.len(), 1);
        assert!((total_area(&intersection) - 4.0).abs() < 1e-6);
    }

    #[test]
    fn split_polyline_cuts_at_every_crossing() {
        let line = [(0.0, 0.0), (10.0, 0.0)];
        let square = square(4.0, -1.0, 2.0);
        let pieces = split_polyline(&line, false, &square);
        assert_eq!(pieces.len(), 3);
        assert_eq!(pieces[0], vec![(0.0, 0.0), (4.0, 0.0)]);
        assert_eq!(pieces[1], vec![(4.0, 0.0), (6.0, 0.0)]);
        assert_eq!(pieces[2], vec![(6.0, 0.0), (10.0, 0.0)]);
    }

    #[test]
    fn split_polyline_without_crossings_returns_one_piece() {
        let line = [(0.0, 0.0), (10.0, 0.0)];
        let square = square(20.0, 20.0, 2.0);
        let pieces = split_polyline(&line, false, &square);
        assert_eq!(pieces.len(), 1);
        assert_eq!(pieces[0], line.to_vec());
    }

    #[test]
    fn split_polyline_wraps_around_a_closed_path() {
        let ring = square(0.0, 0.0, 10.0);
        let cutter = vec![(5.0, -1.0), (5.0, 1.0), (7.0, 1.0), (7.0, -1.0)];
        let pieces = split_polyline(&ring, true, &cutter);
        assert_eq!(pieces.len(), 2);
        let mut lengths: Vec<usize> = pieces.iter().map(Vec::len).collect();
        lengths.sort_unstable();
        assert_eq!(lengths, vec![2, 6]);
    }

    #[test]
    fn invalid_input_is_rejected() {
        let square = square(0.0, 0.0, 4.0);
        assert_eq!(boolean_contours(BooleanOp::Union, &[], &square).len(), 0);
        assert_eq!(
            boolean_contours(BooleanOp::Union, &square[..2], &square).len(),
            0
        );
        let repeated = vec![(0.0, 0.0), (0.0, 0.0), (1.0, 1.0), (0.0, 0.0)];
        assert_eq!(
            boolean_contours(BooleanOp::Union, &repeated, &square).len(),
            0
        );
    }

    #[test]
    fn duplicate_points_are_removed() {
        let a = vec![
            (0.0, 0.0),
            (0.0, 0.0),
            (4.0, 0.0),
            (4.0, 4.0),
            (0.0, 4.0),
            (0.0, 0.0),
        ];
        let b = square(2.0, 2.0, 4.0);
        let union = boolean_contours(BooleanOp::Union, &a, &b);
        assert!((total_area(&union) - 28.0).abs() < 1e-6);
    }

    #[test]
    fn clip_polygon_still_works_for_convex_clipping() {
        let subject = square(0.0, 0.0, 10.0);
        let clip = square(2.0, 2.0, 6.0);
        let clipped = clip_polygon(&subject, &clip);
        assert!((polygon_area(&clipped) - 36.0).abs() < 1e-9);
        assert_eq!(clip_polygon(&subject, &square(20.0, 20.0, 2.0)).len(), 0);
    }
}
