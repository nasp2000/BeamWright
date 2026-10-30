use std::fmt::Write as _;

use crate::gcode::dialect::GCodeOptions;
use crate::gcode::toolpath::{MoveKind, Toolpath};

/// Render toolpaths as a GRBL 1.1 job.
///
/// The file is self-contained: metric units, absolute positioning, XY plane and
/// a laser mode command. It never contains controller configuration.
pub struct GCodeWriter {
    options: GCodeOptions,
}

impl GCodeWriter {
    pub fn new(options: GCodeOptions) -> Self {
        Self { options }
    }

    pub fn render(&self, toolpaths: &[Toolpath]) -> String {
        let newline = if self.options.line_ending_crlf {
            "\r\n"
        } else {
            "\n"
        };
        let mut out = String::new();
        if self.options.include_comments {
            let _ = write!(out, "(BeamWright job){newline}");
        }
        let _ = write!(out, "G21{newline}");
        let _ = write!(out, "G90{newline}");
        let _ = write!(out, "G17{newline}");
        let _ = write!(out, "{}{newline}", self.options.laser_mode.command());
        let precision = self.options.decimals as usize;
        for (index, toolpath) in toolpaths.iter().enumerate() {
            if self.options.include_comments {
                let _ = write!(
                    out,
                    "(Operation {}: {} | F{:.0} S{:.0} x{}){newline}",
                    index + 1,
                    toolpath.name.replace(['(', ')'], " "),
                    toolpath.speed,
                    toolpath.power,
                    toolpath.passes
                );
            }
            let _ = write!(out, "S0{newline}");
            for pass in 0..toolpath.passes {
                if pass > 0 {
                    let _ = write!(out, "S0{newline}");
                }
                let mut laser_on = false;
                for movement in &toolpath.moves {
                    match movement.kind {
                        MoveKind::Travel => {
                            if laser_on {
                                let _ = write!(out, "S0{newline}");
                                laser_on = false;
                            }
                            if self.options.include_travel || pass == 0 {
                                let _ = write!(
                                    out,
                                    "G0 X{} Y{}{newline}",
                                    fmt_num(movement.x, precision),
                                    fmt_num(movement.y, precision)
                                );
                            }
                        }
                        MoveKind::Cut => {
                            if laser_on {
                                let _ = write!(
                                    out,
                                    "G1 X{} Y{}{newline}",
                                    fmt_num(movement.x, precision),
                                    fmt_num(movement.y, precision)
                                );
                            } else {
                                let _ = write!(
                                    out,
                                    "G1 X{} Y{} F{:.0} S{:.0}{newline}",
                                    fmt_num(movement.x, precision),
                                    fmt_num(movement.y, precision),
                                    toolpath.speed,
                                    toolpath.power
                                );
                                laser_on = true;
                            }
                        }
                    }
                }
            }
        }
        let _ = write!(out, "S0{newline}");
        let _ = write!(out, "M5{newline}");
        out
    }
}

fn fmt_num(value: f64, decimals: usize) -> String {
    let mut text = format!("{value:.decimals$}");
    if text.contains('.') {
        while text.ends_with('0') {
            text.pop();
        }
        if text.ends_with('.') {
            text.pop();
        }
    }
    if text == "-0" {
        text = "0".to_string();
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gcode::dialect::LaserMode;

    fn sample() -> Toolpath {
        let mut toolpath = Toolpath::new("Outline", 1000.0, 500.0, 1);
        toolpath.push(MoveKind::Travel, 0.0, 0.0);
        toolpath.push(MoveKind::Cut, 10.0, 0.0);
        toolpath.push(MoveKind::Cut, 10.0, 5.0);
        toolpath
    }

    #[test]
    fn header_and_footer() {
        let writer = GCodeWriter::new(GCodeOptions::default());
        let text = writer.render(&[]);
        for expected in ["G21", "G90", "G17", "M4", "M5"] {
            assert!(text.contains(expected), "missing {expected}");
        }
    }

    #[test]
    fn cut_sets_feed_and_power() {
        let writer = GCodeWriter::new(GCodeOptions::default());
        let text = writer.render(&[sample()]);
        assert!(text.contains("G1 X10 Y0 F1000 S500"));
        assert_eq!(text.matches("G1 X").count(), 2);
    }

    #[test]
    fn travel_can_be_omitted_after_first_pass() {
        let options = GCodeOptions {
            include_travel: false,
            include_comments: false,
            ..GCodeOptions::default()
        };
        let writer = GCodeWriter::new(options);
        let text = writer.render(&[sample()]);
        assert_eq!(text.matches("G0 ").count(), 1);
    }

    #[test]
    fn passes_repeat_cuts() {
        let mut toolpath = sample();
        toolpath.passes = 2;
        let writer = GCodeWriter::new(GCodeOptions::default());
        let text = writer.render(&[toolpath]);
        assert_eq!(text.matches("G1 X").count(), 4);
    }

    #[test]
    fn laser_mode_is_respected() {
        let options = GCodeOptions {
            laser_mode: LaserMode::ConstantPower,
            ..GCodeOptions::default()
        };
        let text = GCodeWriter::new(options).render(&[]);
        assert!(text.contains("M3"));
        assert!(!text.contains("M4"));
    }

    #[test]
    fn crlf_when_requested() {
        let options = GCodeOptions {
            line_ending_crlf: true,
            ..GCodeOptions::default()
        };
        let text = GCodeWriter::new(options).render(&[]);
        assert!(text.contains("\r\n"));
        assert!(!text.replace("\r\n", "").contains('\n'));
    }
}
