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
}
