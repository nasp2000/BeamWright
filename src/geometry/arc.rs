use std::f64::consts::PI;

#[derive(Debug, Clone, Copy)]
pub struct Arc {
    pub cx: f64,
    pub cy: f64,
    pub radius: f64,
    pub start_angle: f64,
    pub end_angle: f64,
}

impl Arc {
    pub fn length(&self) -> f64 {
        let sweep = (self.end_angle - self.start_angle).abs();
        self.radius * sweep
    }
    pub fn point_at(&self, t: f64) -> (f64, f64) {
        let angle = self.start_angle + (self.end_angle - self.start_angle) * t;
        (
            self.cx + self.radius * angle.cos(),
            self.cy + self.radius * angle.sin(),
        )
    }
    pub fn is_full_circle(&self) -> bool {
        (self.end_angle - self.start_angle).abs() >= 2.0 * PI - 1e-9
    }
    /// Convert arc to polyline points.
    pub fn to_polyline(&self, segments: usize) -> Vec<(f64, f64)> {
        if segments == 0 {
            return vec![];
        }
        let mut pts = Vec::with_capacity(segments + 1);
        for i in 0..=segments {
            #[allow(clippy::cast_precision_loss)]
            let t = i as f64 / segments as f64;
            pts.push(self.point_at(t));
        }
        pts
    }
    /// Approximate arc as a single cubic bezier.
    pub fn to_cubic_bezier(&self) -> crate::geometry::bezier::CubicBezier {
        let (s, e) = (self.start_angle, self.end_angle);
        let (sc, ec) = (s.cos(), e.cos());
        let (ss, es) = (s.sin(), e.sin());
        let p0 = (self.cx + self.radius * sc, self.cy + self.radius * ss);
        let p3 = (self.cx + self.radius * ec, self.cy + self.radius * es);
        let kappa = (4.0 / 3.0) * ((e - s) / 4.0).tan();
        let p1 = (
            p0.0 - kappa * self.radius * ss,
            p0.1 + kappa * self.radius * sc,
        );
        let p2 = (
            p3.0 + kappa * self.radius * es,
            p3.1 - kappa * self.radius * ec,
        );
        crate::geometry::bezier::CubicBezier {
            p0: crate::geometry::Point2::new(p0.0, p0.1),
            p1: crate::geometry::Point2::new(p1.0, p1.1),
            p2: crate::geometry::Point2::new(p2.0, p2.1),
            p3: crate::geometry::Point2::new(p3.0, p3.1),
        }
    }
}

pub fn arc_from_points(p0: (f64, f64), p1: (f64, f64), p2: (f64, f64)) -> Option<Arc> {
    // circumcircle through 3 points
    let (x1, y1) = p0;
    let (x2, y2) = p1;
    let (x3, y3) = p2;
    let d = 2.0 * (x1 * (y2 - y3) + x2 * (y3 - y1) + x3 * (y1 - y2));
    if d.abs() < 1e-12 {
        return None;
    }
    let ux = ((x1 * x1 + y1 * y1) * (y2 - y3)
        + (x2 * x2 + y2 * y2) * (y3 - y1)
        + (x3 * x3 + y3 * y3) * (y1 - y2))
        / d;
    let uy = ((x1 * x1 + y1 * y1) * (x3 - x2)
        + (x2 * x2 + y2 * y2) * (x1 - x3)
        + (x3 * x3 + y3 * y3) * (x2 - x1))
        / d;
    let radius = ((ux - x1).powi(2) + (uy - y1).powi(2)).sqrt();
    let start_angle = (y1 - uy).atan2(x1 - ux);
    let end_angle = (y2 - uy).atan2(x2 - ux);
    Some(Arc {
        cx: ux,
        cy: uy,
        radius,
        start_angle,
        end_angle,
    })
}

/// Circumference of a circle.
pub fn circle_circumference(radius: f64) -> f64 {
    2.0 * std::f64::consts::PI * radius
}

/// Area of a circle.
pub fn circle_area(radius: f64) -> f64 {
    std::f64::consts::PI * radius * radius
}

/// Approximate perimeter of an ellipse (Ramanujan).
pub fn ellipse_perimeter(a: f64, b: f64) -> f64 {
    let h = ((a - b).powi(2)) / ((a + b).powi(2));
    std::f64::consts::PI * (a + b) * (1.0 + 3.0 * h / (10.0 + (4.0 - 3.0 * h).sqrt()))
}

