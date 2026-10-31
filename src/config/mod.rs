pub mod paths;
pub mod preferences;
#[derive(Debug, Clone)]
pub struct AppConfig {
    pub default_unit: &'static str,
    pub theme: &'static str,
    pub backup_enabled: bool,
    pub default_dir: Option<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            default_unit: "mm",
            theme: "dark",
            backup_enabled: true,
            default_dir: None,
        }
    }
}

impl AppConfig {
    pub fn validate(&self) -> std::result::Result<(), String> {
        if self.default_unit.is_empty() {
            return Err("default unit must not be empty".to_string());
        }
        if self.theme.is_empty() {
            return Err("theme must not be empty".to_string());
        }
        Ok(())
    }
}

// Snapping targets are independent toggles, so the booleans are not a sign of
// a missing state machine.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DrawingPrefs {
    pub grid_spacing_mm: f64,
    pub snap_tolerance_px: f64,
    pub show_rulers: bool,
    #[serde(default = "default_snap_step")]
    pub snap_step_mm: f64,
    #[serde(default = "default_true")]
    pub snap_enabled: bool,
    /// Snap drawing to path nodes.
    #[serde(default)]
    pub snap_nodes: bool,
    /// Snap drawing to the ends of open paths.
    #[serde(default)]
    pub snap_endpoints: bool,
    /// Snap drawing to object centres.
    #[serde(default)]
    pub snap_centers: bool,
    /// Snap drawing to segment midpoints.
    #[serde(default)]
    pub snap_midpoints: bool,
    /// Snap drawing to crossings between object outlines.
    #[serde(default)]
    pub snap_intersections: bool,
    /// Snap drawing to object bounding-box corners and edges.
    #[serde(default)]
    pub snap_limits: bool,
    /// Draw the document guides.
    #[serde(default = "default_true")]
    pub guides_visible: bool,
    /// Snap drawing to visible guides.
    #[serde(default)]
    pub snap_guides: bool,
    /// Colour of selection outlines and handles.
    #[serde(default = "default_selection_color")]
    pub selection_color: String,
    /// Node handle shape: `square` or `circle`.
    #[serde(default = "default_handle_style")]
    pub handle_style: String,
    /// Zoom factor for the zoom commands.
    #[serde(default = "default_zoom_step")]
    pub zoom_step: f64,
    /// Clicking one object of a group selects the whole group.
    #[serde(default = "default_true")]
    pub select_whole_group: bool,
    /// Zoom factor applied by the mouse wheel.
    #[serde(default = "default_scroll_zoom_step")]
    pub scroll_zoom_step: f64,
    /// Largest raster image accepted on import, in pixels.
    #[serde(default = "default_max_image_pixels")]
    pub max_image_pixels: u64,
}

impl Default for DrawingPrefs {
    fn default() -> Self {
        Self {
            grid_spacing_mm: 10.0,
            snap_tolerance_px: 8.0,
            show_rulers: true,
            snap_step_mm: 1.0,
            snap_enabled: false,
            snap_nodes: false,
            snap_endpoints: false,
            snap_centers: false,
            snap_midpoints: false,
            snap_intersections: false,
            snap_limits: false,
            guides_visible: true,
            snap_guides: false,
            selection_color: default_selection_color(),
            handle_style: default_handle_style(),
            zoom_step: default_zoom_step(),
            select_whole_group: true,
            scroll_zoom_step: default_scroll_zoom_step(),
            max_image_pixels: default_max_image_pixels(),
        }
    }
}

impl DrawingPrefs {
    pub fn validate(&self) -> std::result::Result<(), String> {
        if self.grid_spacing_mm <= 0.0 {
            return Err("grid spacing must be positive".to_string());
        }
        if self.snap_tolerance_px < 0.0 {
            return Err("snap tolerance must be non-negative".to_string());
        }
        if !self.snap_step_mm.is_finite() || self.snap_step_mm <= 0.0 {
            return Err("snap step must be positive".to_string());
        }
        if crate::color::normalize_hex_color(&self.selection_color).is_none() {
            return Err("selection colour must be a hex colour".to_string());
        }
        if self.handle_style != "square" && self.handle_style != "circle" {
            return Err("handle style must be square or circle".to_string());
        }
        if !self.zoom_step.is_finite() || !(1.01..=3.0).contains(&self.zoom_step) {
            return Err("zoom step must be between 1.01 and 3".to_string());
        }
        if !self.scroll_zoom_step.is_finite() || !(1.01..=2.0).contains(&self.scroll_zoom_step) {
            return Err("wheel zoom step must be between 1.01 and 2".to_string());
        }
        if !(1_000_000..=500_000_000).contains(&self.max_image_pixels) {
            return Err("image pixel limit must be between 1 and 500 million".to_string());
        }
        Ok(())
    }
}

fn default_snap_step() -> f64 {
    1.0
}

fn default_true() -> bool {
    true
}

fn default_selection_color() -> String {
    "#008CFF".to_string()
}

fn default_handle_style() -> String {
    "square".to_string()
}

fn default_zoom_step() -> f64 {
    1.25
}

fn default_scroll_zoom_step() -> f64 {
    1.15
}

fn default_max_image_pixels() -> u64 {
    50_000_000
}

#[cfg(test)]
mod tests {
    use super::AppConfig;

    #[test]
    fn default_valid() {
        assert!(AppConfig::default().validate().is_ok());
    }

    #[test]
    fn reject_empty_unit() {
        let c = AppConfig {
            default_unit: "",
            ..AppConfig::default()
        };
        assert!(c.validate().is_err());
    }

    #[test]
    fn reject_empty_theme() {
        let c = AppConfig {
            theme: "",
            ..AppConfig::default()
        };
        assert!(c.validate().is_err());
    }
}

#[cfg(test)]
mod drawing_tests {
    use super::DrawingPrefs;

    #[test]
    fn default_valid() {
        assert!(DrawingPrefs::default().validate().is_ok());
    }

    #[test]
    fn reject_bad_spacing() {
        let p = DrawingPrefs {
            grid_spacing_mm: 0.0,
            ..DrawingPrefs::default()
        };
        assert!(p.validate().is_err());
        let p = DrawingPrefs {
            grid_spacing_mm: -1.0,
            ..DrawingPrefs::default()
        };
        assert!(p.validate().is_err());
    }

    #[test]
    fn reject_negative_tolerance() {
        let p = DrawingPrefs {
            snap_tolerance_px: -1.0,
            ..DrawingPrefs::default()
        };
        assert!(p.validate().is_err());
    }

    #[test]
    fn reject_bad_snap_step() {
        let p = DrawingPrefs {
            snap_step_mm: 0.0,
            ..DrawingPrefs::default()
        };
        assert!(p.validate().is_err());
    }
}
