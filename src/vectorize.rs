#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
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
        loop {
            let Some(next) = edges.get_mut(&current).and_then(Vec::pop) else {
                break;
            };
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
                        *x as f64 / width as f64 * width_mm,
                        height_mm - *y as f64 / height as f64 * height_mm,
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
    use super::{to_path, trace};
    use crate::raster::RasterBitmap;

    fn bitmap(width: usize, height: usize, burn: Vec<bool>) -> RasterBitmap {
        RasterBitmap {
            width,
            height,
            burn,
        }
    }

    #[test]
    fn a_square_becomes_one_closed_contour() {
        let image = bitmap(4, 4, vec![true; 16]);
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
        let image = bitmap(4, 4, burn);
        let loops = trace(&image, 4.0, 4.0, 0.0);
        assert_eq!(loops.len(), 2);
    }

    #[test]
    fn an_empty_bitmap_has_no_contours() {
        let image = bitmap(4, 4, vec![false; 16]);
        assert!(trace(&image, 4.0, 4.0, 0.0).is_empty());
    }

    #[test]
    fn the_coordinates_use_millimetres_and_flip_y() {
        let mut burn = vec![false; 16];
        burn[0] = true;
        let image = bitmap(4, 4, burn);
        let loops = trace(&image, 40.0, 40.0, 0.0);
        assert_eq!(loops.len(), 1);
        let xs: Vec<f64> = loops[0].iter().map(|point| point.x).collect();
        let ys: Vec<f64> = loops[0].iter().map(|point| point.y).collect();
        assert!(xs.iter().all(|x| (0.0..=10.0).contains(x)));
        assert!(ys.iter().all(|y| (30.0..=40.0).contains(y)));
    }
}
