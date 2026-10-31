use serde::{Deserialize, Serialize};

use super::{Point2, bezier::CubicBezier};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Segment {
    Line { from: Point2, to: Point2 },
    Cubic(CubicBezier),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Path {
    pub segments: Vec<Segment>,
    pub closed: bool,
}

impl Path {
    pub fn new() -> Self {
        Self {
            segments: Vec::new(),
            closed: false,
        }
    }
    /// Build a path from a list of points (connected by lines).
    pub fn from_points(points: &[(f64, f64)], closed: bool) -> Self {
        let mut p = Path::new();
        p.closed = closed;
        for i in 1..points.len() {
            p.push(Segment::Line {
                from: Point2::new(points[i - 1].0, points[i - 1].1),
                to: Point2::new(points[i].0, points[i].1),
            });
        }
        if closed && points.len() >= 2 {
            p.push(Segment::Line {
                from: Point2::new(points[points.len() - 1].0, points[points.len() - 1].1),
                to: Point2::new(points[0].0, points[0].1),
            });
        }
        p
    }
    /// Extract endpoints of line segments.
    pub fn to_points(&self) -> Vec<(f64, f64)> {
        let mut pts = Vec::new();
        for seg in &self.segments {
            match seg {
                Segment::Line { from, .. } => {
                    pts.push((from.x, from.y));
                }
                Segment::Cubic(c) => {
                    pts.push((c.p0.x, c.p0.y));
                }
            }
        }
        if let Some(last) = self.segments.last() {
            match last {
                Segment::Line { to, .. } => pts.push((to.x, to.y)),
                Segment::Cubic(c) => pts.push((c.p3.x, c.p3.y)),
            }
        }
        pts
    }
    pub fn push(&mut self, seg: Segment) {
        self.segments.push(seg);
    }
    pub fn append(&mut self, other: &Path) {
        self.segments.extend(other.segments.clone());
        if other.closed {
            self.closed = true;
        }
    }
    pub fn pop(&mut self) -> Option<Segment> {
        self.segments.pop()
    }
    pub fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }
    /// Reverse the direction of travel.
    ///
    /// Reversing only the order of the segments would leave every segment
    /// pointing the wrong way, so each one is flipped as well.
    pub fn reverse(&mut self) {
        self.segments.reverse();
        for segment in &mut self.segments {
            *segment = match *segment {
                Segment::Line { from, to } => Segment::Line { from: to, to: from },
                Segment::Cubic(c) => Segment::Cubic(CubicBezier {
                    p0: c.p3,
                    p1: c.p2,
                    p2: c.p1,
                    p3: c.p0,
                }),
            };
        }
    }
    pub fn close(&mut self) {
        self.closed = true;
    }
    pub fn last_point(&self) -> Option<Point2> {
        self.segments.last().map(|s| match s {
            Segment::Line { to, .. } => *to,
            Segment::Cubic(c) => c.end(),
        })
    }
    /// Bounding box of the path.
    pub fn bounding_box(&self) -> Option<(Point2, Point2)> {
        if self.segments.is_empty() {
            return None;
        }
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        for seg in &self.segments {
            match seg {
                Segment::Line { from, to } => {
                    for p in [from, to] {
                        min_x = min_x.min(p.x);
                        min_y = min_y.min(p.y);
                        max_x = max_x.max(p.x);
                        max_y = max_y.max(p.y);
                    }
                }
                Segment::Cubic(c) => {
                    let (mn, mx) = c.bounding_box();
                    min_x = min_x.min(mn.x);
                    min_y = min_y.min(mn.y);
                    max_x = max_x.max(mx.x);
                    max_y = max_y.max(mx.y);
                }
            }
        }
        Some((Point2::new(min_x, min_y), Point2::new(max_x, max_y)))
    }
    /// Flatten the path into a polyline. `tolerance` is in document units and
    /// controls how many samples each cubic segment receives.
    #[allow(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss
    )]
    pub fn flatten(&self, tolerance: f64) -> Vec<Point2> {
        let tolerance = tolerance.max(0.001);
        let mut points = Vec::new();
        for seg in &self.segments {
            match seg {
                Segment::Line { from, to } => {
                    if points.is_empty() {
                        points.push(*from);
                    }
                    points.push(*to);
                }
                Segment::Cubic(c) => {
                    if points.is_empty() {
                        points.push(c.p0);
                    }
                    let steps = ((c.approx_length(16) / tolerance).ceil() as usize).clamp(4, 512);
                    for i in 1..=steps {
                        let t = i as f64 / steps as f64;
                        points.push(c.eval(t));
                    }
                }
            }
        }
        if self.closed && points.len() > 1 {
            let first = points[0];
            let last = *points.last().unwrap_or(&first);
            if first.distance(last) > 1e-9 {
                points.push(first);
            }
        }
        points
    }
    /// Start point of the path, if any.
    pub fn start_point(&self) -> Option<Point2> {
        self.segments.first().map(|s| match s {
            Segment::Line { from, .. } => *from,
            Segment::Cubic(c) => c.p0,
        })
    }
    pub fn transform(&self, t: &crate::geometry::transform::Transform2) -> Path {
        let mut p = Path::new();
        p.closed = self.closed;
        for seg in &self.segments {
            match seg {
                Segment::Line { from, to } => {
                    p.push(Segment::Line {
                        from: t.apply(*from),
                        to: t.apply(*to),
                    });
                }
                Segment::Cubic(c) => {
                    p.push(Segment::Cubic(crate::geometry::bezier::CubicBezier {
                        p0: t.apply(c.p0),
                        p1: t.apply(c.p1),
                        p2: t.apply(c.p2),
                        p3: t.apply(c.p3),
                    }));
                }
            }
        }
        p
    }
    pub fn segment_count(&self) -> usize {
        self.segments.len()
    }
    #[allow(clippy::cast_precision_loss)]
    pub fn approximate_length(&self) -> f64 {
        self.segments
            .iter()
            .map(|s| match s {
                Segment::Line { from, to } => from.distance(*to),
                Segment::Cubic(c) => c.approx_length(20),
            })
            .sum()
    }
    /// Alias for `approximate_length`.
    pub fn length(&self) -> f64 {
        self.approximate_length()
    }
    /// Check if path is closed.
    pub fn is_closed(&self) -> bool {
        self.closed
    }
}

