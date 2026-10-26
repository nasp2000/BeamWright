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
