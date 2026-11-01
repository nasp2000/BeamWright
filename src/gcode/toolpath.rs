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
    /// Power for this move alone, when it differs from the toolpath power.
    ///
    /// Grayscale engraving uses it to vary `S` along the raster; `None` means
    /// the toolpath power applies.
    pub power: Option<f64>,
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
    /// Operation the toolpath belongs to, when one was assigned.
    pub operation_id: Option<String>,
    /// Name of that operation, for readable comments.
    pub operation_name: Option<String>,
    /// Turn air assist on around this toolpath (M8/M9).
    pub air_assist: bool,
    /// Use M3 (constant power) instead of M4 for this toolpath.
    pub constant_power: bool,
}

impl Toolpath {
    pub fn new(name: impl Into<String>, speed: f64, power: f64, passes: u32) -> Self {
        Self {
            name: name.into(),
            moves: Vec::new(),
            speed,
            power,
            passes: passes.max(1),
            operation_id: None,
            operation_name: None,
            air_assist: false,
            constant_power: false,
        }
    }

    /// Attach the operation that produced this toolpath.
    pub fn with_operation(mut self, operation_id: Option<&str>) -> Self {
        self.operation_id = operation_id.map(str::to_string);
        self
    }

    /// Turn air assist on or off for this toolpath.
    pub fn with_air_assist(mut self, air_assist: bool) -> Self {
        self.air_assist = air_assist;
        self
    }

    /// Select constant or dynamic laser power for this toolpath.
    pub fn with_constant_power(mut self, constant_power: bool) -> Self {
        self.constant_power = constant_power;
        self
    }

    /// Attach the readable name of that operation.
    pub fn with_operation_name(mut self, name: Option<&str>) -> Self {
        self.operation_name = name.map(str::to_string);
        self
    }

    pub fn push(&mut self, kind: MoveKind, x: f64, y: f64) {
        self.moves.push(Move {
            kind,
            x,
            y,
            power: None,
        });
    }

    /// Push a move with its own laser power.
    pub fn push_powered(&mut self, kind: MoveKind, x: f64, y: f64, power: f64) {
        self.moves.push(Move {
            kind,
            x,
            y,
            power: Some(power),
        });
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
    fn moves_carry_an_optional_power() {
        let mut toolpath = Toolpath::new("a", 1000.0, 200.0, 1);
        toolpath.push(MoveKind::Cut, 1.0, 1.0);
        toolpath.push_powered(MoveKind::Cut, 2.0, 1.0, 120.0);
        assert_eq!(toolpath.moves[0].power, None);
        assert_eq!(toolpath.moves[1].power, Some(120.0));
    }

    #[test]
    fn empty_path_is_ignored() {
        let toolpath = Toolpath::new("a", 1000.0, 200.0, 1);
        assert!(toolpath.is_empty());
    }
}
