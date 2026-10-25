#[derive(Debug, Clone, Copy)]
pub enum MoveKind {
    Travel,
    Cut,
}

#[derive(Debug, Clone, Copy)]
pub struct Move {
    pub kind: MoveKind,
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone)]
pub struct Toolpath {
    pub moves: Vec<Move>,
    pub speed: f64,
    pub power: f64,
    pub passes: u32,
}

impl Toolpath {
    pub fn new(speed: f64, power: f64, passes: u32) -> Self {
        Self {
            moves: Vec::new(),
            speed,
            power,
            passes: passes.max(1),
        }
    }
    pub fn push(&mut self, kind: MoveKind, x: f64, y: f64) {
        self.moves.push(Move { kind, x, y });
    }
    pub fn total_length(&self) -> f64 {
        let mut len = 0.0;
        let mut prev: Option<Move> = None;
        for m in &self.moves {
            if let Some(p) = prev {
                let dx = m.x - p.x;
                let dy = m.y - p.y;
                len += (dx * dx + dy * dy).sqrt();
            }
            prev = Some(*m);
        }
        len
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_length() {
        let t = Toolpath::new(1000.0, 50.0, 1);
        assert_eq!(t.total_length(), 0.0);
    }

    #[test]
    fn length() {
        let mut t = Toolpath::new(1000.0, 50.0, 1);
        t.push(MoveKind::Travel, 0.0, 0.0);
        t.push(MoveKind::Cut, 3.0, 4.0);
        assert!((t.total_length() - 5.0).abs() < 1e-9);
    }

    #[test]
    fn passes_clamped() {
        let t = Toolpath::new(1000.0, 50.0, 0);
        assert_eq!(t.passes, 1);
    }
}
