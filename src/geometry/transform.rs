use super::Point2;

/// 2D affine transform (matrix + translation). Stub for Phase 2.
#[derive(Debug, Clone, Copy)]
pub struct Transform2 {
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub d: f64,
    pub tx: f64,
    pub ty: f64,
}

impl Transform2 {
    pub fn identity() -> Self {
        Self {
            a: 1.0,
            b: 0.0,
            c: 0.0,
            d: 1.0,
            tx: 0.0,
            ty: 0.0,
        }
    }
    pub fn translate(tx: f64, ty: f64) -> Self {
        Self {
            a: 1.0,
            b: 0.0,
            c: 0.0,
            d: 1.0,
            tx,
            ty,
        }
    }
    pub fn scale(sx: f64, sy: f64) -> Self {
        Self {
            a: sx,
            b: 0.0,
            c: 0.0,
            d: sy,
            tx: 0.0,
            ty: 0.0,
        }
    }
    pub fn rotate(angle_rad: f64) -> Self {
        let (s, c) = (angle_rad.sin(), angle_rad.cos());
        Self {
            a: c,
            b: s,
            c: -s,
            d: c,
            tx: 0.0,
            ty: 0.0,
        }
    }
    pub fn mirror_x() -> Self {
        Self {
            a: 1.0,
            b: 0.0,
            c: 0.0,
            d: -1.0,
            tx: 0.0,
            ty: 0.0,
        }
    }
    pub fn mirror_y() -> Self {
        Self {
            a: -1.0,
            b: 0.0,
            c: 0.0,
            d: 1.0,
            tx: 0.0,
            ty: 0.0,
        }
    }
    pub fn compose(&self, other: &Self) -> Self {
        Self {
            a: self.a * other.a + self.c * other.b,
            b: self.b * other.a + self.d * other.b,
            c: self.a * other.c + self.c * other.d,
            d: self.b * other.c + self.d * other.d,
            tx: self.a * other.tx + self.c * other.ty + self.tx,
            ty: self.b * other.tx + self.d * other.ty + self.ty,
        }
    }
    pub fn invert(&self) -> Option<Self> {
        let det = self.a * self.d - self.b * self.c;
        if det.abs() < 1e-12 {
            return None;
        }
        Some(Self {
            a: self.d / det,
            b: -self.b / det,
            c: -self.c / det,
            d: self.a / det,
            tx: (self.c * self.ty - self.d * self.tx) / det,
            ty: (self.b * self.tx - self.a * self.ty) / det,
        })
    }
    pub fn apply_to_points(&self, pts: &[(f64, f64)]) -> Vec<(f64, f64)> {
        pts.iter()
            .map(|(x, y)| {
                let p = self.apply(Point2::new(*x, *y));
                (p.x, p.y)
            })
            .collect()
    }
    pub fn apply(&self, p: Point2) -> Point2 {
        Point2 {
            x: self.a * p.x + self.c * p.y + self.tx,
            y: self.b * p.x + self.d * p.y + self.ty,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Point2, Transform2};

    const EPS: f64 = 1e-9;

    #[test]
    fn identity() {
        let t = Transform2::identity();
        let p = Point2::new(3.0, 4.0);
        let r = t.apply(p);
        assert!((r.x - 3.0).abs() < EPS);
        assert!((r.y - 4.0).abs() < EPS);
    }

    #[test]
    fn translate() {
        let t = Transform2::translate(10.0, -5.0);
        let p = Point2::new(1.0, 2.0);
        let r = t.apply(p);
        assert!((r.x - 11.0).abs() < EPS);
        assert!((r.y - (-3.0)).abs() < EPS);
    }

    #[test]
    fn scale_via_matrix() {
        let t = Transform2 {
            a: 2.0,
            b: 0.0,
            c: 0.0,
            d: 2.0,
            tx: 0.0,
            ty: 0.0,
        };
        let r = t.apply(Point2::new(3.0, 4.0));
        assert!((r.x - 6.0).abs() < EPS);
        assert!((r.y - 8.0).abs() < EPS);
    }

    #[test]
    fn scale_factory() {
        let t = Transform2::scale(2.0, 3.0);
        let r = t.apply(Point2::new(1.0, 1.0));
        assert!((r.x - 2.0).abs() < EPS);
        assert!((r.y - 3.0).abs() < EPS);
    }

    #[test]
    fn rotate_factory() {
        let t = Transform2::rotate(std::f64::consts::FRAC_PI_2);
        let r = t.apply(Point2::new(1.0, 0.0));
        assert!(r.x.abs() < 1e-9);
        assert!((r.y - 1.0).abs() < 1e-9);
    }

    #[test]
    fn mirror_x() {
        let t = Transform2::mirror_x();
        let r = t.apply(Point2::new(1.0, 2.0));
        assert!((r.x - 1.0).abs() < 1e-9);
        assert!((r.y + 2.0).abs() < 1e-9);
    }

    #[test]
    fn mirror_y() {
        let t = Transform2::mirror_y();
        let r = t.apply(Point2::new(1.0, 2.0));
        assert!((r.x + 1.0).abs() < 1e-9);
        assert!((r.y - 2.0).abs() < 1e-9);
    }

    #[test]
    fn invert_identity() {
        let t = Transform2::identity();
        let inv = t.invert().unwrap();
        let r = inv.apply(Point2::new(3.0, 4.0));
        assert!((r.x - 3.0).abs() < 1e-9);
        assert!((r.y - 4.0).abs() < 1e-9);
    }

    #[test]
    fn invert_singular() {
        let t = Transform2 {
            a: 0.0,
            b: 0.0,
            c: 0.0,
            d: 0.0,
            tx: 0.0,
            ty: 0.0,
        };
        assert!(t.invert().is_none());
    }

    #[test]
    fn apply_to_points() {
        let t = Transform2::translate(1.0, 2.0);
        let r = t.apply_to_points(&[(0.0, 0.0), (1.0, 1.0)]);
        assert_eq!(r, vec![(1.0, 2.0), (2.0, 3.0)]);
    }

    #[test]
    fn compose_translate() {
        let t = Transform2::translate(1.0, 2.0).compose(&Transform2::translate(3.0, 4.0));
        let r = t.apply(Point2::origin());
        assert!((r.x - 4.0).abs() < 1e-9);
        assert!((r.y - 6.0).abs() < 1e-9);
    }
}
