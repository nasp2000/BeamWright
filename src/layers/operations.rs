use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerOperation {
    pub layer_id: String,
    pub operation_id: String,
    pub enabled: bool,
}

impl LayerOperation {
    pub fn validate(&self) -> std::result::Result<(), String> {
        if self.layer_id.is_empty() {
            return Err("layer_id must not be empty".to_string());
        }
        if self.operation_id.is_empty() {
            return Err("operation_id must not be empty".to_string());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::LayerOperation;

    #[test]
    fn valid() {
        let lo = LayerOperation {
            layer_id: "l1".to_string(),
            operation_id: "op1".to_string(),
            enabled: true,
        };
        assert!(lo.validate().is_ok());
    }

    #[test]
    fn reject_empty_layer() {
        let lo = LayerOperation {
            layer_id: String::new(),
            operation_id: "op1".to_string(),
            enabled: true,
        };
        assert!(lo.validate().is_err());
    }

    #[test]
    fn reject_empty_operation() {
        let lo = LayerOperation {
            layer_id: "l1".to_string(),
            operation_id: String::new(),
            enabled: true,
        };
        assert!(lo.validate().is_err());
    }
}
