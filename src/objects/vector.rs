use serde::{Deserialize, Serialize};

use crate::geometry::Point2;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Line {
    pub start: Point2,
    pub end: Point2,
}

impl Line {
    pub fn length(&self) -> f64 {
        self.start.distance(self.end)
    }
    pub fn translate(&mut self, dx: f64, dy: f64) {
        self.start = crate::geometry::Point2::new(self.start.x + dx, self.start.y + dy);
        self.end = crate::geometry::Point2::new(self.end.x + dx, self.end.y + dy);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rect {
    pub origin: Point2,
    pub width: f64,
    pub height: f64,
}

impl Rect {
    pub fn area(&self) -> f64 {
        self.width * self.height
    }
    pub fn center(&self) -> Point2 {
        Point2::new(
            self.origin.x + self.width / 2.0,
            self.origin.y + self.height / 2.0,
        )
    }
    pub fn translate(&mut self, dx: f64, dy: f64) {
        self.origin = Point2::new(self.origin.x + dx, self.origin.y + dy);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ellipse {
    pub center: Point2,
    pub rx: f64,
    pub ry: f64,
}

impl Ellipse {
    pub fn area(&self) -> f64 {
        std::f64::consts::PI * self.rx * self.ry
    }
    pub fn translate(&mut self, dx: f64, dy: f64) {
        self.center = Point2::new(self.center.x + dx, self.center.y + dy);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VectorObject {
    Line(Line),
    Rect(Rect),
    Ellipse(Ellipse),
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPS: f64 = 1e-9;

    #[test]
    fn line_length() {
        let l = Line {
            start: Point2::new(0.0, 0.0),
            end: Point2::new(3.0, 4.0),
        };
        assert!((l.length() - 5.0).abs() < EPS);
    }

    #[test]
    fn line_zero_length() {
        let l = Line {
            start: Point2::new(1.0, 1.0),
            end: Point2::new(1.0, 1.0),
        };
        assert!(l.length().abs() < EPS);
    }

    #[test]
    fn rect_area() {
        let r = Rect {
            origin: Point2::new(0.0, 0.0),
            width: 4.0,
            height: 3.0,
        };
        assert!((r.area() - 12.0).abs() < EPS);
        assert!((r.center().x - 2.0).abs() < EPS);
        assert!((r.center().y - 1.5).abs() < EPS);
    }

    #[test]
    fn rect_degenerate() {
        let r = Rect {
            origin: Point2::new(0.0, 0.0),
            width: 0.0,
            height: 5.0,
        };
        assert!(r.area().abs() < EPS);
    }

    #[test]
    fn ellipse_area() {
        let e = Ellipse {
            center: Point2::new(0.0, 0.0),
            rx: 2.0,
            ry: 3.0,
        };
        assert!((e.area() - std::f64::consts::PI * 6.0).abs() < EPS);
    }

    #[test]
    fn enum_variant() {
        let o = VectorObject::Line(Line {
            start: Point2::new(0.0, 0.0),
            end: Point2::new(1.0, 0.0),
        });
        match o {
            VectorObject::Line(l) => assert!((l.length() - 1.0).abs() < EPS),
            _ => panic!("wrong variant"),
        }
    }
}
