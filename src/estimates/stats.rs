/// Statistics about generated G-code.
#[derive(Debug, Default, Clone)]
pub struct GCodeStats {
    pub line_count: usize,
    pub cut_moves: usize,
    pub travel_moves: usize,
    pub total_length_mm: f64,
    pub estimated_bytes: usize,
}

pub fn analyze_gcode(src: &str) -> GCodeStats {
    let mut s = GCodeStats::default();
    for line in src.lines() {
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        s.line_count += 1;
        s.estimated_bytes += t.len() + 1;
        if t.starts_with('G') {
            // crude: lines with X/Y after G1 are cut moves
            if t.contains('X') || t.contains('Y') {
                if t.contains('F') {
                    s.cut_moves += 1;
                } else {
                    s.travel_moves += 1;
                }
            }
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty() {
        let s = analyze_gcode("");
        assert_eq!(s.line_count, 0);
        assert_eq!(s.estimated_bytes, 0);
    }

    #[test]
    fn counts_lines() {
        let src = "G21\nG90\nG1 X10 Y0 F1000\nG0 X0 Y0\n";
        let s = analyze_gcode(src);
        assert_eq!(s.line_count, 4);
        assert!(s.estimated_bytes > 0);
    }

    #[test]
    fn cut_vs_travel() {
        let src = "G1 X10 Y0 F1000\nG0 X0 Y0\n";
        let s = analyze_gcode(src);
        assert_eq!(s.cut_moves, 1);
        assert_eq!(s.travel_moves, 1);
    }
}
