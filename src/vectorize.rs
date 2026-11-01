#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss
)]

//! Trace a burn bitmap into closed vector contours.
//!
//! The algorithm is a boundary-edge walk written for this project: every burn
//! pixel contributes the edges that separate it from an unburned pixel, the
//! edges are chained into loops and the loops are simplified. No third-party
//! tracer is used.

use std::collections::HashMap;

use crate::geometry::path::Segment;
use crate::geometry::{Path, Point2};
use crate::raster::RasterBitmap;

type Pixel = (i32, i32);
type Edge = (Pixel, Pixel);

/// Trace every boundary loop of the bitmap into millimetre coordinates.
///
/// `y` is flipped because the raster grows downwards and the document upwards.
/// `tolerance_mm` removes points that stay within that distance of the line
/// between their neighbours.
pub fn trace(
    bitmap: &RasterBitmap,
    width_mm: f64,
    height_mm: f64,
    tolerance_mm: f64,
) -> Vec<Vec<Point2>> {
    let width = bitmap.width;
    let height = bitmap.height;
    if width == 0 || height == 0 {
        return Vec::new();
    }
    let mut edges: HashMap<Pixel, Vec<Pixel>> = HashMap::new();
    for y in 0..height {
        for x in 0..width {
            if !bitmap.get(x, y) {
                continue;
            }
            let (px, py) = (x as i32, y as i32);
            let below = y + 1 < height && bitmap.get(x, y + 1);
            let above = y > 0 && bitmap.get(x, y - 1);
            let left = x > 0 && bitmap.get(x - 1, y);
            let right = x + 1 < width && bitmap.get(x + 1, y);
            if !above {
                edges.entry((px, py)).or_default().push((px + 1, py));
            }
            if !right {
                edges
                    .entry((px + 1, py))
                    .or_default()
                    .push((px + 1, py + 1));
            }
            if !below {
                edges
                    .entry((px + 1, py + 1))
                    .or_default()
                    .push((px, py + 1));
            }
            if !left {
                edges.entry((px, py + 1)).or_default().push((px, py));
            }
        }
    }
    let mut loops = Vec::new();
    while let Some(start) = edges.keys().next().copied() {
        let mut points = vec![start];
        let mut current = start;
        while let Some(next) = edges.get_mut(&current).and_then(Vec::pop) {
            points.push(next);
            current = next;
            if current == start {
                break;
            }
        }
        edges.retain(|_, targets| !targets.is_empty());
        if points.len() >= 4 {
            let mut contour: Vec<Point2> = points
                .iter()
                .map(|(x, y)| {
                    Point2::new(
                        f64::from(*x) / width as f64 * width_mm,
                        height_mm - f64::from(*y) / height as f64 * height_mm,
                    )
                })
                .collect();
            contour = simplify(&contour, tolerance_mm);
            if contour.len() >= 3 {
                loops.push(contour);
            }
        }
    }
    loops
}

/// Group traced contours into outers and the holes directly inside them.
///
/// Each group starts with the outer loop and continues with every contour
/// contained by it. A contour inside a hole is treated as a new island and gets
/// its own group, so no contour is lost.
pub fn group_contours(contours: &[Vec<Point2>]) -> Vec<Vec<usize>> {
    if contours.is_empty() {
        return Vec::new();
    }
    let polygons: Vec<Vec<(f64, f64)>> = contours
        .iter()
        .map(|contour| contour.iter().map(|point| (point.x, point.y)).collect())
        .collect();
    let areas: Vec<f64> = polygons
        .iter()
        .map(|polygon| crate::geometry::boolean::polygon_area(polygon).abs())
        .collect();
    let mut parent: Vec<Option<usize>> = vec![None; contours.len()];
    for (index, contour) in contours.iter().enumerate() {
        let point = contour[0];
        let mut best: Option<usize> = None;
        for (candidate, polygon) in polygons.iter().enumerate() {
            if candidate == index || areas[candidate] <= areas[index] {
                continue;
            }
            if crate::geometry::boolean::point_in_polygon(point.x, point.y, polygon)
                && best.is_none_or(|current| areas[candidate] < areas[current])
            {
                best = Some(candidate);
            }
        }
        parent[index] = best;
    }

    let mut order: Vec<usize> = (0..contours.len()).collect();
    order.sort_by(|left, right| areas[*right].total_cmp(&areas[*left]).then(left.cmp(right)));
    let mut groups: Vec<Vec<usize>> = Vec::new();
    let mut group_of: Vec<Option<usize>> = vec![None; contours.len()];
    for index in order {
        match parent[index] {
            Some(parent) if group_of[parent].is_some() => {
                let group = group_of[parent].expect("checked above");
                groups[group].push(index);
            }
            _ => {
                group_of[index] = Some(groups.len());
                groups.push(vec![index]);
            }
        }
    }
    groups
}

