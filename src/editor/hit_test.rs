#[derive(Debug, Clone, Copy)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Rect {
    pub fn contains(&self, px: f64, py: f64) -> bool {
        px >= self.x && px <= self.x + self.w && py >= self.y && py <= self.y + self.h
    }
}

pub fn point_in_circle(px: f64, py: f64, cx: f64, cy: f64, r: f64) -> bool {
    let dx = px - cx;
    let dy = py - cy;
    dx * dx + dy * dy <= r * r
}

pub fn distance_point_to_segment(px: f64, py: f64, x1: f64, y1: f64, x2: f64, y2: f64) -> f64 {
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
    fn rect_contains() {
        let r = Rect {
            x: 0.0,
            y: 0.0,
            w: 10.0,
            h: 5.0,
        };
        assert!(r.contains(5.0, 2.5));
        assert!(!r.contains(20.0, 2.5));
    }

    #[test]
    fn rect_edge() {
        let r = Rect {
            x: 0.0,
            y: 0.0,
            w: 10.0,
            h: 5.0,
        };
        assert!(r.contains(0.0, 0.0));
        assert!(r.contains(10.0, 5.0));
    }

    #[test]
    fn circle_hit() {
        assert!(point_in_circle(1.0, 1.0, 0.0, 0.0, 2.0));
        assert!(!point_in_circle(5.0, 5.0, 0.0, 0.0, 2.0));
    }

    #[test]
    fn seg_distance() {
        let d = distance_point_to_segment(0.0, 1.0, 0.0, 0.0, 10.0, 0.0);
        assert!((d - 1.0).abs() < 1e-9);
        let d2 = distance_point_to_segment(5.0, 0.0, 0.0, 0.0, 10.0, 0.0);
        assert!(d2.abs() < 1e-9);
    }
}
