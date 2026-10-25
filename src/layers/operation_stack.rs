use crate::layers::LayerOperation;

#[derive(Debug, Default)]
pub struct LayerOperationStack {
    pub items: Vec<LayerOperation>,
}

impl LayerOperationStack {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn add(&mut self, item: LayerOperation) {
        if item.validate().is_ok() {
            self.items.push(item);
        }
    }
    pub fn for_layer(&self, layer_id: &str) -> Vec<&LayerOperation> {
        self.items
            .iter()
            .filter(|i| i.layer_id == layer_id)
            .collect()
    }
    pub fn for_operation(&self, op_id: &str) -> Vec<&LayerOperation> {
        self.items
            .iter()
            .filter(|i| i.operation_id == op_id)
            .collect()
    }
    pub fn set_enabled(&mut self, layer_id: &str, op_id: &str, enabled: bool) {
        for i in &mut self.items {
            if i.layer_id == layer_id && i.operation_id == op_id {
                i.enabled = enabled;
            }
        }
    }
    pub fn remove_layer(&mut self, layer_id: &str) {
        self.items.retain(|i| i.layer_id != layer_id);
    }
    pub fn remove_operation(&mut self, op_id: &str) {
        self.items.retain(|i| i.operation_id != op_id);
    }
    pub fn len(&self) -> usize {
        self.items.len()
    }
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layers::LayerOperation;

    #[test]
    fn add_and_query() {
        let mut s = LayerOperationStack::new();
        s.add(LayerOperation {
            layer_id: "l1".to_string(),
            operation_id: "op1".to_string(),
            enabled: true,
        });
        assert_eq!(s.len(), 1);
        assert_eq!(s.for_layer("l1").len(), 1);
        assert_eq!(s.for_operation("op1").len(), 1);
    }

    #[test]
    fn invalid_not_added() {
        let mut s = LayerOperationStack::new();
        s.add(LayerOperation {
            layer_id: String::new(),
            operation_id: "op1".to_string(),
            enabled: true,
        });
        assert!(s.is_empty());
    }

    #[test]
    fn set_enabled() {
        let mut s = LayerOperationStack::new();
        s.add(LayerOperation {
            layer_id: "l1".to_string(),
            operation_id: "op1".to_string(),
            enabled: true,
        });
        s.set_enabled("l1", "op1", false);
        assert!(!s.items[0].enabled);
    }

    #[test]
    fn remove() {
        let mut s = LayerOperationStack::new();
        s.add(LayerOperation {
            layer_id: "l1".to_string(),
            operation_id: "op1".to_string(),
            enabled: true,
        });
        s.remove_layer("l1");
        assert!(s.is_empty());
    }
}