/// Build a closed path from a traced contour.
pub fn to_path(contour: &[Point2]) -> Path {
    let mut segments = Vec::new();
    for index in 0..contour.len() {
        let from = contour[index];
        let to = contour[(index + 1) % contour.len()];
        if from.distance(to) > 1e-9 {
            segments.push(Segment::Line { from, to });
        }
    }
    Path {
        segments,
        closed: true,
    }
}

/// Build a closed path where gentle runs use cubic curves and sharp corners
/// stay straight.
///
/// Every contour point is a knot; the control points follow the local tangent,
/// except at corners (turns wider than `corner_degrees`), where the control
/// point lies on the edge so the corner is not rounded. This turns the fine
/// polyline left by the tracer into a small number of curves without moving the
/// traced outline.
pub fn to_curved_path(contour: &[Point2], corner_degrees: f64) -> Path {
    let count = contour.len();
    if count < 4 {
        return to_path(contour);
    }
    let corner_limit = corner_degrees.max(1.0);
    let is_corner = |index: usize| {
        let previous = contour[(index + count - 1) % count];
        let current = contour[index];
        let next = contour[(index + 1) % count];
        let incoming = (current.x - previous.x, current.y - previous.y);
        let outgoing = (next.x - current.x, next.y - current.y);
        let dot = incoming.0 * outgoing.0 + incoming.1 * outgoing.1;
        let cross = incoming.0 * outgoing.1 - incoming.1 * outgoing.0;
        dot <= 0.0 || cross.abs().atan2(dot).to_degrees() > corner_limit
    };
    let tangent = |index: usize| {
        let current = contour[index];
        let (dx, dy) = if is_corner(index) {
            let next = contour[(index + 1) % count];
            (next.x - current.x, next.y - current.y)
        } else {
            let previous = contour[(index + count - 1) % count];
            let next = contour[(index + 1) % count];
            (next.x - previous.x, next.y - previous.y)
        };
        let length = dx.hypot(dy);
        if length <= 1e-9 {
            (0.0, 0.0)
        } else {
            (dx / length, dy / length)
        }
    };
    let mut path = Path::new();
    path.closed = true;
    for index in 0..count {
        let from = contour[index];
        let to = contour[(index + 1) % count];
        let next = (index + 1) % count;
        if is_corner(index) && is_corner(next) {
            path.push(Segment::Line { from, to });
            continue;
        }
        let chord = from.distance(to);
        if chord <= 1e-9 {
            continue;
        }
        let start_tangent = tangent(index);
        let end_tangent = tangent(next);
        let control_1 = Point2::new(
            from.x + start_tangent.0 * chord / 3.0,
            from.y + start_tangent.1 * chord / 3.0,
        );
        let control_2 = Point2::new(
            to.x - end_tangent.0 * chord / 3.0,
            to.y - end_tangent.1 * chord / 3.0,
        );
        path.push(Segment::Cubic(crate::geometry::bezier::CubicBezier {
            p0: from,
            p1: control_1,
            p2: control_2,
            p3: to,
        }));
    }
    path
}

fn simplify(points: &[Point2], tolerance: f64) -> Vec<Point2> {
    if points.len() <= 3 || tolerance <= 0.0 {
        return points.to_vec();
    }
    let mut keep = vec![false; points.len()];
    keep[0] = true;
    keep[points.len() - 1] = true;
    douglas_peucker(points, 0, points.len() - 1, tolerance, &mut keep);
    points
        .iter()
        .zip(&keep)
        .filter_map(|(point, keep)| keep.then_some(*point))
        .collect()
}

fn douglas_peucker(
    points: &[Point2],
    first: usize,
    last: usize,
    tolerance: f64,
    keep: &mut [bool],
) {
    if last <= first + 1 {
        return;
    }
    let (a, b) = (points[first], points[last]);
    let mut worst = 0.0;
    let mut index = first;
    for (offset, point) in points.iter().enumerate().take(last).skip(first + 1) {
        let distance = distance_to_line(*point, a, b);
        if distance > worst {
            worst = distance;
            index = offset;
        }
    }
    if worst > tolerance {
        keep[index] = true;
        douglas_peucker(points, first, index, tolerance, keep);
        douglas_peucker(points, index, last, tolerance, keep);
    }
}

fn distance_to_line(point: Point2, a: Point2, b: Point2) -> f64 {
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    let length = dx.hypot(dy);
    if length <= 1e-9 {
        return point.distance(a);
    }
    ((point.x - a.x) * dy - (point.y - a.y) * dx).abs() / length
}

#[cfg(test)]
mod tests {
    use super::{to_curved_path, to_path, trace};
    use crate::geometry::{Point2, path::Segment};
    use crate::raster::RasterBitmap;

