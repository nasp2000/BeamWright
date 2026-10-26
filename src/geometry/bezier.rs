use super::Point2;

#[derive(Debug, Clone, Copy)]
pub struct CubicBezier {
    pub p0: Point2,
    pub p1: Point2,
    pub p2: Point2,
    pub p3: Point2,
}

impl CubicBezier {
    pub fn eval(&self, t: f64) -> Point2 {
        let u = 1.0 - t;
        let u2 = u * u;
        let u3 = u2 * u;
        let t2 = t * t;
        let t3 = t2 * t;
        Point2 {
            x: u3 * self.p0.x
                + 3.0 * u2 * t * self.p1.x
                + 3.0 * u * t2 * self.p2.x
                + t3 * self.p3.x,
            y: u3 * self.p0.y
                + 3.0 * u2 * t * self.p1.y
                + 3.0 * u * t2 * self.p2.y
                + t3 * self.p3.y,
        }
    }
    pub fn start(&self) -> Point2 {
        self.p0
    }
    pub fn end(&self) -> Point2 {
        self.p3
    }
    /// Bounding box of the bezier (approximated by control points).
    pub fn bounding_box(&self) -> (Point2, Point2) {
        let xs = [self.p0.x, self.p1.x, self.p2.x, self.p3.x];
        let ys = [self.p0.y, self.p1.y, self.p2.y, self.p3.y];
        let min_x = xs.iter().copied().fold(f64::INFINITY, f64::min);
        let max_x = xs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let min_y = ys.iter().copied().fold(f64::INFINITY, f64::min);
        let max_y = ys.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        (Point2::new(min_x, min_y), Point2::new(max_x, max_y))
    }
    /// Approximate arc length by sampling.
    #[allow(clippy::cast_precision_loss)]
    pub fn approx_length(&self, steps: usize) -> f64 {
        if steps == 0 {
            return self.start().distance(self.end());
        }
        let mut total = 0.0;
        let mut prev = self.eval(0.0);
        for i in 1..=steps {
            let t = i as f64 / steps as f64;
            let curr = self.eval(t);
            total += prev.distance(curr);
            prev = curr;
        }
        total
    }
    /// Split at parameter t using de Casteljau.
    #[allow(clippy::similar_names)]
    pub fn split(&self, t: f64) -> (CubicBezier, CubicBezier) {
        let p01 = lerp(self.p0, self.p1, t);
        let p12 = lerp(self.p1, self.p2, t);
        let p23 = lerp(self.p2, self.p3, t);
        let p012 = lerp(p01, p12, t);
        let p123 = lerp(p12, p23, t);
        let mid = lerp(p012, p123, t);
        (
            CubicBezier {
                p0: self.p0,
                p1: p01,
                p2: p012,
                p3: mid,
            },
            CubicBezier {
                p0: mid,
                p1: p123,
                p2: p23,
                p3: self.p3,
            },
        )
    }
}

fn lerp(a: Point2, b: Point2, t: f64) -> Point2 {
    Point2::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPS: f64 = 1e-9;

    #[test]
    fn eval_endpoints() {
        let b = CubicBezier {
            p0: Point2::new(0.0, 0.0),
            p1: Point2::new(1.0, 2.0),
            p2: Point2::new(3.0, 4.0),
            p3: Point2::new(5.0, 0.0),
        };
        let s = b.eval(0.0);
        assert!((s.x).abs() < EPS && (s.y).abs() < EPS);
        let e = b.eval(1.0);
        assert!((e.x - 5.0).abs() < EPS && (e.y).abs() < EPS);
    }

    #[test]
    fn eval_mid() {
        let b = CubicBezier {
            p0: Point2::new(0.0, 0.0),
            p1: Point2::new(0.0, 0.0),
            p2: Point2::new(10.0, 0.0),
            p3: Point2::new(10.0, 0.0),
        };
        let m = b.eval(0.5);
        assert!((m.x - 5.0).abs() < 1e-6);
    }

    #[test]
    fn start_end() {
        let b = CubicBezier {
            p0: Point2::new(1.0, 2.0),
            p1: Point2::new(3.0, 4.0),
            p2: Point2::new(5.0, 6.0),
            p3: Point2::new(7.0, 8.0),
        };
        assert_eq!(b.start(), b.p0);
        assert_eq!(b.end(), b.p3);
    }

    #[test]
    fn split_midpoint() {
        let b = CubicBezier {
            p0: Point2::new(0.0, 0.0),
            p1: Point2::new(1.0, 2.0),
            p2: Point2::new(3.0, 4.0),
            p3: Point2::new(5.0, 0.0),
        };
        let (left, right) = b.split(0.5);
        // mid point should match eval(0.5)
        let mid = b.eval(0.5);
        assert!((left.end().x - mid.x).abs() < 1e-9);
        assert!((left.end().y - mid.y).abs() < 1e-9);
        assert!((right.start().x - mid.x).abs() < 1e-9);
    }

    #[test]
    fn approx_length_positive() {
        let b = CubicBezier {
            p0: Point2::new(0.0, 0.0),
            p1: Point2::new(1.0, 2.0),
            p2: Point2::new(3.0, 4.0),
            p3: Point2::new(5.0, 0.0),
        };
        let l = b.approx_length(20);
        assert!(l > 0.0);
        assert!(l >= b.start().distance(b.end()));
    }

    #[test]
    fn bounding_box() {
        let b = CubicBezier {
            p0: Point2::new(0.0, 0.0),
            p1: Point2::new(1.0, 2.0),
            p2: Point2::new(3.0, 4.0),
            p3: Point2::new(5.0, 0.0),
        };
        let (min, max) = b.bounding_box();
        assert!((min.x - 0.0).abs() < 1e-9);
        assert!((max.x - 5.0).abs() < 1e-9);
        assert!((min.y - 0.0).abs() < 1e-9);
        assert!((max.y - 4.0).abs() < 1e-9);
    }

    #[test]
    fn split_endpoints() {
        let b = CubicBezier {
            p0: Point2::new(0.0, 0.0),
            p1: Point2::new(1.0, 2.0),
            p2: Point2::new(3.0, 4.0),
            p3: Point2::new(5.0, 0.0),
        };
        let (left, right) = b.split(0.0);
        assert_eq!(left.start(), b.p0);
        assert_eq!(right.end(), b.p3);
    }
}
