use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Point2 {
    pub x: f64,
    pub y: f64,
}

impl Point2 {
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
    pub fn origin() -> Self {
        Self { x: 0.0, y: 0.0 }
    }
    pub fn add(self, other: Self) -> Self {
        Self {
            x: self.x + other.x,
            y: self.y + other.y,
        }
    }
    pub fn sub(self, other: Self) -> Self {
        Self {
            x: self.x - other.x,
            y: self.y - other.y,
        }
    }
    pub fn scale(self, s: f64) -> Self {
        Self {
            x: self.x * s,
            y: self.y * s,
        }
    }
    pub fn distance(self, other: Self) -> f64 {
        let d = self.sub(other);
        (d.x * d.x + d.y * d.y).sqrt()
    }
    pub fn midpoint(self, other: Self) -> Self {
        Self {
            x: f64::midpoint(self.x, other.x),
            y: f64::midpoint(self.y, other.y),
        }
    }
    pub fn rotate(self, angle_rad: f64, center: Self) -> Self {
        let c = (angle_rad.cos(), angle_rad.sin());
        let p = self.sub(center);
        Self {
            x: center.x + p.x * c.0 - p.y * c.1,
            y: center.y + p.x * c.1 + p.y * c.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Point2;

    const EPS: f64 = 1e-9;

    #[test]
    fn add_sub() {
        let a = Point2::new(1.0, 2.0);
        let b = Point2::new(3.0, 4.0);
        let s = a.add(b);
        assert!((s.x - 4.0).abs() < EPS);
        assert!((s.y - 6.0).abs() < EPS);
        let d = b.sub(a);
        assert!((d.x - 2.0).abs() < EPS);
        assert!((d.y - 2.0).abs() < EPS);
    }

    #[test]
    fn scale() {
        let p = Point2::new(2.0, 3.0).scale(2.0);
        assert!((p.x - 4.0).abs() < EPS);
        assert!((p.y - 6.0).abs() < EPS);
    }

    #[test]
    fn distance() {
        let a = Point2::new(0.0, 0.0);
        let b = Point2::new(3.0, 4.0);
        assert!((a.distance(b) - 5.0).abs() < EPS);
    }

    #[test]
    fn rotate_90() {
        let p = Point2::new(1.0, 0.0);
        let r = p.rotate(std::f64::consts::FRAC_PI_2, Point2::origin());
        assert!(r.x.abs() < EPS);
        assert!((r.y - 1.0).abs() < EPS);
    }

    #[test]
    fn midpoint() {
        let a = Point2::new(0.0, 0.0);
        let b = Point2::new(4.0, 6.0);
        let m = a.midpoint(b);
        assert!((m.x - 2.0).abs() < 1e-9);
        assert!((m.y - 3.0).abs() < 1e-9);
    }

    #[test]
    fn rotate_identity() {
        let p = Point2::new(2.0, 3.0);
        let r = p.rotate(0.0, Point2::new(5.0, 5.0));
        assert!((r.x - p.x).abs() < EPS);
        assert!((r.y - p.y).abs() < EPS);
    }
}