    fn bitmap(width: usize, height: usize, burn: &[bool]) -> RasterBitmap {
        RasterBitmap {
            width,
            height,
            power: burn
                .iter()
                .map(|value| if *value { 255 } else { 0 })
                .collect(),
        }
    }

    #[test]
    fn a_square_becomes_one_closed_contour() {
        let image = bitmap(4, 4, &[true; 16]);
        let loops = trace(&image, 40.0, 40.0, 0.0);
        assert_eq!(loops.len(), 1);
        let path = to_path(&loops[0]);
        assert!(path.closed);
        assert!(path.segments.len() >= 4);
    }

    #[test]
    fn separate_areas_become_separate_contours() {
        let mut burn = vec![false; 16];
        burn[0] = true;
        burn[15] = true;
        let image = bitmap(4, 4, &burn);
        let loops = trace(&image, 4.0, 4.0, 0.0);
        assert_eq!(loops.len(), 2);
    }

    #[test]
    #[allow(clippy::assert_is_empty)]
    fn an_empty_bitmap_has_no_contours() {
        let image = bitmap(4, 4, &[false; 16]);
        assert!(trace(&image, 4.0, 4.0, 0.0).is_empty());
    }

    fn square(left: f64, bottom: f64, size: f64) -> Vec<Point2> {
        vec![
            Point2::new(left, bottom),
            Point2::new(left + size, bottom),
            Point2::new(left + size, bottom + size),
            Point2::new(left, bottom + size),
        ]
    }

    #[test]
    fn nested_contours_become_one_outer_with_a_hole() {
        let groups = super::group_contours(&[square(0.0, 0.0, 20.0), square(5.0, 5.0, 10.0)]);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0], vec![0, 1]);
    }

    #[test]
    fn separate_contours_stay_separate() {
        let groups = super::group_contours(&[square(0.0, 0.0, 5.0), square(20.0, 20.0, 5.0)]);
        assert_eq!(groups.len(), 2);
    }

    #[test]
    fn an_island_inside_a_hole_is_its_own_group() {
        let groups = super::group_contours(&[
            square(0.0, 0.0, 30.0),
            square(5.0, 5.0, 20.0),
            square(10.0, 10.0, 5.0),
        ]);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0], vec![0, 1]);
        assert_eq!(groups[1], vec![2]);
    }

    #[test]
    fn a_square_keeps_its_corners_straight() {
        let square = vec![
            Point2::new(0.0, 0.0),
            Point2::new(10.0, 0.0),
            Point2::new(10.0, 10.0),
            Point2::new(0.0, 10.0),
        ];
        let path = to_curved_path(&square, 40.0);
        assert!(path.closed);
        assert_eq!(path.segments.len(), 4);
        assert!(
            path.segments
                .iter()
                .all(|segment| matches!(segment, Segment::Line { .. }))
        );
    }

    #[test]
    fn a_smooth_circle_becomes_cubic_segments() {
        let circle: Vec<Point2> = (0..16)
            .map(|index| {
                let angle = std::f64::consts::TAU * f64::from(index) / 16.0;
                Point2::new(10.0 * angle.cos(), 10.0 * angle.sin())
            })
            .collect();
        let path = to_curved_path(&circle, 40.0);
        assert!(path.closed);
        assert_eq!(path.segments.len(), 16);
        assert!(
            path.segments
                .iter()
                .all(|segment| matches!(segment, Segment::Cubic(_)))
        );
        // Every knot is preserved exactly.
        for (index, segment) in path.segments.iter().enumerate() {
            let Segment::Cubic(curve) = segment else {
                panic!("expected a cubic");
            };
            assert!(curve.p0.distance(circle[index]) < 1e-9);
            assert!(curve.p3.distance(circle[(index + 1) % 16]) < 1e-9);
        }
    }

    #[test]
    fn short_contours_fall_back_to_lines() {
        let triangle = vec![
            Point2::new(0.0, 0.0),
            Point2::new(10.0, 0.0),
            Point2::new(5.0, 8.0),
        ];
        let path = to_curved_path(&triangle, 40.0);
        assert!(path.closed);
        assert_eq!(path.segments.len(), 3);
        assert!(
            path.segments
                .iter()
                .all(|segment| matches!(segment, Segment::Line { .. }))
        );
    }

    #[test]
    fn the_coordinates_use_millimetres_and_flip_y() {
        let mut burn = vec![false; 16];
        burn[0] = true;
        let image = bitmap(4, 4, &burn);
        let loops = trace(&image, 40.0, 40.0, 0.0);
        assert_eq!(loops.len(), 1);
        let xs: Vec<f64> = loops[0].iter().map(|point| point.x).collect();
        let ys: Vec<f64> = loops[0].iter().map(|point| point.y).collect();
        assert!(xs.iter().all(|x| (0.0..=10.0).contains(x)));
        assert!(ys.iter().all(|y| (30.0..=40.0).contains(y)));
    }
}
