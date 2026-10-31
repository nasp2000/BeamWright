//! Convert text to vector outlines with the bundled font (PLAN.md §7).
//!
//! The outline geometry is produced from a font embedded in the binary, so it
//! never depends on a font installed on the machine. The font is `DejaVu Sans`
//! (`Bitstream Vera` licence, `assets/fonts/LICENSE-DejaVu.txt`).

use ab_glyph::{Font, FontRef, GlyphId, OutlineCurve};

use crate::geometry::Path;
use crate::objects::TextObject;

/// `DejaVu Sans`, embedded so outline conversion works everywhere.
const FONT_DATA: &[u8] = include_bytes!("../../assets/fonts/DejaVuSans.ttf");

/// Samples per curve; enough for engraving-sized text.
const CURVE_STEPS: usize = 12;

/// Convert a text object into closed contours in the text object's local
/// coordinates.
///
/// Every glyph may contribute several contours (one per closed loop, so the hole
/// of an "o" is its own contour). The caller places them with the object's
/// transform. Layout follows the same model as the canvas: line spacing,
/// letter spacing and left/centre/right alignment around the anchor.
pub fn text_to_paths(text: &TextObject) -> Result<Vec<Path>, String> {
    text.validate()?;
    let font = FontRef::try_from_slice(FONT_DATA)
        .map_err(|error| format!("the bundled font is unreadable: {error}"))?;
    let units = font.units_per_em().unwrap_or(1000.0);
    if units <= 0.0 {
        return Err("the bundled font has no units per em".to_string());
    }
    // Millimetres per font unit.
    let factor = text.size / f64::from(units);
    let ascent = f64::from(font.ascent_unscaled()) * factor;
    let top = text.y + text.block_height();
    let left = text.left();
    let mut paths = Vec::new();
    for (line_index, line) in text.lines().iter().enumerate() {
        #[allow(clippy::cast_precision_loss)]
        let baseline = top - line_index as f64 * text.line_height() - ascent;
        let mut pen = left;
        let mut previous: Option<GlyphId> = None;
        for character in line.chars() {
            let id = font.glyph_id(character);
            if let Some(previous) = previous {
                pen += f64::from(font.kern_unscaled(previous, id)) * factor;
            }
            if let Some(outline) = font.outline(id) {
                paths.extend(contours(&outline.curves, factor, pen, baseline));
            }
            pen += f64::from(font.h_advance_unscaled(id)) * factor + text.letter_spacing;
            previous = Some(id);
        }
    }
    Ok(paths)
}

/// Split a glyph outline into closed contours, scaled and offset to a baseline.
fn contours(curves: &[OutlineCurve], factor: f64, offset_x: f64, offset_y: f64) -> Vec<Path> {
    let mut paths = Vec::new();
    let mut current: Vec<(f64, f64)> = Vec::new();
    let mut finish = |points: &mut Vec<(f64, f64)>| {
        if points.len() >= 3 {
            paths.push(Path::from_points(points, true));
        }
        points.clear();
    };
    let place = |point: ab_glyph::Point| {
        (
            offset_x + f64::from(point.x) * factor,
            offset_y + f64::from(point.y) * factor,
        )
    };
    for curve in curves {
        let start = match curve {
            OutlineCurve::Line(start, _)
            | OutlineCurve::Quad(start, _, _)
            | OutlineCurve::Cubic(start, _, _, _) => place(*start),
        };
        if let Some(last) = current.last() {
            if (last.0 - start.0).abs() > 1e-6 || (last.1 - start.1).abs() > 1e-6 {
                finish(&mut current);
            }
        }
        if current.is_empty() {
            current.push(start);
        }
        match curve {
            OutlineCurve::Line(_, end) => current.push(place(*end)),
            OutlineCurve::Quad(start, control, end) => {
                let start = place(*start);
                let control = place(*control);
                let end = place(*end);
                for step in 1..=CURVE_STEPS {
                    #[allow(clippy::cast_precision_loss)]
                    let t = step as f64 / CURVE_STEPS as f64;
                    let one = 1.0 - t;
                    current.push((
                        one * one * start.0 + 2.0 * one * t * control.0 + t * t * end.0,
                        one * one * start.1 + 2.0 * one * t * control.1 + t * t * end.1,
                    ));
                }
            }
            OutlineCurve::Cubic(start, control_a, control_b, end) => {
                let start = place(*start);
                let control_a = place(*control_a);
                let control_b = place(*control_b);
                let end = place(*end);
                for step in 1..=CURVE_STEPS {
                    #[allow(clippy::cast_precision_loss)]
                    let t = step as f64 / CURVE_STEPS as f64;
                    let one = 1.0 - t;
                    current.push((
                        one * one * one * start.0
                            + 3.0 * one * one * t * control_a.0
                            + 3.0 * one * t * t * control_b.0
                            + t * t * t * end.0,
                        one * one * one * start.1
                            + 3.0 * one * one * t * control_a.1
                            + 3.0 * one * t * t * control_b.1
                            + t * t * t * end.1,
                    ));
                }
            }
        }
    }
    finish(&mut current);
    paths
}

