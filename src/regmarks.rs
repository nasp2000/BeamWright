//! Registration marks around a job, for print-and-cut alignment.
//!
//! The marks are ordinary open paths in millimetres: the user can move, scale
//! or delete them like any other object. Nothing here knows about a camera or
//! a machine; alignment stays a manual step.

use crate::geometry::Point2;

/// Shape of one registration mark.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MarkStyle {
    /// Two crossing strokes centred on the corner.
    #[default]
    Cross,
    /// A right angle drawn inwards from the corner.
    Corner,
}

impl MarkStyle {
    pub fn label(self) -> &'static str {
        match self {
            MarkStyle::Cross => "Cross",
            MarkStyle::Corner => "Corner",
        }
    }

    pub fn all() -> [MarkStyle; 2] {
        [MarkStyle::Cross, MarkStyle::Corner]
    }
}

/// Size and placement of the marks around the job.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Options {
    /// Length of each stroke, in millimetres.
    pub size_mm: f64,
    /// Distance between the job bounds and the marks, in millimetres.
    pub margin_mm: f64,
    pub style: MarkStyle,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            size_mm: 5.0,
            margin_mm: 3.0,
            style: MarkStyle::Cross,
        }
    }
}

impl Options {
    pub fn validate(&self) -> Result<(), String> {
        if !self.size_mm.is_finite() || !(0.5..=100.0).contains(&self.size_mm) {
            return Err("mark size must be between 0.5 and 100 mm".to_string());
        }
        if !self.margin_mm.is_finite() || !(0.0..=200.0).contains(&self.margin_mm) {
            return Err("mark margin must be between 0 and 200 mm".to_string());
        }
        Ok(())
    }
}

/// Four marks around `bounds`, one per corner of the expanded rectangle.
///
/// Crosses are centred on the corner; corners are right angles drawn inwards
/// from it. The marks stay outside the job, so they never overlap the artwork.
pub fn marks(bounds: (Point2, Point2), options: &Options) -> Result<Vec<Vec<Point2>>, String> {
    options.validate()?;
    let (min, max) = bounds;
    if !min.x.is_finite()
        || !min.y.is_finite()
        || !max.x.is_finite()
        || !max.y.is_finite()
        || max.x < min.x
        || max.y < min.y
    {
        return Err("the registration area is invalid".to_string());
    }
    let left = min.x - options.margin_mm;
    let right = max.x + options.margin_mm;
    let bottom = min.y - options.margin_mm;
    let top = max.y + options.margin_mm;
    let half = options.size_mm / 2.0;
    // Corner point and the direction towards the inside of the job.
    let corners = [
        (left, bottom, 1.0, 1.0),
        (right, bottom, -1.0, 1.0),
        (right, top, -1.0, -1.0),
        (left, top, 1.0, -1.0),
    ];
    let mut outlines = Vec::with_capacity(8);
    for (x, y, inward_x, inward_y) in corners {
        match options.style {
            MarkStyle::Cross => {
                outlines.push(vec![Point2::new(x - half, y), Point2::new(x + half, y)]);
                outlines.push(vec![Point2::new(x, y - half), Point2::new(x, y + half)]);
            }
            MarkStyle::Corner => {
                outlines.push(vec![
                    Point2::new(x + inward_x * options.size_mm, y),
                    Point2::new(x, y),
                    Point2::new(x, y + inward_y * options.size_mm),
                ]);
            }
        }
    }
    Ok(outlines)
}

#[cfg(test)]
mod tests {
    use super::{MarkStyle, Options, marks};
    use crate::geometry::Point2;

    fn bounds() -> (Point2, Point2) {
        (Point2::new(0.0, 0.0), Point2::new(10.0, 6.0))
    }

    #[test]
    fn crosses_are_centred_on_the_expanded_corners() {
        let options = Options {
            size_mm: 4.0,
            margin_mm: 2.0,
            style: MarkStyle::Cross,
        };
        let outlines = marks(bounds(), &options).unwrap();
        assert_eq!(outlines.len(), 8);
        // Lower-left cross: horizontal from (-4, -2) to (0, -2).
        assert_eq!(outlines[0][0], Point2::new(-4.0, -2.0));
        assert_eq!(outlines[0][1], Point2::new(0.0, -2.0));
        assert_eq!(outlines[1][0], Point2::new(-2.0, -4.0));
        assert_eq!(outlines[1][1], Point2::new(-2.0, 0.0));
        // Upper-right cross is centred on (12, 8).
        assert_eq!(outlines[4][0], Point2::new(10.0, 8.0));
        assert_eq!(outlines[4][1], Point2::new(14.0, 8.0));
    }

    #[test]
    fn corner_marks_are_right_angles_drawn_inwards() {
        let options = Options {
            size_mm: 3.0,
            margin_mm: 1.0,
            style: MarkStyle::Corner,
        };
        let outlines = marks(bounds(), &options).unwrap();
        assert_eq!(outlines.len(), 4);
        // Lower-left: inwards means right and up.
        assert_eq!(outlines[0][0], Point2::new(2.0, -1.0));
        assert_eq!(outlines[0][1], Point2::new(-1.0, -1.0));
        assert_eq!(outlines[0][2], Point2::new(-1.0, 2.0));
        // Upper-right: inwards means left and down.
        assert_eq!(outlines[2][0], Point2::new(8.0, 7.0));
        assert_eq!(outlines[2][1], Point2::new(11.0, 7.0));
        assert_eq!(outlines[2][2], Point2::new(11.0, 4.0));
    }

    #[test]
    fn marks_stay_outside_the_job() {
        let options = Options {
            size_mm: 5.0,
            margin_mm: 2.0,
            style: MarkStyle::Cross,
        };
        let (min, max) = bounds();
        for outline in marks(bounds(), &options).unwrap() {
            for point in outline {
                let outside = point.x <= min.x + 1e-9
                    || point.x >= max.x - 1e-9
                    || point.y <= min.y + 1e-9
                    || point.y >= max.y - 1e-9;
                assert!(outside, "a mark entered the job: {point:?}");
            }
        }
    }

    #[test]
    fn invalid_options_and_bounds_are_rejected() {
        let options = Options::default();
        assert!(
            marks(
                (Point2::new(0.0, 0.0), Point2::new(10.0, 10.0)),
                &Options {
                    size_mm: 0.0,
                    ..options
                }
            )
            .is_err()
        );
        assert!(
            marks(
                (Point2::new(0.0, 0.0), Point2::new(10.0, 10.0)),
                &Options {
                    margin_mm: -1.0,
                    ..options
                }
            )
            .is_err()
        );
        assert!(marks((Point2::new(10.0, 0.0), Point2::new(0.0, 10.0)), &options).is_err());
        assert!(
            marks(
                (Point2::new(f64::NAN, 0.0), Point2::new(1.0, 1.0)),
                &options
            )
            .is_err()
        );
    }
}
