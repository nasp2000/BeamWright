#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeType {
    Corner,
    Smooth,
    Symmetric,
}

impl NodeType {
    pub fn label(self) -> &'static str {
        match self {
            NodeType::Corner => "Corner",
            NodeType::Smooth => "Smooth",
            NodeType::Symmetric => "Symmetric",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Node {
    pub id: u32,
    pub pos_x: f64,
    pub pos_y: f64,
    pub kind: NodeType,
}

impl Node {
    pub fn move_to(&mut self, x: f64, y: f64) {
        self.pos_x = x;
        self.pos_y = y;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels() {
        assert_eq!(NodeType::Corner.label(), "Corner");
        assert_eq!(NodeType::Smooth.label(), "Smooth");
        assert_eq!(NodeType::Symmetric.label(), "Symmetric");
    }

    #[test]
    fn move_to() {
        let mut n = Node {
            id: 1,
            pos_x: 0.0,
            pos_y: 0.0,
            kind: NodeType::Corner,
        };
        n.move_to(5.0, 7.0);
        assert!((n.pos_x - 5.0).abs() < 1e-9);
        assert!((n.pos_y - 7.0).abs() < 1e-9);
    }

    #[test]
    fn equality() {
        assert_eq!(NodeType::Smooth, NodeType::Smooth);
        assert_ne!(NodeType::Corner, NodeType::Smooth);
    }
}
