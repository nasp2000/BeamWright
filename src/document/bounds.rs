/// Axis-aligned bounding box in millimetres. Stub for Phase 1.
#[derive(Debug, Clone, Copy)]
pub struct Bounds {
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
}

impl Bounds {
    pub fn width(&self) -> f64 {
        self.max_x - self.min_x
    }
    pub fn height(&self) -> f64 {
        self.max_y - self.min_y
    }
    pub fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.min_x && x <= self.max_x && y >= self.min_y && y <= self.max_y
    }
}

#[cfg(test)]
mod tests {
    use super::Bounds;

    #[test]
    fn width_height() {
        let b = Bounds {
            min_x: 0.0,
            min_y: 0.0,
            max_x: 10.0,
            max_y: 5.0,
        };
        assert!((b.width() - 10.0).abs() < 1e-9);
        assert!((b.height() - 5.0).abs() < 1e-9);
    }

    #[test]
    fn contains() {
        let b = Bounds {
            min_x: 0.0,
            min_y: 0.0,
            max_x: 10.0,
            max_y: 5.0,
        };
        assert!(b.contains(5.0, 2.5));
        assert!(b.contains(0.0, 0.0));
        assert!(b.contains(10.0, 5.0));
    }

    #[test]
    fn not_contains() {
        let b = Bounds {
            min_x: 0.0,
            min_y: 0.0,
            max_x: 10.0,
            max_y: 5.0,
        };
        assert!(!b.contains(11.0, 2.5));
        assert!(!b.contains(5.0, 6.0));
        assert!(!b.contains(-1.0, 2.5));
    }
}
