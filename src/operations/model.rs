use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OperationType {
    Line,
    LineEngraving,
    Fill,
    FillWithOutline,
    FillWithOffset,
    OutlineCutting,
    ImageProcessing,
    Custom,
}

impl OperationType {
    pub fn label(self) -> &'static str {
        match self {
            OperationType::Line => "Line",
            OperationType::LineEngraving => "Line engraving",
            OperationType::Fill => "Fill",
            OperationType::FillWithOutline => "Fill with outline",
            OperationType::FillWithOffset => "Fill with offset",
            OperationType::OutlineCutting => "Outline cutting",
            OperationType::ImageProcessing => "Image processing",
            OperationType::Custom => "Custom",
        }
    }

    pub fn all() -> [OperationType; 8] {
        [
            OperationType::Line,
            OperationType::LineEngraving,
            OperationType::Fill,
            OperationType::FillWithOutline,
            OperationType::FillWithOffset,
            OperationType::OutlineCutting,
            OperationType::ImageProcessing,
            OperationType::Custom,
        ]
    }
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OperationParams {
    pub speed: f64,
    pub power: f64,
    pub passes: u32,
    pub line_interval_mm: f64,
    /// Rotation of a linear fill, in degrees.
    #[serde(default)]
    pub fill_angle_degrees: f64,
    /// Reverse every other fill line.
    #[serde(default = "default_true")]
    pub fill_alternate: bool,
    /// Inset kept between the outline and the fill, in millimetres.
    #[serde(default)]
    pub fill_margin_mm: f64,
    /// Perpendicular shift of the fill pattern, in millimetres.
    #[serde(default)]
    pub fill_offset_mm: f64,
    /// Colour used for this cut in the table and the toolpath preview.
    #[serde(default = "default_cut_color")]
    pub color: String,
    /// Turn air assist on for this operation (M8/M9 in the job).
    #[serde(default)]
    pub air_assist: bool,
    /// Show this operation's toolpaths in the canvas preview.
    #[serde(default = "default_true")]
    pub show_in_preview: bool,
    /// Use M3 (constant power) instead of M4 for this operation.
    #[serde(default)]
    pub constant_power: bool,
    /// Add a second fill pass rotated by 90°.
    #[serde(default)]
    pub cross_hatch: bool,
    /// Extend every fill line by this distance at both ends.
    #[serde(default)]
    pub overscan_mm: f64,
    /// Half of this is removed from contour cuts to compensate the kerf.
    #[serde(default)]
    pub kerf_mm: f64,
    /// Number of holding tabs left uncut on contour cuts.
    #[serde(default)]
    pub tabs: u32,
    /// Width of each holding tab, in millimetres.
    #[serde(default = "default_tab_width")]
    pub tab_width_mm: f64,
    /// Distance between tabs, in millimetres.
    ///
    /// Zero uses the tab count; a positive value derives the count from the
    /// perimeter of each contour.
    #[serde(default)]
    pub tab_spacing_mm: f64,
    /// Extra cut distance before the contour, in millimetres.
    #[serde(default)]
    pub lead_in_mm: f64,
    /// Extra cut distance after the contour, in millimetres.
    #[serde(default)]
    pub lead_out_mm: f64,
    /// Enter and leave along a quarter-circle arc instead of a straight line.
    #[serde(default)]
    pub curved_leads: bool,
}

fn default_tab_width() -> f64 {
    2.0
}

fn default_cut_color() -> String {
    "#000000".to_string()
}

fn default_true() -> bool {
    true
}

fn yes_no(value: bool) -> String {
    if value { "Yes" } else { "No" }.to_string()
}

