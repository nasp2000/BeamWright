#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]

//! Trace the centre of drawn strokes into open polylines.
//!
//! Filled shapes are vectorized as outlines elsewhere; a drawing made of lines
//! is better engraved along its middle. The bitmap is thinned with the
//! Zhang–Suen algorithm and the one-pixel skeleton is walked into chains between
//! endpoints and junctions. Everything is written here, with no third-party
//! tracer.

use std::collections::HashSet;

use crate::geometry::path::Segment;
use crate::geometry::{Path, Point2};
use crate::raster::RasterBitmap;

type Pixel = (i32, i32);

/// Eight neighbours, clockwise from north.
const OFFSETS: [(i32, i32); 8] = [
    (0, -1),
    (1, -1),
    (1, 0),
    (1, 1),
    (0, 1),
    (-1, 1),
    (-1, 0),
    (-1, -1),
];

/// Trace the skeleton of the burnable pixels into millimetre polylines.
///
/// `y` is flipped because the raster grows downwards and the document upwards.
/// `tolerance_mm` removes points that stay within that distance of the line
/// between their neighbours. Closed loops repeat their first point at the end.
pub fn trace(
    bitmap: &RasterBitmap,
    width_mm: f64,
    height_mm: f64,
    tolerance_mm: f64,
) -> Vec<Vec<Point2>> {
    if bitmap.width == 0 || bitmap.height == 0 {
        return Vec::new();
    }
    let skeleton = thin(bitmap);
    if skeleton.is_empty() {
        return Vec::new();
    }
    let chains = walk(&skeleton);
    let mut polylines = Vec::new();
    for chain in chains {
        let points: Vec<(f64, f64)> = chain
            .iter()
            .map(|(x, y)| {
                (
                    (f64::from(*x) + 0.5) / bitmap.width as f64 * width_mm,
                    height_mm - (f64::from(*y) + 0.5) / bitmap.height as f64 * height_mm,
                )
            })
            .collect();
        let simplified = crate::geometry::polygon::simplify(&points, tolerance_mm);
        if simplified.len() < 2 {
            continue;
        }
        polylines.push(
            simplified
                .into_iter()
                .map(|(x, y)| Point2::new(x, y))
                .collect::<Vec<Point2>>(),
        );
    }
    polylines
}

/// Build a path from a traced centreline.
///
/// A loop repeats its first point at the end, so it becomes a closed path;
/// every other chain stays open.
pub fn to_path(polyline: &[Point2]) -> Path {
    let closed = polyline.len() > 2 && polyline[0].distance(polyline[polyline.len() - 1]) <= 1e-9;
    let points = if closed {
        &polyline[..polyline.len() - 1]
    } else {
        polyline
    };
    let mut segments = Vec::new();
    for pair in points.windows(2) {
        if pair[0].distance(pair[1]) > 1e-9 {
            segments.push(Segment::Line {
                from: pair[0],
                to: pair[1],
            });
        }
    }
    if closed && points.len() > 1 {
        let from = points[points.len() - 1];
        let to = points[0];
        if from.distance(to) > 1e-9 {
            segments.push(Segment::Line { from, to });
        }
    }
    Path { segments, closed }
}

/// Reduce the burnable pixels to a one-pixel-wide skeleton.
fn thin(bitmap: &RasterBitmap) -> HashSet<Pixel> {
    let mut pixels: HashSet<Pixel> = HashSet::new();
    for y in 0..bitmap.height {
        for x in 0..bitmap.width {
            if bitmap.get(x, y) {
                pixels.insert((x as i32, y as i32));
            }
        }
    }
    if pixels.is_empty() {
        return pixels;
    }
    loop {
        let mut removed = 0;
        for step in 0..2 {
            let snapshot: Vec<Pixel> = pixels.iter().copied().collect();
            let mut doomed = Vec::new();
            for pixel in snapshot {
                if should_remove(&pixels, pixel, step) {
                    doomed.push(pixel);
                }
            }
            for pixel in doomed {
                pixels.remove(&pixel);
                removed += 1;
            }
        }
        if removed == 0 {
            break;
        }
    }
    pixels
}

