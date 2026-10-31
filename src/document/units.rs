use serde::{Deserialize, Serialize};

/// Supported measurement units. All internal geometry is stored in millimetres.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Unit {
    Millimeters,
    Centimeters,
    Inches,
    Pixels,
}

impl Unit {
    /// Convert a value expressed in this unit into millimetres.
    ///
    /// `dpi` is only used for `Pixels`; pass `96.0` (or the document's
    /// configured resolution) when converting pixel quantities.
    #[allow(clippy::wrong_self_convention)]
    pub fn to_mm(self, value: f64, dpi: f64) -> f64 {
        match self {
            Unit::Millimeters => value,
            Unit::Centimeters => value * 10.0,
            Unit::Inches => value * 25.4,
            Unit::Pixels => {
                if dpi <= 0.0 {
                    value
                } else {
                    value * 25.4 / dpi
                }
            }
        }
    }

    /// Convert a value in millimetres into this unit.
    #[allow(clippy::wrong_self_convention)]
    pub fn from_mm(self, mm: f64, dpi: f64) -> f64 {
        match self {
            Unit::Millimeters => mm,
            Unit::Centimeters => mm / 10.0,
            Unit::Inches => mm / 25.4,
            Unit::Pixels => {
                if dpi <= 0.0 {
                    mm
                } else {
                    mm * dpi / 25.4
                }
            }
        }
    }

    /// Parse the short label used by preferences.
    pub fn from_label(label: &str) -> Option<Self> {
        match label {
            "mm" => Some(Unit::Millimeters),
            "cm" => Some(Unit::Centimeters),
            "in" => Some(Unit::Inches),
            "px" => Some(Unit::Pixels),
            _ => None,
        }
    }

    /// Short label for display.
    pub fn label(self) -> &'static str {
        match self {
            Unit::Millimeters => "mm",
            Unit::Centimeters => "cm",
            Unit::Inches => "in",
            Unit::Pixels => "px",
        }
    }
}

#[cfg(test)]
mod label_tests {
    use super::Unit;

    #[test]
    fn labels_round_trip() {
        for unit in [
            Unit::Millimeters,
            Unit::Centimeters,
            Unit::Inches,
            Unit::Pixels,
        ] {
            assert_eq!(Unit::from_label(unit.label()), Some(unit));
        }
        assert_eq!(Unit::from_label("feet"), None);
    }
}

#[cfg(test)]
mod tests {
    use super::Unit;

    const EPS: f64 = 1e-9;

    #[test]
    fn mm_identity() {
        assert!((Unit::Millimeters.to_mm(5.0, 96.0) - 5.0).abs() < EPS);
        assert!((Unit::Millimeters.from_mm(5.0, 96.0) - 5.0).abs() < EPS);
    }

    #[test]
    fn cm_conversion() {
        assert!((Unit::Centimeters.to_mm(1.0, 96.0) - 10.0).abs() < EPS);
        assert!((Unit::Centimeters.from_mm(10.0, 96.0) - 1.0).abs() < EPS);
    }

    #[test]
    fn in_conversion() {
        assert!((Unit::Inches.to_mm(1.0, 96.0) - 25.4).abs() < EPS);
        assert!((Unit::Inches.from_mm(25.4, 96.0) - 1.0).abs() < EPS);
    }

    #[test]
    fn px_conversion() {
        // 96 px/in => 1 px = 25.4/96 mm
        let mm = Unit::Pixels.to_mm(96.0, 96.0);
        assert!((mm - 25.4).abs() < EPS);
        assert!((Unit::Pixels.from_mm(25.4, 96.0) - 96.0).abs() < EPS);
    }

    #[test]
    fn round_trip() {
        for u in [Unit::Millimeters, Unit::Centimeters, Unit::Inches] {
            let v = 12.34;
            let mm = u.to_mm(v, 96.0);
            let back = u.from_mm(mm, 96.0);
            assert!((back - v).abs() < 1e-6, "{u:?}");
        }
    }

    #[test]
    fn labels() {
        assert_eq!(Unit::Millimeters.label(), "mm");
        assert_eq!(Unit::Centimeters.label(), "cm");
        assert_eq!(Unit::Inches.label(), "in");
        assert_eq!(Unit::Pixels.label(), "px");
    }
}
