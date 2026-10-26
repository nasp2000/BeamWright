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

#[allow(clippy::many_single_char_names)]
pub fn segment_intersection_point(a: &Segment, b: &Segment) -> Option<(f64, f64)> {
    let d1 = cross(b, a.x1, a.y1);
    let d2 = cross(b, a.x2, a.y2);
    let d3 = cross(a, b.x1, b.y1);
    let d4 = cross(a, b.x2, b.y2);
    if ((d1 > 0.0) != (d2 > 0.0)) && ((d3 > 0.0) != (d4 > 0.0)) {
        let t = d3 / (d3 - d4);
        let x = a.x1 + t * (a.x2 - a.x1);
        let y = a.y1 + t * (a.y2 - a.y1);
        return Some((x, y));
    }
    None
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

/// Distance between two (possibly skew) line segments in 2D (min distance).
pub fn segment_distance(a: &Segment, b: &Segment) -> f64 {
    // check intersection first
    if let Some(p) = segment_intersection_point(a, b) {
        let _ = p;
        return 0.0;
    }
    // min of endpoint-to-segment distances
    let d1 = distance_point_to_segment(a.x1, a.y1, b.x1, b.y1, b.x2, b.y2);
    let d2 = distance_point_to_segment(a.x2, a.y2, b.x1, b.y1, b.x2, b.y2);
    let d3 = distance_point_to_segment(b.x1, b.y1, a.x1, a.y1, a.x2, a.y2);
    let d4 = distance_point_to_segment(b.x2, b.y2, a.x1, a.y1, a.x2, a.y2);
    d1.min(d2).min(d3).min(d4)
}

fn distance_point_to_segment(px: f64, py: f64, x1: f64, y1: f64, x2: f64, y2: f64) -> f64 {
    let dx = x2 - x1;
    let dy = y2 - y1;
    let len2 = dx * dx + dy * dy;
    if len2 == 0.0 {
        return ((px - x1).powi(2) + (py - y1).powi(2)).sqrt();
    }
    let t = ((px - x1) * dx + (py - y1) * dy) / len2;
    let t = t.clamp(0.0, 1.0);
    let cx = x1 + t * dx;
    let cy = y1 + t * dy;
    ((px - cx).powi(2) + (py - cy).powi(2)).sqrt()
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

    #[test]
    fn intersection_point() {
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
        let p = segment_intersection_point(&a, &b).unwrap();
        assert!((p.0 - 5.0).abs() < 1e-9);
        assert!((p.1 - 5.0).abs() < 1e-9);
    }

    #[test]
    fn no_intersection_point() {
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
        assert!(segment_intersection_point(&a, &b).is_none());
    }

    #[test]
    fn distance_parallel() {
        let a = Segment {
            x1: 0.0,
            y1: 0.0,
            x2: 10.0,
            y2: 0.0,
        };
        let b = Segment {
            x1: 0.0,
            y1: 5.0,
            x2: 10.0,
            y2: 5.0,
        };
        let d = segment_distance(&a, &b);
        assert!((d - 5.0).abs() < 1e-9);
    }

    #[test]
    fn distance_intersecting() {
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
        let d = segment_distance(&a, &b);
        assert!(d.abs() < 1e-9);
    }

    #[test]
    fn distance_point() {
        let a = Segment {
            x1: 0.0,
            y1: 0.0,
            x2: 0.0,
            y2: 0.0,
        };
        let b = Segment {
            x1: 3.0,
            y1: 4.0,
            x2: 3.0,
            y2: 4.0,
        };
        let d = segment_distance(&a, &b);
        assert!((d - 5.0).abs() < 1e-9);
    }
}