/// One Zhang–Suen sub-iteration test for a single pixel.
fn should_remove(pixels: &HashSet<Pixel>, pixel: Pixel, step: usize) -> bool {
    let p: [bool; 8] = std::array::from_fn(|index| {
        let (dx, dy) = OFFSETS[index];
        pixels.contains(&(pixel.0 + dx, pixel.1 + dy))
    });
    let count = p.iter().filter(|value| **value).count();
    if !(2..=6).contains(&count) {
        return false;
    }
    let mut transitions = 0;
    for index in 0..8 {
        if !p[index] && p[(index + 1) % 8] {
            transitions += 1;
        }
    }
    if transitions != 1 {
        return false;
    }
    let (north, east, south, west) = (p[0], p[2], p[4], p[6]);
    let corner = if step == 0 {
        east && south && (north || west)
    } else {
        north && west && (east || south)
    };
    !corner
}

/// Neighbours of a skeleton pixel, in the fixed clockwise order.
fn neighbours(pixels: &HashSet<Pixel>, pixel: Pixel) -> Vec<Pixel> {
    OFFSETS
        .iter()
        .map(|(dx, dy)| (pixel.0 + dx, pixel.1 + dy))
        .filter(|candidate| pixels.contains(candidate))
        .collect()
}

/// Connectivity of a skeleton pixel: the number of 0→1 transitions around it.
///
/// The raw count of eight neighbours overstates corners: a pixel on a straight
/// edge with one diagonal neighbour has three neighbours but still belongs to a
/// single stroke. The crossing number is 1 at an endpoint, 2 along a stroke and
/// 3 or more at a real junction.
fn connections(pixels: &HashSet<Pixel>, pixel: Pixel) -> usize {
    let mut present = [false; 8];
    for (index, (dx, dy)) in OFFSETS.iter().enumerate() {
        present[index] = pixels.contains(&(pixel.0 + dx, pixel.1 + dy));
    }
    let mut count = 0;
    for index in 0..8 {
        if !present[index] && present[(index + 1) % 8] {
            count += 1;
        }
    }
    count
}

/// Neighbours reachable by one step of a stroke.
///
/// A diagonal step that cuts a right angle is dropped when one of the two
/// orthogonal pixels joining the same ends is present. Without this, every
/// corner of an eight-connected skeleton is a small triangle and the walk would
/// report the same stroke several times.
fn step_neighbours(pixels: &HashSet<Pixel>, pixel: Pixel) -> Vec<Pixel> {
    neighbours(pixels, pixel)
        .into_iter()
        .filter(|candidate| {
            let dx = candidate.0 - pixel.0;
            let dy = candidate.1 - pixel.1;
            if dx == 0 || dy == 0 {
                return true;
            }
            !(pixels.contains(&(pixel.0 + dx, pixel.1))
                || pixels.contains(&(pixel.0, pixel.1 + dy)))
        })
        .collect()
}

/// Pick the next pixel of a chain, preferring a straight step over a diagonal.
fn next_pixel(pixels: &HashSet<Pixel>, current: Pixel, previous: Pixel) -> Option<Pixel> {
    let options: Vec<Pixel> = step_neighbours(pixels, current)
        .into_iter()
        .filter(|candidate| *candidate != previous)
        .collect();
    options
        .iter()
        .copied()
        .find(|candidate| (candidate.0 - current.0).abs() + (candidate.1 - current.1).abs() == 1)
        .or_else(|| options.first().copied())
}

/// Walk the skeleton into chains between endpoints, junctions and loops.
fn walk(pixels: &HashSet<Pixel>) -> Vec<Vec<Pixel>> {
    let mut visited: HashSet<(Pixel, Pixel)> = HashSet::new();
    let mut chains = Vec::new();

    let mut nodes: Vec<Pixel> = pixels
        .iter()
        .copied()
        .filter(|pixel| connections(pixels, *pixel) != 2)
        .collect();
    nodes.sort_unstable();
    for node in nodes {
        for start in step_neighbours(pixels, node) {
            if visited.contains(&(node, start)) {
                continue;
            }
            chains.push(walk_chain(pixels, &mut visited, node, start));
        }
    }

    // Everything left belongs to a closed loop with no endpoint.
    let mut remaining: Vec<Pixel> = pixels.iter().copied().collect();
    remaining.sort_unstable();
    for start in remaining {
        for next in step_neighbours(pixels, start) {
            if visited.contains(&(start, next)) {
                continue;
            }
            chains.push(walk_chain(pixels, &mut visited, start, next));
        }
    }

    chains
        .into_iter()
        .filter(|chain| chain.len() >= 2)
        .collect()
}

