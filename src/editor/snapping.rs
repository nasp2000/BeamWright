#[derive(Debug, Clone, Copy)]
#[allow(clippy::struct_excessive_bools)]
pub struct SnapSettings {
    pub grid: bool,
    pub nodes: bool,
    pub endpoints: bool,
    pub centers: bool,
    pub tolerance_px: f64,
}

impl Default for SnapSettings {
    fn default() -> Self {
        Self {
            grid: true,
            nodes: true,
            endpoints: true,
            centers: true,
            tolerance_px: 8.0,
        }
    }
}

impl SnapSettings {
    pub fn any_enabled(&self) -> bool {
        self.grid || self.nodes || self.endpoints || self.centers
    }
}

pub fn snap_to_grid(value: f64, spacing: f64) -> f64 {
    if spacing <= 0.0 {
        return value;
    }
    (value / spacing).round() * spacing
}

pub fn snap_point(
    px: f64,
    py: f64,
    candidates: &[(f64, f64)],
    tolerance: f64,
) -> Option<(f64, f64)> {
    let mut best: Option<(f64, f64)> = None;
    let mut best_dist = tolerance;
    for (cx, cy) in candidates {
        let dx = cx - px;
        let dy = cy - py;
        let d = (dx * dx + dy * dy).sqrt();
        if d < best_dist {
            best_dist = d;
            best = Some((*cx, *cy));
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::SnapSettings;

    #[test]
    fn default_enabled() {
        let s = SnapSettings::default();
        assert!(s.any_enabled());
        assert!(s.grid);
    }

    #[test]
    fn all_disabled() {
        let s = SnapSettings {
            grid: false,
            nodes: false,
            endpoints: false,
            centers: false,
            tolerance_px: 8.0,
        };
        assert!(!s.any_enabled());
    }

    #[test]
    fn tolerance() {
        let s = SnapSettings::default();
        assert!(s.tolerance_px > 0.0);
    }
}

#[cfg(test)]
mod snap_tests {
    use super::snap_to_grid;

    #[test]
    fn snap_basic() {
        assert!((snap_to_grid(2.4, 1.0) - 2.0).abs() < 1e-9);
        assert!((snap_to_grid(2.6, 1.0) - 3.0).abs() < 1e-9);
    }

    #[test]
    fn snap_zero_spacing() {
        assert!((snap_to_grid(2.4, 0.0) - 2.4).abs() < 1e-9);
    }

    #[test]
    fn snap_negative_spacing() {
        assert!((snap_to_grid(2.4, -1.0) - 2.4).abs() < 1e-9);
    }
}

#[cfg(test)]
mod snap_point_tests {
    use super::snap_point;

    #[test]
    fn snap_to_nearest() {
        let cands = [(0.0, 0.0), (10.0, 0.0)];
        let r = snap_point(0.5, 0.0, &cands, 2.0).unwrap();
        assert_eq!(r, (0.0, 0.0));
    }

    #[test]
    fn no_snap_outside_tolerance() {
        let cands = [(0.0, 0.0)];
        assert!(snap_point(5.0, 0.0, &cands, 2.0).is_none());
    }

    #[test]
    fn empty_candidates() {
        assert!(snap_point(0.0, 0.0, &[], 2.0).is_none());
    }
}
