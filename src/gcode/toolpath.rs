use crate::geometry::Point2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveKind {
    /// Laser off.
    Travel,
    /// Laser on.
    Cut,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Move {
    pub kind: MoveKind,
    pub x: f64,
    pub y: f64,
}

/// One continuous operation: a set of moves repeated for every pass.
#[derive(Debug, Clone, PartialEq)]
pub struct Toolpath {
    pub name: String,
    pub moves: Vec<Move>,
    /// Feed rate in millimetres per minute.
    pub speed: f64,
    /// Laser power in the writer's `S` range (0–1000).
    pub power: f64,
    pub passes: u32,
}

impl Toolpath {
    pub fn new(name: impl Into<String>, speed: f64, power: f64, passes: u32) -> Self {
        Self {
            name: name.into(),
            moves: Vec::new(),
            speed,
            power,
            passes: passes.max(1),
        }
    }

    pub fn push(&mut self, kind: MoveKind, x: f64, y: f64) {
        self.moves.push(Move { kind, x, y });
    }

    pub fn start(&self) -> Option<Point2> {
        self.moves.first().map(|m| Point2::new(m.x, m.y))
    }

    pub fn end(&self) -> Option<Point2> {
        self.moves.last().map(|m| Point2::new(m.x, m.y))
    }

    /// Total cut length, in millimetres, for one pass.
    pub fn cut_length(&self) -> f64 {
        self.length_of(MoveKind::Cut)
    }

    /// Total travel length, in millimetres, for one pass.
    pub fn travel_length(&self) -> f64 {
        self.length_of(MoveKind::Travel)
    }

    fn length_of(&self, kind: MoveKind) -> f64 {
        let mut length = 0.0;
        let mut previous: Option<Move> = None;
        for movement in &self.moves {
            if let Some(previous) = previous {
                if movement.kind == kind {
                    let dx = movement.x - previous.x;
                    let dy = movement.y - previous.y;
                    length += (dx * dx + dy * dy).sqrt();
                }
            }
            previous = Some(*movement);
        }
        length
    }

    pub fn is_empty(&self) -> bool {
        self.moves.len() < 2
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passes_are_clamped() {
        assert_eq!(Toolpath::new("a", 1000.0, 200.0, 0).passes, 1);
    }

    #[test]
    fn length_of_polyline() {
        let mut toolpath = Toolpath::new("a", 1000.0, 200.0, 1);
        toolpath.push(MoveKind::Travel, 0.0, 0.0);
        toolpath.push(MoveKind::Cut, 3.0, 4.0);
        assert!((toolpath.cut_length() - 5.0).abs() < 1e-9);
        assert!(toolpath.end().is_some());
    }

    #[test]
    fn empty_path_is_ignored() {
        let toolpath = Toolpath::new("a", 1000.0, 200.0, 1);
        assert!(toolpath.is_empty());
    }
}
