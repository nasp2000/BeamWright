use std::fmt::Write as _;

use crate::gcode::dialect::{GCodeOptions, LaserMode};
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

    #[allow(clippy::too_many_lines)]
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
        let initial_command = self
            .options
            .dialect
            .laser_command(self.options.laser_mode == LaserMode::ConstantPower);
        let _ = write!(out, "{initial_command}{newline}");
        let precision = self.options.decimals as usize;
        let mut air_on = false;
        let mut laser_command = initial_command;
        for (index, toolpath) in toolpaths.iter().enumerate() {
            if toolpath.air_assist && !air_on {
                let _ = write!(out, "M8{newline}");
                air_on = true;
            } else if !toolpath.air_assist && air_on {
                let _ = write!(out, "M9{newline}");
                air_on = false;
            }
            if self.options.dialect.supports_dynamic_power() {
                let command = self.options.dialect.laser_command(toolpath.constant_power);
                if command != laser_command {
                    let _ = write!(out, "{command}{newline}");
                    laser_command = command;
                }
            }
            if self.options.include_comments {
                let operation = toolpath
                    .operation_name
                    .as_deref()
                    .unwrap_or("Default parameters");
                let _ = write!(
                    out,
                    "(Operation {}: {} - {} | F{:.0} S{:.0} x{}){newline}",
                    index + 1,
                    operation.replace(['(', ')'], " "),
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
                let mut current_power = 0.0;
                for movement in &toolpath.moves {
                    match movement.kind {
                        MoveKind::Travel => {
                            if laser_on {
                                let _ = write!(out, "S0{newline}");
                                laser_on = false;
                                current_power = 0.0;
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
                            let power = movement.power.unwrap_or(toolpath.power);
                            if laser_on {
                                if (power - current_power).abs() > 0.5 {
                                    let _ = write!(out, "S{power:.0}{newline}");
                                    current_power = power;
                                }
                                let _ = write!(
                                    out,
                                    "G1 X{} Y{}{newline}",
                                    fmt_num(movement.x, precision),
                                    fmt_num(movement.y, precision)
                                );
                            } else {
                                let _ = write!(
                                    out,
                                    "G1 X{} Y{} F{:.0} S{power:.0}{newline}",
                                    fmt_num(movement.x, precision),
                                    fmt_num(movement.y, precision),
                                    toolpath.speed
                                );
                                laser_on = true;
                                current_power = power;
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

    #[test]
    fn air_assist_and_constant_power_follow_the_toolpaths() {
        let mut assisted = Toolpath::new("Cut", 1000.0, 500.0, 1)
            .with_air_assist(true)
            .with_constant_power(true);
        assisted.push(MoveKind::Travel, 0.0, 0.0);
        assisted.push(MoveKind::Cut, 10.0, 0.0);
        let mut plain = Toolpath::new("Engrave", 800.0, 300.0, 1);
        plain.push(MoveKind::Travel, 0.0, 5.0);
        plain.push(MoveKind::Cut, 10.0, 5.0);
        let text = GCodeWriter::new(GCodeOptions::default()).render(&[assisted, plain]);
        assert!(text.contains("M8"), "air assist was not enabled\n{text}");
        assert!(
            text.contains("M3"),
            "constant power was not selected\n{text}"
        );
        assert!(text.contains("M9"), "air assist was not disabled\n{text}");
        assert!(
            text.matches("M4").count() >= 2,
            "dynamic power was not restored\n{text}"
        );
    }

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
    fn a_move_can_change_the_power_without_turning_the_laser_off() {
        let mut toolpath = Toolpath::new("Raster", 3000.0, 800.0, 1);
        toolpath.push(MoveKind::Travel, 0.0, 0.0);
        toolpath.push_powered(MoveKind::Cut, 1.0, 0.0, 800.0);
        toolpath.push_powered(MoveKind::Cut, 2.0, 0.0, 400.0);
        toolpath.push_powered(MoveKind::Cut, 3.0, 0.0, 400.0);
        toolpath.push_powered(MoveKind::Cut, 4.0, 0.0, 0.0);
        let text = GCodeWriter::new(GCodeOptions::default()).render(&[toolpath]);
        assert!(text.contains("G1 X1 Y0 F3000 S800"), "{text}");
        assert_eq!(text.matches("S400").count(), 1, "{text}");
        assert_eq!(text.matches("S800").count(), 2, "{text}");
        // The pass start, the change to zero power and the footer.
        assert_eq!(text.matches("S0").count(), 3, "{text}");
        // The unchanged power is not repeated for the third move.
        assert_eq!(text.matches("G1 X").count(), 4, "{text}");
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