impl OperationParams {
    /// Parameters that differ from `target`, ready to show before applying.
    ///
    /// Floating-point fields are compared with a small tolerance so a value that
    /// is unchanged does not appear in the list.
    #[allow(clippy::too_many_lines)]
    pub fn changes_to(&self, target: &Self) -> Vec<ParamChange> {
        fn changed(current: &str, next: &str) -> bool {
            current != next
        }
        let mut changes = Vec::new();
        let mut push = |label: &'static str, from: String, to: String| {
            if changed(&from, &to) {
                changes.push(ParamChange { label, from, to });
            }
        };
        push(
            "Speed",
            format!("{:.0} mm/min", self.speed),
            format!("{:.0} mm/min", target.speed),
        );
        push(
            "Power",
            format!("{:.0} %", self.power),
            format!("{:.0} %", target.power),
        );
        push("Passes", self.passes.to_string(), target.passes.to_string());
        push(
            "Line interval",
            format!("{:.2} mm", self.line_interval_mm),
            format!("{:.2} mm", target.line_interval_mm),
        );
        push(
            "Fill angle",
            format!("{:.0}°", self.fill_angle_degrees),
            format!("{:.0}°", target.fill_angle_degrees),
        );
        push(
            "Alternating fill",
            self.fill_alternate.to_string(),
            target.fill_alternate.to_string(),
        );
        push(
            "Fill margin",
            format!("{:.2} mm", self.fill_margin_mm),
            format!("{:.2} mm", target.fill_margin_mm),
        );
        push(
            "Fill offset",
            format!("{:.2} mm", self.fill_offset_mm),
            format!("{:.2} mm", target.fill_offset_mm),
        );
        push(
            "Overscan",
            format!("{:.2} mm", self.overscan_mm),
            format!("{:.2} mm", target.overscan_mm),
        );
        push(
            "Cross-hatch",
            yes_no(self.cross_hatch),
            yes_no(target.cross_hatch),
        );
        push(
            "Air assist",
            yes_no(self.air_assist),
            yes_no(target.air_assist),
        );
        push(
            "Constant power",
            yes_no(self.constant_power),
            yes_no(target.constant_power),
        );
        push(
            "Show in preview",
            yes_no(self.show_in_preview),
            yes_no(target.show_in_preview),
        );
        push("Colour", self.color.clone(), target.color.clone());
        push("Tabs", self.tabs.to_string(), target.tabs.to_string());
        push(
            "Lead-in",
            format!("{:.2} mm", self.lead_in_mm),
            format!("{:.2} mm", target.lead_in_mm),
        );
        push(
            "Lead-out",
            format!("{:.2} mm", self.lead_out_mm),
            format!("{:.2} mm", target.lead_out_mm),
        );
        push(
            "Curved leads",
            if self.curved_leads { "yes" } else { "no" }.to_string(),
            if target.curved_leads { "yes" } else { "no" }.to_string(),
        );
        push(
            "Tab width",
            format!("{:.2} mm", self.tab_width_mm),
            format!("{:.2} mm", target.tab_width_mm),
        );
        push(
            "Tab spacing",
            format!("{:.2} mm", self.tab_spacing_mm),
            format!("{:.2} mm", target.tab_spacing_mm),
        );
        push(
            "Kerf",
            format!("{:.2} mm", self.kerf_mm),
            format!("{:.2} mm", target.kerf_mm),
        );
        changes
    }
}

impl Default for OperationParams {
    fn default() -> Self {
        Self {
            speed: 1000.0,
            power: 20.0,
            passes: 1,
            line_interval_mm: 0.1,
            fill_angle_degrees: 0.0,
            fill_alternate: true,
            fill_margin_mm: 0.0,
            fill_offset_mm: 0.0,
            color: default_cut_color(),
            air_assist: false,
            show_in_preview: true,
            constant_power: false,
            cross_hatch: false,
            overscan_mm: 0.0,
            kerf_mm: 0.0,
            tabs: 0,
            tab_width_mm: default_tab_width(),
            tab_spacing_mm: 0.0,
            lead_in_mm: 0.0,
            lead_out_mm: 0.0,
            curved_leads: false,
        }
    }
}

/// One parameter that a profile would change, with both formatted values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParamChange {
    pub label: &'static str,
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Operation {
    pub id: String,
    pub name: String,
    pub op_type: OperationType,
    pub params: OperationParams,
    pub enabled: bool,
    /// Operations that must run before this one.
    #[serde(default)]
    pub depends_on: Vec<String>,
}