#[cfg(test)]
mod tests {
    use super::text_to_paths;
    use crate::geometry::Path;
    use crate::objects::{TextAlign, TextObject};

    fn text(content: &str) -> TextObject {
        TextObject {
            content: content.to_string(),
            size: 10.0,
            ..TextObject::default()
        }
    }

    #[test]
    fn a_letter_becomes_a_closed_contour() {
        let paths = text_to_paths(&text("S")).unwrap();
        assert_eq!(paths.len(), 1);
        assert!(paths[0].closed);
        assert!(paths[0].segments.len() > 8);
        let (min, max) = paths[0].bounding_box().unwrap();
        // 10 mm text is a few millimetres high and wide.
        assert!(max.y - min.y > 5.0);
        assert!(max.x - min.x > 3.0);
    }

    #[test]
    fn holes_become_their_own_contours() {
        let paths = text_to_paths(&text("o")).unwrap();
        assert_eq!(paths.len(), 2);
        let areas: Vec<f64> = paths
            .iter()
            .map(|path| {
                let points = path.flatten(0.05);
                let pairs: Vec<(f64, f64)> = points.iter().map(|p| (p.x, p.y)).collect();
                crate::geometry::boolean::polygon_area(&pairs).abs()
            })
            .collect();
        assert!(areas[0] > 1.0);
        assert!(areas[1] > 0.1);
        assert!((areas[0] - areas[1]).abs() > 1e-6);
    }

    #[test]
    fn lines_are_stacked_and_alignment_moves_the_block() {
        let left = text_to_paths(&text("AB\nCD")).unwrap();
        let mut centred = text("AB\nCD");
        centred.align = TextAlign::Center;
        let centred_paths = text_to_paths(&centred).unwrap();
        assert_eq!(left.len(), centred_paths.len());
        let left_min = left
            .iter()
            .filter_map(Path::bounding_box)
            .map(|(min, _)| min.x)
            .fold(f64::INFINITY, f64::min);
        let centred_min = centred_paths
            .iter()
            .filter_map(Path::bounding_box)
            .map(|(min, _)| min.x)
            .fold(f64::INFINITY, f64::min);
        assert!(centred_min < left_min);
        let top = left
            .iter()
            .filter_map(Path::bounding_box)
            .map(|(_, max)| max.y)
            .fold(f64::NEG_INFINITY, f64::max);
        let bottom = left
            .iter()
            .filter_map(Path::bounding_box)
            .map(|(min, _)| min.y)
            .fold(f64::INFINITY, f64::min);
        assert!(top - bottom > 10.0);
    }

    #[test]
    fn letter_spacing_widens_the_result() {
        let normal = text_to_paths(&text("AA")).unwrap();
        let mut spaced = text("AA");
        spaced.letter_spacing = 4.0;
        let spaced = text_to_paths(&spaced).unwrap();
        let width = |paths: &[crate::geometry::Path]| {
            let min = paths
                .iter()
                .filter_map(Path::bounding_box)
                .map(|(min, _)| min.x)
                .fold(f64::INFINITY, f64::min);
            let max = paths
                .iter()
                .filter_map(Path::bounding_box)
                .map(|(_, max)| max.x)
                .fold(f64::NEG_INFINITY, f64::max);
            max - min
        };
        assert!(width(&spaced) > width(&normal) + 3.0);
    }
}