/// Check if a point is inside a circle.
pub fn circle_contains(cx: f64, cy: f64, r: f64, px: f64, py: f64) -> bool {
    let dx = px - cx;
    let dy = py - cy;
    dx * dx + dy * dy <= r * r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quarter_circle() {
        let a = Arc {
            cx: 0.0,
            cy: 0.0,
            radius: 5.0,
            start_angle: 0.0,
            end_angle: std::f64::consts::FRAC_PI_2,
        };
        assert!((a.length() - 5.0 * std::f64::consts::FRAC_PI_2).abs() < 1e-9);
    }

    #[test]
    fn point_at_start() {
        let a = Arc {
            cx: 0.0,
            cy: 0.0,
            radius: 5.0,
            start_angle: 0.0,
            end_angle: std::f64::consts::FRAC_PI_2,
        };
        let (x, y) = a.point_at(0.0);
        assert!((x - 5.0).abs() < 1e-9);
        assert!(y.abs() < 1e-9);
    }

    #[test]
    fn full_circle() {
        let a = Arc {
            cx: 0.0,
            cy: 0.0,
            radius: 1.0,
            start_angle: 0.0,
            end_angle: 2.0 * std::f64::consts::PI,
        };
        assert!(a.is_full_circle());
    }

    #[test]
    fn arc_from_three_points() {
        let a = arc_from_points((5.0, 0.0), (0.0, 5.0), (-5.0, 0.0));
        assert!(a.is_some());
        let a = a.unwrap();
        assert!((a.radius - 5.0).abs() < 1e-9);
    }

    #[test]
    fn collinear_returns_none() {
        assert!(arc_from_points((0.0, 0.0), (1.0, 0.0), (2.0, 0.0)).is_none());
    }
}

#[cfg(test)]
mod circle_tests {
    use super::*;

    #[test]
    fn circumference() {
        assert!((circle_circumference(1.0) - 2.0 * std::f64::consts::PI).abs() < 1e-9);
    }

    #[test]
    fn area() {
        assert!((circle_area(2.0) - std::f64::consts::PI * 4.0).abs() < 1e-9);
    }

    #[test]
    fn ellipse_perimeter_circle() {
        // circle is ellipse with a=b
        let p = ellipse_perimeter(5.0, 5.0);
        assert!((p - circle_circumference(5.0)).abs() < 1e-6);
    }

    #[test]
    fn zero_radius() {
        assert_eq!(circle_circumference(0.0), 0.0);
        assert_eq!(circle_area(0.0), 0.0);
    }
}

#[cfg(test)]
mod polyline_tests {
    use super::Arc;

    #[test]
    fn count() {
        let a = Arc {
            cx: 0.0,
            cy: 0.0,
            radius: 5.0,
            start_angle: 0.0,
            end_angle: std::f64::consts::FRAC_PI_2,
        };
        assert_eq!(a.to_polyline(4).len(), 5);
    }

    #[test]
    fn zero() {
        let a = Arc {
            cx: 0.0,
            cy: 0.0,
            radius: 5.0,
            start_angle: 0.0,
            end_angle: std::f64::consts::FRAC_PI_2,
        };
        assert_eq!(a.to_polyline(0), [] as [(f64, f64); 0]);
    }

    #[test]
    fn endpoints() {
        let a = Arc {
            cx: 0.0,
            cy: 0.0,
            radius: 5.0,
            start_angle: 0.0,
            end_angle: std::f64::consts::FRAC_PI_2,
        };
        let pts = a.to_polyline(4);
        assert!((pts[0].0 - 5.0).abs() < 1e-9);
        let last = pts.last().unwrap();
        assert!(last.0.abs() < 1e-9);
        assert!((last.1 - 5.0).abs() < 1e-9);
    }
}

#[cfg(test)]
mod bezier_conv_tests {
    use super::Arc;

    #[test]
    fn to_cubic_bezier() {
        let a = Arc {
            cx: 0.0,
            cy: 0.0,
            radius: 5.0,
            start_angle: 0.0,
            end_angle: std::f64::consts::FRAC_PI_2,
        };
        let b = a.to_cubic_bezier();
        assert!((b.start().x - 5.0).abs() < 1e-9);
        assert!((b.end().y - 5.0).abs() < 1e-9);
    }
}

#[cfg(test)]
mod circle_contains_tests {
    use super::circle_contains;

    #[test]
    fn inside() {
        assert!(circle_contains(0.0, 0.0, 5.0, 1.0, 1.0));
    }

    #[test]
    fn outside() {
        assert!(!circle_contains(0.0, 0.0, 5.0, 10.0, 10.0));
    }

    #[test]
    fn on_boundary() {
        assert!(circle_contains(0.0, 0.0, 5.0, 5.0, 0.0));
    }
}