impl Operation {
    pub fn new(name: impl Into<String>, op_type: OperationType) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.into(),
            op_type,
            params: OperationParams::default(),
            enabled: true,
            depends_on: Vec::new(),
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.name.trim().is_empty() {
            return Err("operation name must not be empty".to_string());
        }
        if self.id.is_empty() {
            return Err("operation id must not be empty".to_string());
        }
        if self.params.speed <= 0.0 {
            return Err("speed must be positive".to_string());
        }
        if self.params.power < 0.0 {
            return Err("power must be non-negative".to_string());
        }
        if self.params.passes == 0 {
            return Err("passes must be >= 1".to_string());
        }
        if self.params.line_interval_mm <= 0.0 {
            return Err("line interval must be positive".to_string());
        }
        if !self.params.fill_angle_degrees.is_finite() {
            return Err("fill angle must be finite".to_string());
        }
        if !self.params.fill_margin_mm.is_finite() || self.params.fill_margin_mm < 0.0 {
            return Err("fill margin must be zero or positive".to_string());
        }
        if !self.params.fill_offset_mm.is_finite() {
            return Err("fill offset must be finite".to_string());
        }
        if crate::color::normalize_hex_color(&self.params.color).is_none() {
            return Err(format!("invalid cut colour: {}", self.params.color));
        }
        if !self.params.overscan_mm.is_finite() || !(0.0..=50.0).contains(&self.params.overscan_mm)
        {
            return Err("overscan must be between 0 and 50 mm".to_string());
        }
        for (index, dependency) in self.depends_on.iter().enumerate() {
            if dependency.trim().is_empty() {
                return Err("operation dependencies must not be empty".to_string());
            }
            if dependency == &self.id {
                return Err("an operation cannot depend on itself".to_string());
            }
            if self.depends_on[..index].contains(dependency) {
                return Err(format!("duplicate operation dependency: {dependency}"));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Operation {
        Operation {
            id: "op-1".to_string(),
            name: "Cut".to_string(),
            op_type: OperationType::OutlineCutting,
            params: OperationParams::default(),
            enabled: true,
            depends_on: Vec::new(),
        }
    }

    #[test]
    fn operation_type_labels_are_unique() {
        let labels: Vec<&str> = OperationType::all().iter().map(|t| t.label()).collect();
        let mut sorted = labels.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), labels.len());
        assert_eq!(OperationType::Fill.label(), "Fill");
    }

    #[test]
    fn new_operation_is_valid_and_unique() {
        let first = Operation::new("Cut", OperationType::OutlineCutting);
        let second = Operation::new("Cut", OperationType::OutlineCutting);
        assert!(first.validate().is_ok());
        assert_ne!(first.id, second.id);
        assert!(first.enabled);
    }

    #[test]
    fn new_operation_uses_default_params() {
        let operation = Operation::new("Engrave", OperationType::LineEngraving);
        assert_eq!(operation.params, OperationParams::default());
    }

    #[test]
    fn valid() {
        assert!(sample().validate().is_ok());
    }

    #[test]
    fn reject_empty_name() {
        let mut o = sample();
        o.name = String::new();
        assert!(o.validate().is_err());
    }

    #[test]
    fn reject_bad_speed() {
        let mut o = sample();
        o.params.speed = 0.0;
        assert!(o.validate().is_err());
        o.params.speed = -1.0;
        assert!(o.validate().is_err());
    }

    #[test]
    fn reject_bad_power() {
        let mut o = sample();
        o.params.power = -1.0;
        assert!(o.validate().is_err());
    }

    #[test]
    fn parameter_changes_are_listed() {
        let current = OperationParams::default();
        let target = OperationParams {
            speed: 800.0,
            power: 55.0,
            passes: 3,
            line_interval_mm: 0.05,
            fill_angle_degrees: 45.0,
            fill_alternate: false,
            fill_margin_mm: 0.5,
            fill_offset_mm: 0.2,
            ..OperationParams::default()
        };
        let changes = current.changes_to(&target);
        let labels: Vec<&str> = changes.iter().map(|change| change.label).collect();
        assert!(labels.contains(&"Speed"));
        assert!(labels.contains(&"Power"));
        assert!(labels.contains(&"Passes"));
        assert!(labels.contains(&"Fill angle"));
        assert_eq!(changes.len(), 8);
        assert_eq!(changes[0].from, "1000 mm/min");
        assert_eq!(changes[0].to, "800 mm/min");
        assert_eq!(current.changes_to(&current).len(), 0);
    }

    #[test]
    fn reject_zero_passes() {
        let mut o = sample();
        o.params.passes = 0;
        assert!(o.validate().is_err());
    }

    #[test]
    fn reject_bad_interval() {
        let mut o = sample();
        o.params.line_interval_mm = 0.0;
        assert!(o.validate().is_err());
    }
}
