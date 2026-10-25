#[derive(Debug, Clone, Copy)]
pub struct CanvasState {
    pub zoom: f64,
    pub pan_x: f64,
    pub pan_y: f64,
    pub grid_visible: bool,
    pub grid_spacing_mm: f64,
}

impl Default for CanvasState {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            pan_x: 0.0,
            pan_y: 0.0,
            grid_visible: true,
            grid_spacing_mm: 10.0,
        }
    }
}

impl CanvasState {
    pub fn zoom_in(&mut self) {
        self.zoom = (self.zoom * 1.25).min(64.0);
    }
    pub fn zoom_out(&mut self) {
        self.zoom = (self.zoom / 1.25).max(0.05);
    }
    pub fn reset_zoom(&mut self) {
        self.zoom = 1.0;
    }
    pub fn pan(&mut self, dx: f64, dy: f64) {
        self.pan_x += dx;
        self.pan_y += dy;
    }
    pub fn reset_pan(&mut self) {
        self.pan_x = 0.0;
        self.pan_y = 0.0;
    }
    /// Convert a screen point to document (mm) coordinates.
    pub fn screen_to_doc(&self, sx: f64, sy: f64) -> (f64, f64) {
        ((sx - self.pan_x) / self.zoom, (sy - self.pan_y) / self.zoom)
    }
    /// Convert document (mm) coordinates to screen.
    pub fn doc_to_screen(&self, dx: f64, dy: f64) -> (f64, f64) {
        (dx * self.zoom + self.pan_x, dy * self.zoom + self.pan_y)
    }
}

#[cfg(test)]
mod tests {
    use super::CanvasState;

    const EPS: f64 = 1e-9;

    #[test]
    fn defaults() {
        let c = CanvasState::default();
        assert!((c.zoom - 1.0).abs() < EPS);
        assert!(c.grid_visible);
    }

    #[test]
    fn zoom_in_out() {
        let mut c = CanvasState::default();
        c.zoom_in();
        assert!(c.zoom > 1.0);
        c.zoom_out();
        assert!((c.zoom - 1.0).abs() < 1e-6);
    }

    #[test]
    fn zoom_clamps() {
        let mut c = CanvasState::default();
        for _ in 0..100 {
            c.zoom_in();
        }
        assert!(c.zoom <= 64.0);
        for _ in 0..1000 {
            c.zoom_out();
        }
        assert!(c.zoom >= 0.05);
    }

    #[test]
    fn pan() {
        let mut c = CanvasState::default();
        c.pan(10.0, -5.0);
        assert!((c.pan_x - 10.0).abs() < EPS);
        assert!((c.pan_y + 5.0).abs() < EPS);
        c.reset_pan();
        assert!((c.pan_x).abs() < EPS);
    }

    #[test]
    fn coord_round_trip() {
        let mut c = CanvasState {
            zoom: 2.0,
            ..CanvasState::default()
        };
        c.pan(5.0, 7.0);
        let (sx, sy) = c.doc_to_screen(3.0, 4.0);
        let (dx, dy) = c.screen_to_doc(sx, sy);
        assert!((dx - 3.0).abs() < EPS);
        assert!((dy - 4.0).abs() < EPS);
    }
}