/// Follow one chain from `from` to `to`, marking every edge as visited.
fn walk_chain(
    pixels: &HashSet<Pixel>,
    visited: &mut HashSet<(Pixel, Pixel)>,
    from: Pixel,
    to: Pixel,
) -> Vec<Pixel> {
    let mut chain = vec![from];
    let mut previous = from;
    let mut current = to;
    loop {
        chain.push(current);
        visited.insert((previous, current));
        visited.insert((current, previous));
        if current == from || connections(pixels, current) != 2 {
            break;
        }
        let Some(next) = next_pixel(pixels, current, previous) else {
            break;
        };
        previous = current;
        current = next;
    }
    chain
}

#[cfg(test)]
mod tests {
    use super::{to_path, trace};
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
    fn a_horizontal_line_becomes_one_polyline() {
        let mut burn = vec![false; 7 * 3];
        for x in 1..6 {
            burn[7 + x] = true;
        }
        let image = bitmap(7, 3, &burn);
        let lines = trace(&image, 7.0, 3.0, 0.0);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].len(), 2);
        // The stroke keeps its y and spans the drawn pixels in millimetres.
        assert!((lines[0][0].x - 1.5).abs() < 1e-9);
        assert!((lines[0][1].x - 5.5).abs() < 1e-9);
        assert!((lines[0][0].y - 1.5).abs() < 1e-9);
    }

    #[test]
    fn a_hollow_square_becomes_one_closed_loop() {
        let mut burn = vec![false; 5 * 5];
        for index in 0..5 {
            burn[index] = true;
            burn[4 * 5 + index] = true;
            burn[index * 5] = true;
            burn[index * 5 + 4] = true;
        }
        let image = bitmap(5, 5, &burn);
        let lines = trace(&image, 5.0, 5.0, 0.0);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].len() >= 4);
        let first = lines[0][0];
        let last = lines[0][lines[0].len() - 1];
        assert!((first.x - last.x).abs() < 1e-9);
        assert!((first.y - last.y).abs() < 1e-9);
    }

    #[test]
    fn a_cross_produces_more_than_one_chain() {
        let mut burn = vec![false; 5 * 5];
        for index in 0..5 {
            burn[2 * 5 + index] = true;
            burn[index * 5 + 2] = true;
        }
        let image = bitmap(5, 5, &burn);
        let lines = trace(&image, 5.0, 5.0, 0.0);
        assert!(lines.len() >= 2, "a junction was not split: {lines:?}");
    }

    #[test]
    #[allow(clippy::assert_is_empty)]
    fn an_empty_bitmap_has_no_centerline() {
        let image = bitmap(4, 4, &[false; 16]);
        assert!(trace(&image, 4.0, 4.0, 0.0).is_empty());
    }

    #[test]
    fn the_coordinates_use_millimetres_and_flip_y() {
        let mut burn = vec![false; 4 * 4];
        burn[3] = true;
        burn[7] = true;
        burn[11] = true;
        let image = bitmap(4, 4, &burn);
        let lines = trace(&image, 40.0, 40.0, 0.0);
        assert_eq!(lines.len(), 1);
        let mut ys: Vec<f64> = lines[0].iter().map(|point| point.y).collect();
        ys.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert!((ys[0] - 15.0).abs() < 1e-9);
        assert!((ys[ys.len() - 1] - 35.0).abs() < 1e-9);
        for point in &lines[0] {
            assert!((0.0..=40.0).contains(&point.x));
        }
    }

    #[test]
    #[allow(clippy::assert_is_empty)]
    fn a_traced_centerline_is_an_open_path() {
        let line = [
            crate::geometry::Point2::new(0.0, 0.0),
            crate::geometry::Point2::new(10.0, 0.0),
            crate::geometry::Point2::new(10.0, 5.0),
        ];
        let path = to_path(&line);
        assert!(!path.closed);
        assert_eq!(path.segments.len(), 2);
        assert!(to_path(&line[..1]).segments.is_empty());
    }

    #[test]
    fn a_loop_becomes_a_closed_path() {
        let square = [
            crate::geometry::Point2::new(0.0, 0.0),
            crate::geometry::Point2::new(10.0, 0.0),
            crate::geometry::Point2::new(10.0, 10.0),
            crate::geometry::Point2::new(0.0, 10.0),
            crate::geometry::Point2::new(0.0, 0.0),
        ];
        let path = to_path(&square);
        assert!(path.closed);
        assert_eq!(path.segments.len(), 4);
    }
}
