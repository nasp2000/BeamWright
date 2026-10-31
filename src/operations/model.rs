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
