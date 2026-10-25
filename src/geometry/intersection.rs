/// Intersection test helpers (PLAN.md §6.9).
#[derive(Debug, Clone, Copy)]
pub struct Segment {
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
}

impl Segment {
    pub fn length(&self) -> f64 {
        let dx = self.x2 - self.x1;
        let dy = self.y2 - self.y1;
        (dx * dx + dy * dy).sqrt()
    }
}

pub fn segments_intersect(a: &Segment, b: &Segment) -> bool {
    // Cross-product based segment intersection test.
    let d1 = cross(b, a.x1, a.y1);
    let d2 = cross(b, a.x2, a.y2);
    let d3 = cross(a, b.x1, b.y1);
    let d4 = cross(a, b.x2, b.y2);
    if ((d1 > 0.0) != (d2 > 0.0)) && ((d3 > 0.0) != (d4 > 0.0)) {
        return true;
    }
    false
}

fn cross(s: &Segment, px: f64, py: f64) -> f64 {
    (s.x2 - s.x1) * (py - s.y1) - (s.y2 - s.y1) * (px - s.x1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn length() {
        let s = Segment {
            x1: 0.0,
            y1: 0.0,
            x2: 3.0,
            y2: 4.0,
        };
        assert!((s.length() - 5.0).abs() < 1e-9);
    }

    #[test]
    fn intersect_cross() {
        let a = Segment {
            x1: 0.0,
            y1: 0.0,
            x2: 10.0,
            y2: 10.0,
        };
        let b = Segment {
            x1: 0.0,
            y1: 10.0,
            x2: 10.0,
            y2: 0.0,
        };
        assert!(segments_intersect(&a, &b));
    }

    #[test]
    fn no_intersect() {
        let a = Segment {
            x1: 0.0,
            y1: 0.0,
            x2: 1.0,
            y2: 1.0,
        };
        let b = Segment {
            x1: 5.0,
            y1: 5.0,
            x2: 6.0,
            y2: 6.0,
        };
        assert!(!segments_intersect(&a, &b));
    }
}
