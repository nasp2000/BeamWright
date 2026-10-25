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
}
