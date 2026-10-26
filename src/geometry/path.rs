use super::{Point2, bezier::CubicBezier};

#[derive(Debug, Clone, Copy)]
pub enum Segment {
    Line { from: Point2, to: Point2 },
    Cubic(CubicBezier),
}

#[derive(Debug, Clone)]
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
    pub fn reverse(&mut self) {
        self.segments.reverse();
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
