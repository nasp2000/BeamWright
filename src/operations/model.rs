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
}

fn default_true() -> bool {
    true
}

impl OperationParams {
    /// Parameters that differ from `target`, ready to show before applying.
    ///
    /// Floating-point fields are compared with a small tolerance so a value that
    /// is unchanged does not appear in the list.
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
