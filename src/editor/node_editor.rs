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
    pub fn as_point(&self) -> (f64, f64) {
        (self.pos_x, self.pos_y)
    }
}

#[derive(Debug, Clone, Default)]
pub struct NodePath {
    pub nodes: Vec<Node>,
}

impl NodePath {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn add(&mut self, node: Node) {
        self.nodes.push(node);
    }
    pub fn remove(&mut self, id: u32) {
        self.nodes.retain(|n| n.id != id);
    }
    pub fn find(&self, id: u32) -> Option<&Node> {
        self.nodes.iter().find(|n| n.id == id)
    }
    pub fn find_mut(&mut self, id: u32) -> Option<&mut Node> {
        self.nodes.iter_mut().find(|n| n.id == id)
    }
    pub fn len(&self) -> usize {
        self.nodes.len()
    }
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
    pub fn clear(&mut self) {
        self.nodes.clear();
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

#[cfg(test)]
mod node_path_tests {
    use super::{Node, NodePath, NodeType};

    #[test]
    fn add_find() {
        let mut p = NodePath::new();
        p.add(Node {
            id: 1,
            pos_x: 0.0,
            pos_y: 0.0,
            kind: NodeType::Corner,
        });
        assert_eq!(p.len(), 1);
        assert!(p.find(1).is_some());
    }

    #[test]
    fn remove() {
        let mut p = NodePath::new();
        p.add(Node {
            id: 1,
            pos_x: 0.0,
            pos_y: 0.0,
            kind: NodeType::Corner,
        });
        p.remove(1);
        assert!(p.is_empty());
        assert!(p.find(1).is_none());
    }

    #[test]
    fn move_node() {
        let mut p = NodePath::new();
        p.add(Node {
            id: 1,
            pos_x: 0.0,
            pos_y: 0.0,
            kind: NodeType::Corner,
        });
        if let Some(n) = p.find_mut(1) {
            n.move_to(3.0, 4.0);
        }
        assert_eq!(p.find(1).unwrap().as_point(), (3.0, 4.0));
    }
}
