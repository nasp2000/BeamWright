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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationParams {
    pub speed: f64,
    pub power: f64,
    pub passes: u32,
    pub line_interval_mm: f64,
}

impl Default for OperationParams {
    fn default() -> Self {
        Self {
            speed: 1000.0,
            power: 20.0,
            passes: 1,
            line_interval_mm: 0.1,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Operation {
    pub id: String,
    pub name: String,
    pub op_type: OperationType,
    pub params: OperationParams,
    pub enabled: bool,
}

impl Operation {
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
        }
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