impl Default for Path {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty() {
        let p = Path::new();
        assert!(p.is_empty());
        assert_eq!(p.segment_count(), 0);
    }

    #[test]
    fn reverse_empty() {
        let mut p = Path::new();
        p.reverse();
        assert!(p.is_empty());
    }

    #[test]
    fn reverse_flips_segment_direction() {
        let mut p = Path::from_points(&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0)], false);
        p.reverse();
        assert_eq!(p.start_point().unwrap(), Point2::new(10.0, 10.0));
        assert_eq!(p.last_point().unwrap(), Point2::new(0.0, 0.0));
        let points = p.to_points();
        assert_eq!(points, vec![(10.0, 10.0), (10.0, 0.0), (0.0, 0.0)]);
    }

    #[test]
    fn reverse_keeps_cubic_shape() {
        let curve = CubicBezier {
            p0: Point2::new(0.0, 0.0),
            p1: Point2::new(1.0, 2.0),
            p2: Point2::new(3.0, 2.0),
            p3: Point2::new(4.0, 0.0),
        };
        let mut p = Path::new();
        p.push(Segment::Cubic(curve));
        p.reverse();
        let Segment::Cubic(reversed) = p.segments[0] else {
            panic!("expected a cubic segment");
        };
        assert_eq!(reversed.p0, curve.p3);
        assert_eq!(reversed.p1, curve.p2);
        assert_eq!(reversed.p2, curve.p1);
        assert_eq!(reversed.p3, curve.p0);
    }

    #[test]
    fn close_sets_flag() {
        let mut p = Path::new();
        assert!(!p.closed);
        p.close();
        assert!(p.closed);
    }

    #[test]
    fn last_point() {
        let mut p = Path::new();
        assert!(p.last_point().is_none());
        p.push(Segment::Line {
            from: Point2::new(0.0, 0.0),
            to: Point2::new(1.0, 1.0),
        });
        let lp = p.last_point().unwrap();
        assert!((lp.x - 1.0).abs() < 1e-9);
        assert!((lp.y - 1.0).abs() < 1e-9);
    }

    #[test]
    fn bounding_box_path() {
        let mut p = Path::new();
        p.push(Segment::Line {
            from: Point2::new(0.0, 0.0),
            to: Point2::new(10.0, 5.0),
        });
        let (mn, mx) = p.bounding_box().unwrap();
        assert!((mn.x - 0.0).abs() < 1e-9);
        assert!((mx.x - 10.0).abs() < 1e-9);
    }

    #[test]
    fn bounding_box_empty() {
        let p = Path::new();
        assert!(p.bounding_box().is_none());
    }

    #[test]
    fn length_alias() {
        let mut p = Path::new();
        p.push(Segment::Line {
            from: Point2::new(0.0, 0.0),
            to: Point2::new(3.0, 4.0),
        });
        assert!((p.length() - 5.0).abs() < 1e-9);
        assert!((p.approximate_length() - 5.0).abs() < 1e-9);
    }

    #[test]
    fn is_closed() {
        let mut p = Path::new();
        assert!(!p.is_closed());
        p.close();
        assert!(p.is_closed());
    }

    #[test]
    fn transform_path() {
        let mut p = Path::new();
        p.push(Segment::Line {
            from: Point2::new(0.0, 0.0),
            to: Point2::new(1.0, 0.0),
        });
        let t = crate::geometry::transform::Transform2::translate(5.0, 3.0);
        let r = p.transform(&t);
        match &r.segments[0] {
            Segment::Line { from, to } => {
                assert!((from.x - 5.0).abs() < 1e-9);
                assert!((to.x - 6.0).abs() < 1e-9);
            }
            Segment::Cubic(_) => panic!("wrong segment"),
        }
    }

    #[test]
    fn from_points() {
        let pts = [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0)];
        let p = Path::from_points(&pts, false);
        assert_eq!(p.segment_count(), 2);
        assert!(!p.closed);
    }

    #[test]
    fn from_points_closed() {
        let pts = [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0)];
        let p = Path::from_points(&pts, true);
        assert_eq!(p.segment_count(), 3);
        assert!(p.closed);
    }

    #[test]
    fn to_points() {
        let mut p = Path::new();
        p.push(Segment::Line {
            from: Point2::new(0.0, 0.0),
            to: Point2::new(1.0, 0.0),
        });
        let pts = p.to_points();
        assert_eq!(pts.len(), 2);
    }

    #[test]
    fn append() {
        let mut a = Path::new();
        a.push(Segment::Line {
            from: Point2::new(0.0, 0.0),
            to: Point2::new(1.0, 0.0),
        });
        let mut b = Path::new();
        b.push(Segment::Line {
            from: Point2::new(1.0, 0.0),
            to: Point2::new(2.0, 0.0),
        });
        a.append(&b);
        assert_eq!(a.segment_count(), 2);
    }

    #[test]
    fn pop() {
        let mut p = Path::new();
        p.push(Segment::Line {
            from: Point2::new(0.0, 0.0),
            to: Point2::new(1.0, 0.0),
        });
        let popped = p.pop();
        assert!(popped.is_some());
        assert!(p.is_empty());
    }

    #[test]
    fn push_line() {
        let mut p = Path::new();
        p.push(Segment::Line {
            from: Point2::new(0.0, 0.0),
            to: Point2::new(1.0, 0.0),
        });
        assert_eq!(p.segment_count(), 1);
        assert!(!p.is_empty());
    }

    #[test]
    fn closed_flag() {
        let mut p = Path::new();
        p.closed = true;
        assert!(p.closed);
    }
}
