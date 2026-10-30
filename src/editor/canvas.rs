/// Canvas view transform.
///
/// Document coordinates are millimetres with the origin at the bottom-left
/// (`y` up). Screen coordinates are egui points with `y` down. `pan_x`/`pan_y`
/// are the screen position of the document origin relative to the canvas
/// rectangle's top-left corner, and `zoom` is pixels per millimetre.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CanvasState {
    pub zoom: f64,
    pub pan_x: f64,
    pub pan_y: f64,
    pub grid_visible: bool,
    pub grid_spacing_mm: f64,
}

pub const MIN_ZOOM: f64 = 0.05;
pub const MAX_ZOOM: f64 = 64.0;

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
        self.zoom = (self.zoom * 1.25).min(MAX_ZOOM);
    }

    pub fn zoom_out(&mut self) {
        self.zoom = (self.zoom / 1.25).max(MIN_ZOOM);
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

    /// Zoom while keeping the document point under `(anchor_x, anchor_y)`
    /// fixed on screen.
    pub fn zoom_at(&mut self, anchor_x: f64, anchor_y: f64, factor: f64) {
        let (doc_x, doc_y) = self.screen_to_doc(anchor_x, anchor_y);
        self.zoom = (self.zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
        self.pan_x = anchor_x - doc_x * self.zoom;
        self.pan_y = anchor_y + doc_y * self.zoom;
    }

    /// Fit a document page into a viewport, leaving a margin in pixels.
    pub fn fit(
        &mut self,
        width_mm: f64,
        height_mm: f64,
        viewport_width: f64,
        viewport_height: f64,
        margin: f64,
    ) {
        let width_mm = width_mm.max(0.001);
        let height_mm = height_mm.max(0.001);
        let available_width = (viewport_width - 2.0 * margin).max(1.0);
        let available_height = (viewport_height - 2.0 * margin).max(1.0);
        self.zoom = (available_width / width_mm)
            .min(available_height / height_mm)
            .clamp(MIN_ZOOM, MAX_ZOOM);
        self.pan_x = (viewport_width - width_mm * self.zoom) / 2.0;
        self.pan_y = f64::midpoint(viewport_height, height_mm * self.zoom);
    }

    pub fn screen_to_doc(&self, screen_x: f64, screen_y: f64) -> (f64, f64) {
        (
            (screen_x - self.pan_x) / self.zoom,
            (self.pan_y - screen_y) / self.zoom,
        )
    }

    pub fn doc_to_screen(&self, doc_x: f64, doc_y: f64) -> (f64, f64) {
        (
            self.pan_x + doc_x * self.zoom,
            self.pan_y - doc_y * self.zoom,
        )
    }

    /// Grid spacing in millimetres that stays legible at the current zoom.
    pub fn effective_grid_spacing(&self) -> f64 {
        let base = if self.grid_spacing_mm > 0.0 {
            self.grid_spacing_mm
        } else {
            10.0
        };
        let mut spacing = base;
        while spacing * self.zoom < 8.0 {
            spacing *= 2.0;
        }
        while spacing * self.zoom > 80.0 {
            spacing /= 2.0;
        }
        spacing
    }
}

#[cfg(test)]
mod tests {
    use super::CanvasState;

    const EPS: f64 = 1e-9;

    #[test]
    fn round_trip_coordinates() {
        let canvas = CanvasState {
            zoom: 2.0,
            pan_x: 40.0,
            pan_y: 300.0,
            ..CanvasState::default()
        };
        let (sx, sy) = canvas.doc_to_screen(10.0, 20.0);
        let (dx, dy) = canvas.screen_to_doc(sx, sy);
        assert!((dx - 10.0).abs() < EPS);
        assert!((dy - 20.0).abs() < EPS);
    }

    #[test]
    fn y_axis_points_up() {
        let canvas = CanvasState {
            zoom: 1.0,
            pan_x: 0.0,
            pan_y: 100.0,
            ..CanvasState::default()
        };
        let (_, low) = canvas.doc_to_screen(0.0, 10.0);
        let (_, high) = canvas.doc_to_screen(0.0, 50.0);
        assert!(high < low);
    }

    #[test]
    fn fit_centres_page() {
        let mut canvas = CanvasState::default();
        canvas.fit(100.0, 50.0, 400.0, 300.0, 20.0);
        assert!((canvas.zoom - 3.6).abs() < EPS);
        assert!((canvas.pan_x - 20.0).abs() < EPS);
        assert!((canvas.pan_y - 240.0).abs() < EPS);
    }

    #[test]
    fn zoom_at_keeps_anchor_fixed() {
        let mut canvas = CanvasState {
            pan_x: 30.0,
            pan_y: 200.0,
            zoom: 2.0,
            ..CanvasState::default()
        };
        let (before_x, before_y) = canvas.screen_to_doc(100.0, 120.0);
        canvas.zoom_at(100.0, 120.0, 1.5);
        let (after_x, after_y) = canvas.screen_to_doc(100.0, 120.0);
        assert!((after_x - before_x).abs() < 1e-9);
        assert!((after_y - before_y).abs() < 1e-9);
    }

    #[test]
    fn zoom_is_clamped() {
        let mut canvas = CanvasState::default();
        for _ in 0..100 {
            canvas.zoom_in();
        }
        assert!(canvas.zoom <= super::MAX_ZOOM);
        for _ in 0..200 {
            canvas.zoom_out();
        }
        assert!(canvas.zoom >= super::MIN_ZOOM);
    }

    #[test]
    fn grid_spacing_adapts_to_zoom() {
        let mut canvas = CanvasState {
            zoom: 0.1,
            ..CanvasState::default()
        };
        assert!(canvas.effective_grid_spacing() * canvas.zoom >= 8.0);
        canvas.zoom = 20.0;
        assert!(canvas.effective_grid_spacing() * canvas.zoom <= 80.0);
    }
}
