//! HPGL/PLT import for pen-plotter drawings.
//!
//! Coordinates are given in plotter units (40 per millimetre) with the Y axis
//! up, like the document. Pen-down runs become open paths; pen-up moves only
//! reposition the head.

use crate::document::Document;
use crate::document::model::{Object, Shape};
use crate::errors::{BeamError, Result};
use crate::geometry::{Path, Point2, path::Segment};

/// Plotter units per millimetre.
const UNITS_PER_MM: f64 = 40.0;

/// Result of an HPGL import.
#[derive(Debug, Clone, PartialEq)]
pub struct HpglImport {
    pub document: Document,
    pub warnings: Vec<String>,
}

/// Import an HPGL file.
pub fn import_hpgl(src: &str) -> Result<Document> {
    Ok(import_hpgl_report(src)?.document)
}

pub fn import_hpgl_report(src: &str) -> Result<HpglImport> {
    let mut document = Document::default();
    let mut warnings = Vec::new();
    let text = src.trim();
    if text.is_empty() {
        return Ok(HpglImport { document, warnings });
    }
    if !text.bytes().any(|byte| byte.is_ascii_alphabetic()) {
        return Err(BeamError::InvalidInput(
            "not an HPGL file: no plotter commands".to_string(),
        ));
    }

    let mut absolute = true;
    let mut position = Point2::origin();
    let mut paths: Vec<Path> = Vec::new();
    let mut current: Option<Path> = None;
    let mut unsupported: Vec<String> = Vec::new();
    let mut odd_coordinates = false;

    for command in commands(text) {
        match command.name.as_str() {
            "IN" => {
                position = Point2::origin();
                if let Some(path) = current.take() {
                    paths.push(path);
                }
            }
            "PA" => absolute = true,
            "PR" => absolute = false,
            "PU" | "PD" => {
                let pen_down = command.name == "PD";
                if !pen_down {
                    if let Some(path) = current.take() {
                        if !path.segments.is_empty() {
                            paths.push(path);
                        }
                    }
                }
                if command.params.len() % 2 != 0 {
                    odd_coordinates = true;
                }
                for pair in command.params.chunks_exact(2) {
                    let target = if absolute {
                        Point2::new(pair[0] / UNITS_PER_MM, pair[1] / UNITS_PER_MM)
                    } else {
                        Point2::new(
                            position.x + pair[0] / UNITS_PER_MM,
                            position.y + pair[1] / UNITS_PER_MM,
                        )
                    };
                    if pen_down {
                        let path = current.get_or_insert_with(Path::new);
                        if target.distance(position) > 1e-9 {
                            path.push(Segment::Line {
                                from: position,
                                to: target,
                            });
                        }
                    } else if let Some(path) = current.take() {
                        if !path.segments.is_empty() {
                            paths.push(path);
                        }
                    }
                    position = target;
                }
            }
            // Speed, pen selection, line type and page setup do not change the
            // geometry prepared here.
            "SP" | "LT" | "PW" | "FS" | "VS" | "CS" | "CT" | "PC" | "NP" | "WU" | "PM" => {}
            "ZZ" => break,
            other => {
                if !unsupported.iter().any(|known| known == other) {
                    unsupported.push(other.to_string());
                }
            }
        }
    }
    if let Some(path) = current.take() {
        if !path.segments.is_empty() {
            paths.push(path);
        }
    }

    if odd_coordinates {
        warnings.push("a coordinate list had an odd number of values".to_string());
    }
    for name in unsupported {
        warnings.push(format!("unsupported HPGL command ignored: {name}"));
    }
    let layer_id = document.layers[0].id.clone();
    for path in paths {
        let mut object = Object::new(layer_id.clone(), Shape::Path(path));
        object.style.stroke = Some("#000000".to_string());
        object.style.fill = None;
        document.add_object(object);
    }
    document
        .validate()
        .map_err(|error| BeamError::InvalidInput(format!("imported HPGL is invalid: {error}")))?;
    Ok(HpglImport { document, warnings })
}

#[derive(Debug, Clone, PartialEq)]
struct Command {
    name: String,
    params: Vec<f64>,
}

