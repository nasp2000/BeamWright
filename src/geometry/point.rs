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

/// Corner of a square (or circle) drag: both sides take the larger length.
///
/// The sign of each axis is kept, so the shape grows in the direction the
/// pointer was dragged.
pub fn square_corner(start: Point2, end: Point2) -> Point2 {
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let size = dx.abs().max(dy.abs());
    Point2::new(
        start.x + size * if dx < 0.0 { -1.0 } else { 1.0 },
        start.y + size * if dy < 0.0 { -1.0 } else { 1.0 },
    )
}

/// Corner for an exact-size drag, keeping the drag direction.
pub fn sized_corner(start: Point2, end: Point2, width: f64, height: f64) -> Point2 {
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    Point2::new(
        start.x + width.abs() * if dx < 0.0 { -1.0 } else { 1.0 },
        start.y + height.abs() * if dy < 0.0 { -1.0 } else { 1.0 },
    )
}

/// Constrain a drag to multiples of `step_degrees`, keeping its length.
pub fn constrain_angle(start: Point2, end: Point2, step_degrees: f64) -> Point2 {
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let length = dx.hypot(dy);
    if length <= 1e-9 || !step_degrees.is_finite() || step_degrees <= 0.0 {
        return end;
    }
    let step = step_degrees.to_radians();
    let angle = (dy.atan2(dx) / step).round() * step;
    Point2::new(
        start.x + length * angle.cos(),
        start.y + length * angle.sin(),
    )
}

#[cfg(test)]
mod tests {
    use super::{Point2, constrain_angle, sized_corner, square_corner};

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
    fn square_corner_uses_the_larger_side() {
        let start = Point2::new(0.0, 0.0);
        let corner = square_corner(start, Point2::new(10.0, 4.0));
        assert!((corner.x - 10.0).abs() < EPS);
        assert!((corner.y - 10.0).abs() < EPS);
        let corner = square_corner(start, Point2::new(-4.0, -12.0));
        assert!((corner.x + 12.0).abs() < EPS);
        assert!((corner.y + 12.0).abs() < EPS);
    }

    #[test]
    fn angle_constraint_keeps_length() {
        let start = Point2::new(0.0, 0.0);
        let near_horizontal = Point2::new(10.0, 1.0);
        let constrained = constrain_angle(start, near_horizontal, 45.0);
        assert!((constrained.y).abs() < 1e-9);
        assert!((constrained.distance(start) - near_horizontal.distance(start)).abs() < 1e-9);
        let diagonal = constrain_angle(start, Point2::new(10.0, 9.0), 45.0);
        assert!((diagonal.x - diagonal.y).abs() < 1e-9);
    }

    #[test]
    fn sized_corner_keeps_the_direction() {
        let start = Point2::new(5.0, 5.0);
        let corner = sized_corner(start, Point2::new(1.0, 2.0), 20.0, 10.0);
        assert!((corner.x + 15.0).abs() < EPS);
        assert!((corner.y + 5.0).abs() < EPS);
        let corner = sized_corner(start, Point2::new(30.0, 40.0), 20.0, 10.0);
        assert!((corner.x - 25.0).abs() < EPS);
        assert!((corner.y - 15.0).abs() < EPS);
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
