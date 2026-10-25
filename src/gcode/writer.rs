use std::fmt::Write as _;

use crate::gcode::dialect::GCodeOptions;
use crate::gcode::toolpath::{MoveKind, Toolpath};

pub struct GCodeWriter {
    opts: GCodeOptions,
}

impl GCodeWriter {
    pub fn new(opts: GCodeOptions) -> Self {
        Self { opts }
    }

    pub fn render(&self, toolpaths: &[Toolpath]) -> String {
        let nl = if self.opts.line_ending_crlf {
            "\r\n"
        } else {
            "\n"
        };
        let mut out = String::new();
        if self.opts.include_comments {
            let _ = writeln!(out, "(BeamWright G-code job)");
        }
        let _ = write!(out, "G21{nl}");
        let _ = write!(out, "G90{nl}");
        let _ = write!(out, "G17{nl}");
        let _ = write!(out, "{}{nl}", self.opts.laser_mode.command());
        for (i, tp) in toolpaths.iter().enumerate() {
            if self.opts.include_comments {
                let _ = writeln!(out, "(Operation {})", i + 1);
            }
            for _pass in 0..tp.passes {
                let mut first_cut = true;
                for m in &tp.moves {
                    match m.kind {
                        MoveKind::Travel => {
                            if self.opts.include_travel {
                                let _ = write!(
                                    out,
                                    "G0 X{:.prec$} Y{:.prec$}{}",
                                    m.x,
                                    m.y,
                                    nl,
                                    prec = self.opts.decimals as usize
                                );
                            }
                        }
                        MoveKind::Cut => {
                            if first_cut {
                                let _ = write!(
                                    out,
                                    "G1 X{:.prec$} Y{:.prec$} F{:.0} S{:.0}{}",
                                    m.x,
                                    m.y,
                                    tp.speed,
                                    tp.power,
                                    nl,
                                    prec = self.opts.decimals as usize
                                );
                                first_cut = false;
                            } else {
                                let _ = write!(
                                    out,
                                    "G1 X{:.prec$} Y{:.prec$}{}",
                                    m.x,
                                    m.y,
                                    nl,
                                    prec = self.opts.decimals as usize
                                );
                            }
                        }
                    }
                }
            }
        }
        let _ = write!(out, "M5{nl}");
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gcode::LaserMode;
    use crate::gcode::dialect::GCodeOptions;

    #[test]
    fn header_present() {
        let w = GCodeWriter::new(GCodeOptions::default());
        let s = w.render(&[]);
        assert!(s.contains("G21"));
        assert!(s.contains("G90"));
        assert!(s.contains("G17"));
        assert!(s.contains("M4"));
        assert!(s.contains("M5"));
    }

    #[test]
    fn m3_mode() {
        let o = GCodeOptions {
            laser_mode: LaserMode::ConstantPower,
            ..GCodeOptions::default()
        };
        let w = GCodeWriter::new(o);
        let s = w.render(&[]);
        assert!(s.contains("M3"));
        assert!(!s.contains("M4"));
    }

    #[test]
    fn crlf() {
        let o = GCodeOptions {
            line_ending_crlf: true,
            include_comments: false,
            ..GCodeOptions::default()
        };
        let w = GCodeWriter::new(o);
        let s = w.render(&[]);
        assert!(s.contains("\r\n"));
    }

    #[test]
    fn lf_default() {
        let o = GCodeOptions {
            include_comments: false,
            ..GCodeOptions::default()
        };
        let w = GCodeWriter::new(o);
        let s = w.render(&[]);
        assert!(!s.contains("\r\n"));
    }

    #[test]
    fn travel_and_cut() {
        let mut tp = Toolpath::new(1000.0, 50.0, 1);
        tp.push(MoveKind::Travel, 0.0, 0.0);
        tp.push(MoveKind::Cut, 3.0, 4.0);
        let w = GCodeWriter::new(GCodeOptions::default());
        let s = w.render(&[tp]);
        assert!(s.contains("G0 X"));
        assert!(s.contains("G1 X"));
        assert!(s.contains("F1000"));
        assert!(s.contains("S50"));
    }

    #[test]
    fn passes_repeat() {
        let mut tp = Toolpath::new(1000.0, 50.0, 2);
        tp.push(MoveKind::Cut, 1.0, 1.0);
        let o = GCodeOptions {
            include_comments: false,
            include_travel: false,
            ..GCodeOptions::default()
        };
        let w = GCodeWriter::new(o);
        let s = w.render(&[tp]);
        // G1 appears twice (one per pass)
        assert_eq!(s.matches("G1 X").count(), 2);
    }
}