/// Split the stream into two-letter commands and their numeric parameters.
fn commands(src: &str) -> Vec<Command> {
    let mut result = Vec::new();
    let mut characters = src.chars().peekable();
    while let Some(character) = characters.next() {
        if !character.is_ascii_alphabetic() {
            continue;
        }
        let mut name = String::new();
        name.push(character.to_ascii_uppercase());
        if let Some(&next) = characters.peek() {
            if next.is_ascii_alphabetic() {
                name.push(next.to_ascii_uppercase());
                characters.next();
            }
        }
        let mut params = Vec::new();
        let mut number = String::new();
        while let Some(&next) = characters.peek() {
            if next.is_ascii_alphabetic() {
                break;
            }
            characters.next();
            if next == ';' {
                break;
            }
            if next == ',' || next.is_whitespace() {
                if !number.is_empty() {
                    params.push(number.parse().unwrap_or(0.0));
                    number.clear();
                }
                continue;
            }
            number.push(next);
        }
        if !number.is_empty() {
            params.push(number.parse().unwrap_or(0.0));
        }
        result.push(Command { name, params });
    }
    result
}

#[cfg(test)]
mod tests {
    use super::{import_hpgl, import_hpgl_report};
    use crate::document::Shape;

    #[test]
    fn a_square_becomes_one_open_path_in_millimetres() {
        let document = import_hpgl("IN;PA;PU0,0;PD400,0,400,400,0,400,0,0;PU;").unwrap();
        assert_eq!(document.objects.len(), 1);
        let Shape::Path(path) = &document.objects[0].shape else {
            panic!("expected a path");
        };
        assert!(!path.closed);
        assert_eq!(path.segments.len(), 4);
        let last = path.segments.last().unwrap();
        let crate::geometry::path::Segment::Line { to, .. } = last else {
            panic!("expected lines");
        };
        assert!(to.x.abs() < 1e-9 && to.y.abs() < 1e-9);
    }

    #[test]
    fn relative_moves_accumulate() {
        let document = import_hpgl("IN;PA;PU0,0;PR;PD400,0;PD0,400;").unwrap();
        let Shape::Path(path) = &document.objects[0].shape else {
            panic!("expected a path");
        };
        let crate::geometry::path::Segment::Line { to, .. } = path.segments.last().unwrap() else {
            panic!("expected lines");
        };
        assert!((to.x - 10.0).abs() < 1e-9, "{to:?}");
        assert!((to.y - 10.0).abs() < 1e-9, "{to:?}");
    }

    #[test]
    fn each_pen_down_run_is_a_separate_path() {
        let document = import_hpgl("IN;PA;PU0,0;PD400,0;PU0,400;PD400,400;").unwrap();
        assert_eq!(document.objects.len(), 2);
    }

    #[test]
    fn an_empty_file_is_an_empty_document() {
        let document = import_hpgl("   ").unwrap();
        assert_eq!(document.objects.len(), 0);
        assert_eq!(document.layers.len(), 1);
    }

    #[test]
    fn a_file_without_commands_is_rejected() {
        assert!(import_hpgl("12345").is_err());
    }

    #[test]
    fn unknown_commands_are_reported_once() {
        let report = import_hpgl_report("IN;PA;ZZ;ZZ;PU0,0;PD400,0;").unwrap();
        assert_eq!(
            report
                .warnings
                .iter()
                .filter(|warning| warning.contains("ZZ"))
                .count(),
            0
        );
        let report = import_hpgl_report("IN;PA;QQ1;PU0,0;PD400,0;").unwrap();
        assert!(
            report.warnings.iter().any(|warning| warning.contains("QQ")),
            "{:?}",
            report.warnings
        );
    }

    #[test]
    fn an_odd_coordinate_list_is_reported() {
        let report = import_hpgl_report("IN;PA;PU0,0;PD400,0,400;").unwrap();
        assert!(
            report
                .warnings
                .iter()
                .any(|warning| warning.contains("odd")),
            "{:?}",
            report.warnings
        );
    }
}
