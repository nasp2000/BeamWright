#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    Select,
    Line,
    Rect,
    Ellipse,
    Pan,
}

impl Tool {
    pub fn label(self) -> &'static str {
        match self {
            Tool::Select => "Select",
            Tool::Line => "Line",
            Tool::Rect => "Rectangle",
            Tool::Ellipse => "Ellipse",
            Tool::Pan => "Pan",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Tool;

    #[test]
    fn labels() {
        assert_eq!(Tool::Select.label(), "Select");
        assert_eq!(Tool::Line.label(), "Line");
        assert_eq!(Tool::Rect.label(), "Rectangle");
        assert_eq!(Tool::Ellipse.label(), "Ellipse");
        assert_eq!(Tool::Pan.label(), "Pan");
    }

    #[test]
    fn equality() {
        assert_eq!(Tool::Select, Tool::Select);
        assert_ne!(Tool::Select, Tool::Line);
    }

    #[test]
    fn copy() {
        let t = Tool::Rect;
        let t2 = t;
        assert_eq!(t, t2);
    }
}
