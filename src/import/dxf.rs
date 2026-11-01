use std::collections::HashMap;

use crate::document::Document;
use crate::document::model::{Object, Shape};
use crate::errors::{BeamError, Result};
use crate::geometry::{Path, Point2, Transform2, path::Segment};

/// Result of a DXF import, including elements that could not be converted.
#[derive(Debug, Clone, PartialEq)]
pub struct DxfImport {
    pub document: Document,
    pub warnings: Vec<String>,
}

/// Entities converted directly; `INSERT`, `TEXT`, `MTEXT` and `HATCH` are
/// handled separately.
const SUPPORTED: [&str; 6] = ["LINE", "CIRCLE", "ARC", "LWPOLYLINE", "ELLIPSE", "SPLINE"];

/// Import a DXF document into the native model.
///
/// Supported: lines, circles, arcs, lightweight polylines, ellipses and splines;
/// `INSERT` expands its block with position, scale and rotation; `TEXT` and
/// `MTEXT` become text objects; `HATCH` boundaries become closed outlines.
/// Layer names are kept, true colours (420) are exact and indexed colours 8–249
/// are approximated. Unsupported entities are reported in
/// [`DxfImport::warnings`].
pub fn import_dxf(src: &str) -> Result<Document> {
    Ok(import_dxf_report(src)?.document)
}

pub fn import_dxf_report(src: &str) -> Result<DxfImport> {
    let mut document = Document::default();
    let mut warnings = Vec::new();
    if src.trim().is_empty() {
        return Ok(DxfImport { document, warnings });
    }
    let pairs = group_pairs(src);
    if pairs.is_empty() {
        return Err(BeamError::InvalidInput(
            "not a DXF document: no group-code pairs".to_string(),
        ));
    }
    let records = split_records(&pairs);

    let mut layers: HashMap<String, (u8, u8, u8)> = HashMap::new();
    let mut blocks: HashMap<String, Block> = HashMap::new();
    let mut entities: Vec<Record> = Vec::new();
    let mut section = String::new();
    let mut block: Option<(String, Point2, Vec<Record>)> = None;
    for record in records {
        match record.kind.as_str() {
            "SECTION" => {
                section = record.text(2).unwrap_or_default().to_ascii_uppercase();
            }
            "ENDSEC" => section.clear(),
            "BLOCK" => {
                block = Some((
                    record.text(2).unwrap_or_default().to_string(),
                    Point2::new(
                        record.number(10).unwrap_or(0.0),
                        record.number(20).unwrap_or(0.0),
                    ),
                    Vec::new(),
                ));
            }
            "ENDBLK" => {
                if let Some((name, base, members)) = block.take() {
                    if !name.is_empty() {
                        blocks.insert(
                            name,
                            Block {
                                base,
                                entities: members,
                            },
                        );
                    }
                }
            }
            "LAYER" => {
                if let Some(name) = record.text(2) {
                    if let Some(color) = record_color(&record) {
                        layers.insert(name.to_string(), color);
                    }
                }
            }
            "TABLE" | "ENDTAB" | "EOF" | "CLASS" | "VPORT" | "STYLE" | "APPID" | "DIMSTYLE"
            | "BLOCK_RECORD" => {}
            _ => {
                if let Some((_, _, members)) = &mut block {
                    members.push(record);
                } else if section == "ENTITIES" || section.is_empty() {
                    entities.push(record);
                }
            }
        }
    }

    let mut skipped: Vec<String> = Vec::new();
    {
        let mut context = Context {
            blocks: &blocks,
            layer_colors: &layers,
            document: &mut document,
            warnings: &mut warnings,
            skipped: &mut skipped,
            approximate_color: false,
        };
        for record in &entities {
            context.convert(record, Transform2::identity(), None, 0);
        }
        if context.approximate_color {
            warning_once(
                context.warnings,
                "indexed DXF colours were approximated; true colours (420) are exact",
            );
        }
    }
    for entity in skipped {
        warnings.push(format!("unsupported DXF entity skipped: {entity}"));
    }
    document
        .validate()
        .map_err(|error| BeamError::InvalidInput(format!("imported DXF is invalid: {error}")))?;
    Ok(DxfImport { document, warnings })
}

#[derive(Debug, Clone)]
struct Record {
    kind: String,
    fields: Vec<(i32, String)>,
}

impl Record {
    fn text(&self, code: i32) -> Option<&str> {
        self.fields
            .iter()
            .find(|(field, _)| *field == code)
            .map(|(_, value)| value.trim())
    }

    fn number(&self, code: i32) -> Option<f64> {
        self.text(code).and_then(|value| value.parse().ok())
    }

    #[allow(clippy::cast_possible_truncation)]
    fn integer(&self, code: i32) -> Option<i64> {
        self.number(code).map(|value| value as i64)
    }

    fn texts(&self, codes: &[i32]) -> Vec<&str> {
        self.fields
            .iter()
            .filter(|(field, _)| codes.contains(field))
            .map(|(_, value)| value.as_str())
            .collect()
    }
}

#[derive(Debug, Clone)]
struct Block {
    base: Point2,
    entities: Vec<Record>,
}

struct Context<'a> {
    blocks: &'a HashMap<String, Block>,
    layer_colors: &'a HashMap<String, (u8, u8, u8)>,
    document: &'a mut Document,
    warnings: &'a mut Vec<String>,
    skipped: &'a mut Vec<String>,
    approximate_color: bool,
}

impl Context<'_> {
    #[allow(clippy::too_many_lines)]
    fn convert(
        &mut self,
        record: &Record,
        parent: Transform2,
        inherited_layer: Option<&str>,
        depth: usize,
    ) {
        match record.kind.as_str() {
            "INSERT" => self.insert(record, parent, depth),
            "TEXT" => self.text_entity(record, parent, inherited_layer, false),
            "MTEXT" => self.text_entity(record, parent, inherited_layer, true),
            "HATCH" => self.hatch(record, parent, inherited_layer),
            kind if SUPPORTED.contains(&kind) => {
                if let Some(object) = self.geometry_entity(record, parent, inherited_layer) {
                    self.document.add_object(object);
                } else {
                    warning_once(
                        self.warnings,
                        "DXF entities with invalid geometry were skipped",
                    );
                }
            }
            other => {
                if !self.skipped.iter().any(|skipped| skipped == other) {
                    self.skipped.push(other.to_string());
                }
            }
        }
    }

    #[allow(clippy::too_many_lines)]
    fn geometry_entity(
        &mut self,
        record: &Record,
        parent: Transform2,
        inherited_layer: Option<&str>,
    ) -> Option<Object> {
        let layer_id = self.layer_for(record, inherited_layer);
        let object = match record.kind.as_str() {
            "LINE" => {
                let from = Point2::new(record.number(10)?, record.number(20)?);
                let to = Point2::new(record.number(11)?, record.number(21)?);
                let mut path = Path::new();
                path.push(Segment::Line { from, to });
                Object::new(layer_id, Shape::Path(path)).with_transform(parent)
            }
            "CIRCLE" => {
                let center = Point2::new(record.number(10)?, record.number(20)?);
                let radius = record.number(40)?;
                if radius <= 0.0 {
                    return None;
                }
                Object::new(
                    layer_id,
                    Shape::Ellipse {
                        rx: radius,
                        ry: radius,
                    },
                )
                .with_transform(parent.compose(&Transform2::translate(center.x, center.y)))
            }
            "ARC" => {
                let center = Point2::new(record.number(10)?, record.number(20)?);
                let radius = record.number(40)?;
                if radius <= 0.0 {
                    return None;
                }
                let start = record.number(50)?.to_radians();
                let end = record.number(60)?.to_radians();
                Object::new(layer_id, Shape::Path(arc_path(center, radius, start, end)))
                    .with_transform(parent)
            }
            "LWPOLYLINE" => {
                let closed = record.integer(70).is_some_and(|flags| flags & 1 == 1);
                let xs = record
                    .fields
                    .iter()
                    .filter(|(code, _)| *code == 10)
                    .filter_map(|(_, value)| value.trim().parse::<f64>().ok());
                let ys = record
                    .fields
                    .iter()
                    .filter(|(code, _)| *code == 20)
                    .filter_map(|(_, value)| value.trim().parse::<f64>().ok());
                let vertices: Vec<(f64, f64)> = xs.zip(ys).collect();
                if vertices.len() < 2 {
                    return None;
                }
                Object::new(layer_id, Shape::Path(Path::from_points(&vertices, closed)))
                    .with_transform(parent)
            }
            "ELLIPSE" => {
                let centre = Point2::new(record.number(10)?, record.number(20)?);
                let major = Point2::new(record.number(11)?, record.number(21)?);
                let ratio = record.number(40).unwrap_or(1.0).abs();
                if major.x.hypot(major.y) <= 1e-9 || ratio <= 1e-9 {
                    return None;
                }
                let start = record.number(41).unwrap_or(0.0);
                let mut end = record.number(42).unwrap_or(std::f64::consts::TAU);
                if (end - start).abs() < 1e-9 {
                    end = start + std::f64::consts::TAU;
                }
                let sweep = end - start;
                let closed = (sweep.abs() - std::f64::consts::TAU).abs() < 1e-6;
                let minor = Point2::new(-major.y * ratio, major.x * ratio);
                let steps = ((sweep.abs() / std::f64::consts::TAU) * 64.0)
                    .ceil()
                    .max(8.0);
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let steps = steps as usize;
                let mut points = Vec::with_capacity(steps + 1);
                for step in 0..=steps {
                    #[allow(clippy::cast_precision_loss)]
                    let t = start + sweep * step as f64 / steps as f64;
                    points.push((
                        centre.x + major.x * t.cos() + minor.x * t.sin(),
                        centre.y + major.y * t.cos() + minor.y * t.sin(),
                    ));
                }
                if closed && points.len() > 1 {
                    points.pop();
                }
                Object::new(layer_id, Shape::Path(Path::from_points(&points, closed)))
                    .with_transform(parent)
            }
            "SPLINE" => {
                let xs: Vec<f64> = record
                    .fields
                    .iter()
                    .filter(|(code, _)| *code == 10)
                    .filter_map(|(_, value)| value.trim().parse().ok())
                    .collect();
                let ys: Vec<f64> = record
                    .fields
                    .iter()
                    .filter(|(code, _)| *code == 20)
                    .filter_map(|(_, value)| value.trim().parse().ok())
                    .collect();
                let control: Vec<Point2> = xs
                    .iter()
                    .zip(&ys)
                    .map(|(x, y)| Point2::new(*x, *y))
                    .collect();
                if control.len() < 2 {
                    return None;
                }
                let degree = record
                    .integer(71)
                    .map_or(3, |value| usize::try_from(value.max(1)).unwrap_or(1))
                    .clamp(1, control.len() - 1);
                let knots: Vec<f64> = record
                    .fields
                    .iter()
                    .filter(|(code, _)| *code == 40)
                    .filter_map(|(_, value)| value.trim().parse().ok())
                    .collect();
                if record.fields.iter().any(|(code, value)| {
                    *code == 41
                        && value
                            .trim()
                            .parse::<f64>()
                            .is_ok_and(|weight| (weight - 1.0).abs() > 1e-9)
                }) {
                    warning_once(
                        self.warnings,
                        "SPLINE control-point weights are ignored; the curve is not rational",
                    );
                }
                let points = sample_spline(&control, degree, &knots, 64);
                Object::new(layer_id, Shape::Path(Path::from_points(&points, false)))
                    .with_transform(parent)
            }
            _ => return None,
        };
        Some(object)
    }

    fn insert(&mut self, record: &Record, parent: Transform2, depth: usize) {
        let name = record.text(2).unwrap_or_default().to_string();
        if depth >= 8 {
            warning_once(
                self.warnings,
                "DXF block nesting is deeper than eight levels",
            );
            return;
        }
        let Some(block) = self.blocks.get(&name).cloned() else {
            warning_once(
                self.warnings,
                &format!("INSERT references a missing block: {name}"),
            );
            return;
        };
        let insertion = Point2::new(
            record.number(10).unwrap_or(0.0),
            record.number(20).unwrap_or(0.0),
        );
        let scale_x = record.number(41).unwrap_or(1.0);
        let scale_y = record.number(42).unwrap_or(scale_x);
        let rotation = record.number(50).unwrap_or(0.0).to_radians();
        let local = Transform2::translate(insertion.x, insertion.y)
            .compose(&Transform2::rotate(rotation))
            .compose(&Transform2::scale(scale_x, scale_y))
            .compose(&Transform2::translate(-block.base.x, -block.base.y));
        let transform = parent.compose(&local);
        let inherited = record
            .text(8)
            .filter(|name| !name.is_empty() && *name != "0")
            .map(str::to_string);
        for entity in &block.entities {
            self.convert(entity, transform, inherited.as_deref(), depth + 1);
        }
    }

    fn text_entity(
        &mut self,
        record: &Record,
        parent: Transform2,
        inherited_layer: Option<&str>,
        mtext: bool,
    ) {
        let (x, mut y) = if mtext {
            (
                record.number(10).unwrap_or(0.0),
                record.number(20).unwrap_or(0.0),
            )
        } else {
            let aligned = record.integer(72).is_some_and(|value| value != 0)
                && record.number(11).is_some()
                && record.number(21).is_some();
            if aligned {
                (
                    record.number(11).unwrap_or(0.0),
                    record.number(21).unwrap_or(0.0),
                )
            } else {
                (
                    record.number(10).unwrap_or(0.0),
                    record.number(20).unwrap_or(0.0),
                )
            }
        };
        let size = record.number(40).unwrap_or(2.5).max(0.1);
        let content = if mtext {
            clean_mtext(&record.texts(&[1, 3]).concat())
        } else {
            record.text(1).unwrap_or_default().to_string()
        };
        if content.trim().is_empty() {
            return;
        }
        let rotation = record.number(50).unwrap_or(0.0).to_radians();
        let align = if mtext {
            match record.integer(71).unwrap_or(1) {
                2 | 3 | 5 | 6 | 8 | 9 => crate::objects::TextAlign::Center,
                _ => crate::objects::TextAlign::Left,
            }
        } else {
            match record.integer(72).unwrap_or(0) {
                1 => crate::objects::TextAlign::Center,
                2 => crate::objects::TextAlign::Right,
                _ => crate::objects::TextAlign::Left,
            }
        };
        if mtext {
            match record.integer(71).unwrap_or(1) {
                1..=3 => y -= size,
                4..=6 => y -= size / 2.0,
                _ => {}
            }
        }
        let layer_id = self.layer_for(record, inherited_layer);
        let text = crate::objects::TextObject {
            content,
            font: crate::textfont::BUNDLED_NAME.to_string(),
            size,
            x,
            y,
            letter_spacing: 0.0,
            line_spacing: 1.2,
            align,
        };
        let transform = parent.compose(&Transform2::rotate_around(rotation, x, y));
        self.document
            .add_object(Object::new(layer_id, Shape::Text(text)).with_transform(transform));
    }

    fn hatch(&mut self, record: &Record, parent: Transform2, inherited_layer: Option<&str>) {
        let layer_id = self.layer_for(record, inherited_layer);
        let (loops, unsupported) = hatch_loops(&record.fields);
        if !loops.is_empty() {
            warning_once(
                self.warnings,
                "HATCH boundaries were imported as closed outlines; the fill is not reproduced",
            );
        }
        if unsupported {
            warning_once(self.warnings, "HATCH spline boundaries were skipped");
        }
        for mut points in loops {
            if points.len() < 3 {
                continue;
            }
            if points[0] != points[points.len() - 1] {
                points.push(points[0]);
            }
            let tuples: Vec<(f64, f64)> = points.iter().map(|point| (point.x, point.y)).collect();
            let object = Object::new(
                layer_id.clone(),
                Shape::Path(Path::from_points(&tuples, true)),
            )
            .with_transform(parent);
            self.document.add_object(object);
        }
    }

    fn layer_for(&mut self, record: &Record, inherited: Option<&str>) -> String {
        let name = record
            .text(8)
            .filter(|name| !name.is_empty() && *name != "0")
            .or(inherited);
        let Some(name) = name else {
            return self.document.layers[0].id.clone();
        };
        if let Some(layer) = self.document.layers.iter().find(|layer| layer.name == name) {
            return layer.id.clone();
        }
        let color = self
            .entity_color(record)
            .or_else(|| self.layer_colors.get(name).copied())
            .unwrap_or((0, 0, 0));
        let layer = self.document.add_layer(name);
        let id = layer.id.clone();
        if let Some(layer) = self.document.layer_mut(&id) {
            layer.color = crate::color::to_hex(color.0, color.1, color.2);
        }
        id
    }

    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_possible_wrap,
        clippy::cast_sign_loss
    )]
    fn entity_color(&mut self, record: &Record) -> Option<(u8, u8, u8)> {
        if let Some(value) = record.integer(420) {
            let packed = value.rem_euclid(0x100_0000) as u32;
            return Some((
                ((packed >> 16) & 0xFF) as u8,
                ((packed >> 8) & 0xFF) as u8,
                (packed & 0xFF) as u8,
            ));
        }
        let index = record.integer(62)?;
        if index <= 0 || index == 256 {
            return None;
        }
        if index >= 8 {
            self.approximate_color = true;
        }
        Some(aci_to_rgb(index as i32))
    }
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss
)]
fn record_color(record: &Record) -> Option<(u8, u8, u8)> {
    if let Some(value) = record.integer(420) {
        let packed = value.rem_euclid(0x100_0000) as u32;
        return Some((
            ((packed >> 16) & 0xFF) as u8,
            ((packed >> 8) & 0xFF) as u8,
            (packed & 0xFF) as u8,
        ));
    }
    let index = record.integer(62)?;
    (1..=255).contains(&index).then(|| aci_to_rgb(index as i32))
}

/// Approximate an indexed colour.
///
/// 1–7 are the standard colours, 250–255 are the grey ramp and the rest are
/// mapped on the 24-hue wheel with two brightness tiers. Only true colours
/// (420) are exact, so the caller reports the approximation once.
fn aci_to_rgb(index: i32) -> (u8, u8, u8) {
    match index.abs() {
        1 => (255, 0, 0),
        2 => (255, 255, 0),
        3 => (0, 255, 0),
        4 => (0, 255, 255),
        5 => (0, 0, 255),
        6 => (255, 0, 255),
        7 => (255, 255, 255),
        250..=255 => {
            let level = 51 + (index - 250) * 34;
            let value = u8::try_from(level).unwrap_or(255);
            (value, value, value)
        }
        _ => {
            let offset = (index.abs() - 8).max(0);
            let hue = f64::from(offset / 10) * 15.0;
            let shade = offset % 10;
            let (saturation, value) = if shade < 5 {
                (0.95, 1.0 - f64::from(shade) * 0.12)
            } else {
                (0.5, 0.55 - f64::from(shade - 5) * 0.06)
            };
            hsv_to_rgb(hue, saturation, value)
        }
    }
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss
)]
fn hsv_to_rgb(hue: f64, saturation: f64, value: f64) -> (u8, u8, u8) {
    let chroma = value * saturation;
    let sector = (hue / 60.0).rem_euclid(6.0);
    let x = chroma * (1.0 - (sector % 2.0 - 1.0).abs());
    let (red, green, blue) = match sector as u32 {
        0 => (chroma, x, 0.0),
        1 => (x, chroma, 0.0),
        2 => (0.0, chroma, x),
        3 => (0.0, x, chroma),
        4 => (x, 0.0, chroma),
        _ => (chroma, 0.0, x),
    };
    let m = value - chroma;
    let channel = |value: f64| u8::try_from((value * 255.0).round() as i64).unwrap_or(255);
    (channel(red + m), channel(green + m), channel(blue + m))
}

fn warning_once(warnings: &mut Vec<String>, message: &str) {
    if !warnings.iter().any(|warning| warning == message) {
        warnings.push(message.to_string());
    }
}

/// Split the flat group-code pairs into records, one per `0` code.
fn split_records(pairs: &[(i32, String)]) -> Vec<Record> {
    let mut records = Vec::new();
    let mut current: Option<Record> = None;
    for (code, value) in pairs {
        if *code == 0 {
            if let Some(record) = current.take() {
                records.push(record);
            }
            current = Some(Record {
                kind: value.to_ascii_uppercase(),
                fields: Vec::new(),
            });
        } else if let Some(record) = &mut current {
            record.fields.push((*code, value.clone()));
        }
    }
    if let Some(record) = current.take() {
        records.push(record);
    }
    records
}

/// Sample a B-spline through `control`, clamped to its end points.
fn sample_spline(
    control: &[Point2],
    degree: usize,
    knots: &[f64],
    steps: usize,
) -> Vec<(f64, f64)> {
    let count = control.len();
    let degree = degree.clamp(1, count - 1);
    let knots = normalized_knots(count, degree, knots);
    #[allow(clippy::cast_precision_loss)]
    let start = knots[degree];
    let end = knots[count];
    let mut points = Vec::with_capacity(steps + 1);
    for step in 0..=steps {
        #[allow(clippy::cast_precision_loss)]
        let u = start + (end - start) * step as f64 / steps as f64;
        points.push(de_boor(control, degree, &knots, u.min(end - 1e-12)));
    }
    points[0] = (control[0].x, control[0].y);
    points[steps] = (control[count - 1].x, control[count - 1].y);
    points
}

/// A valid, non-decreasing knot vector, clamped at both ends.
fn normalized_knots(count: usize, degree: usize, knots: &[f64]) -> Vec<f64> {
    if knots.len() == count + degree + 1 && knots.windows(2).all(|pair| pair[0] <= pair[1]) {
        return knots.to_vec();
    }
    let spans = count - degree;
    let mut result = vec![0.0; degree + 1];
    for span in 1..spans {
        #[allow(clippy::cast_precision_loss)]
        result.push(span as f64);
    }
    #[allow(clippy::cast_precision_loss)]
    result.extend(std::iter::repeat_n(spans as f64, degree + 1));
    result
}

/// One point on a B-spline, evaluated with de Boor's algorithm.
fn de_boor(control: &[Point2], degree: usize, knots: &[f64], u: f64) -> (f64, f64) {
    let count = control.len();
    let mut span = degree;
    while span < count - 1 && u >= knots[span + 1] {
        span += 1;
    }
    let mut points: Vec<(f64, f64)> = (0..=degree)
        .map(|offset| {
            (
                control[span - degree + offset].x,
                control[span - degree + offset].y,
            )
        })
        .collect();
    for level in 1..=degree {
        for index in (level..=degree).rev() {
            let left = knots[span + index - degree];
            let right = knots[span + 1 + index - level];
            let denominator = right - left;
            let alpha = if denominator.abs() < 1e-12 {
                0.0
            } else {
                (u - left) / denominator
            };
            points[index] = (
                points[index - 1].0 * (1.0 - alpha) + points[index].0 * alpha,
                points[index - 1].1 * (1.0 - alpha) + points[index].1 * alpha,
            );
        }
    }
    points[degree]
}

fn group_pairs(src: &str) -> Vec<(i32, String)> {
    let lines: Vec<&str> = src.lines().collect();
    let mut pairs = Vec::new();
    let mut index = 0;
    while index + 1 < lines.len() {
        if let Ok(code) = lines[index].trim().parse::<i32>() {
            pairs.push((code, lines[index + 1].trim().to_string()));
            index += 2;
        } else {
            index += 1;
        }
    }
    pairs
}

fn arc_path(center: Point2, radius: f64, start: f64, end: f64) -> Path {
    let sweep = if end > start {
        end - start
    } else {
        end - start + std::f64::consts::TAU
    };
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let steps = ((sweep.abs() / std::f64::consts::TAU) * 64.0)
        .ceil()
        .max(8.0) as usize;
    let mut path = Path::new();
    let mut previous = Point2::new(
        center.x + radius * start.cos(),
        center.y + radius * start.sin(),
    );
    for step in 1..=steps {
        #[allow(clippy::cast_precision_loss)]
        let angle = start + sweep * (step as f64 / steps as f64);
        let point = Point2::new(
            center.x + radius * angle.cos(),
            center.y + radius * angle.sin(),
        );
        path.push(Segment::Line {
            from: previous,
            to: point,
        });
        previous = point;
    }
    path
}

/// Clean the inline formatting of an `MTEXT` value.
fn clean_mtext(raw: &str) -> String {
    let mut out = String::new();
    let mut characters = raw.chars().peekable();
    while let Some(character) = characters.next() {
        match character {
            '\\' => match characters.next() {
                Some('P' | 'p') => out.push('\n'),
                Some('~') => out.push(' '),
                Some('\\') => out.push('\\'),
                Some('{') => out.push('{'),
                Some('}') => out.push('}'),
                Some(_) => {
                    // Skip a formatting command up to its semicolon.
                    for next in characters.by_ref() {
                        if next == ';' {
                            break;
                        }
                    }
                }
                None => {}
            },
            '{' | '}' => {}
            other => out.push(other),
        }
    }
    out.trim().to_string()
}

/// Extract the boundary loops of a `HATCH`.
///
/// Returns the sampled loops and whether a spline edge had to be skipped.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn hatch_loops(fields: &[(i32, String)]) -> (Vec<Vec<Point2>>, bool) {
    let mut loops = Vec::new();
    let mut unsupported = false;
    let mut index = 0;
    while index < fields.len() {
        match fields[index].0 {
            93 => {
                let count: usize = fields[index].1.trim().parse().unwrap_or(0);
                index += 1;
                let mut points: Vec<Point2> = Vec::new();
                for _ in 0..count {
                    let (next, edge, skipped) = hatch_edge(fields, index);
                    unsupported |= skipped;
                    index = next;
                    if let Some(mut edge_points) = edge {
                        if let Some(last) = points.last().copied() {
                            if edge_points
                                .first()
                                .is_some_and(|first| first.distance(last) < 1e-6)
                            {
                                edge_points.remove(0);
                            }
                        }
                        points.extend(edge_points);
                    }
                }
                if points.len() >= 3 {
                    loops.push(points);
                }
            }
            _ => index += 1,
        }
    }
    (loops, unsupported)
}

/// Parse one hatch edge starting at its `72` code.
fn hatch_edge(fields: &[(i32, String)], start: usize) -> (usize, Option<Vec<Point2>>, bool) {
    let mut index = start;
    let mut edge_type = 0;
    if let Some((code, value)) = fields.get(index) {
        if *code == 72 {
            edge_type = value.trim().parse().unwrap_or(0);
            index += 1;
        }
    }
    let mut values: HashMap<i32, Vec<f64>> = HashMap::new();
    while let Some((code, value)) = fields.get(index) {
        if matches!(*code, 72 | 91 | 92 | 93) {
            break;
        }
        if let Ok(number) = value.trim().parse::<f64>() {
            values.entry(*code).or_default().push(number);
        }
        index += 1;
    }
    let first = |code: i32| values.get(&code).and_then(|list| list.first()).copied();
    let points = match edge_type {
        1 => match (first(10), first(20), first(11), first(21)) {
            (Some(x0), Some(y0), Some(x1), Some(y1)) => {
                Some(vec![Point2::new(x0, y0), Point2::new(x1, y1)])
            }
            _ => None,
        },
        2 => match (first(10), first(20), first(40), first(50), first(51)) {
            (Some(x), Some(y), Some(radius), Some(start), Some(end)) => Some(sample_arc(
                x,
                y,
                radius,
                start.to_radians(),
                end.to_radians(),
            )),
            _ => None,
        },
        3 => match (
            first(10),
            first(20),
            first(11),
            first(21),
            first(40),
            first(50),
            first(51),
        ) {
            (Some(cx), Some(cy), Some(mx), Some(my), Some(ratio), Some(start), Some(end)) => {
                Some(sample_ellipse(cx, cy, mx, my, ratio, start, end))
            }
            _ => None,
        },
        4 => return (index, None, true),
        _ => None,
    };
    (index, points, false)
}

#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]
fn sample_arc(cx: f64, cy: f64, radius: f64, start: f64, end: f64) -> Vec<Point2> {
    let sweep = if end > start {
        end - start
    } else {
        end - start + std::f64::consts::TAU
    };
    let steps = ((sweep.abs() / std::f64::consts::TAU) * 32.0)
        .ceil()
        .max(4.0);
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let steps = steps as usize;
    (0..=steps)
        .map(|step| {
            let angle = start + sweep * step as f64 / steps as f64;
            Point2::new(cx + radius * angle.cos(), cy + radius * angle.sin())
        })
        .collect()
}

#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]
fn sample_ellipse(
    cx: f64,
    cy: f64,
    major_x: f64,
    major_y: f64,
    ratio: f64,
    start: f64,
    end: f64,
) -> Vec<Point2> {
    let minor = Point2::new(-major_y * ratio, major_x * ratio);
    let sweep = if end > start {
        end - start
    } else {
        end - start + std::f64::consts::TAU
    };
    let steps = ((sweep.abs() / std::f64::consts::TAU) * 32.0)
        .ceil()
        .max(4.0);
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let steps = steps as usize;
    (0..=steps)
        .map(|step| {
            let angle = start + sweep * step as f64 / steps as f64;
            Point2::new(
                cx + major_x * angle.cos() + minor.x * angle.sin(),
                cy + major_y * angle.cos() + minor.y * angle.sin(),
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dxf(body: &str) -> String {
        format!("0\nSECTION\n2\nENTITIES\n{body}0\nENDSEC\n0\nEOF\n")
    }

    fn count_objects(document: &Document) -> usize {
        document.objects.len()
    }

    #[test]
    fn empty_is_empty_document() {
        let document = import_dxf("").unwrap();
        assert_eq!(count_objects(&document), 0);
        assert_eq!(document.layers.len(), 1);
    }

    #[test]
    fn rejects_garbage() {
        assert!(import_dxf("not a dxf").is_err());
    }

    #[test]
    fn line_entity_and_layer() {
        let report =
            import_dxf_report(&dxf("0\nLINE\n8\nCut\n10\n1\n20\n2\n11\n11\n21\n12\n")).unwrap();
        let document = report.document;
        assert_eq!(count_objects(&document), 1);
        assert_eq!(document.layers.len(), 2);
        assert_eq!(document.layers[1].name, "Cut");
        match &document.objects[0].shape {
            Shape::Path(path) => match &path.segments[0] {
                Segment::Line { from, to } => {
                    assert_eq!((from.x, from.y), (1.0, 2.0));
                    assert_eq!((to.x, to.y), (11.0, 12.0));
                }
                Segment::Cubic(_) => panic!("unexpected cubic segment"),
            },
            other => panic!("unexpected shape: {other:?}"),
        }
        assert_eq!(report.warnings.len(), 0);
    }

    #[test]
    fn circle_entity() {
        let document = import_dxf(&dxf("0\nCIRCLE\n10\n5\n20\n5\n40\n2\n")).unwrap();
        assert_eq!(
            document.objects[0].shape,
            Shape::Ellipse { rx: 2.0, ry: 2.0 }
        );
        assert_eq!(document.objects[0].transform.tx, 5.0);
    }

    #[test]
    fn lwpolyline_closed() {
        let document = import_dxf(&dxf(
            "0\nLWPOLYLINE\n90\n3\n70\n1\n10\n0\n20\n0\n10\n10\n20\n0\n10\n10\n20\n10\n",
        ))
        .unwrap();
        match &document.objects[0].shape {
            Shape::Path(path) => {
                assert!(path.closed);
                assert_eq!(path.segment_count(), 3);
            }
            other => panic!("unexpected shape: {other:?}"),
        }
    }

    #[test]
    fn arc_has_endpoints() {
        let document = import_dxf(&dxf("0\nARC\n10\n0\n20\n0\n40\n10\n50\n0\n60\n90\n")).unwrap();
        match &document.objects[0].shape {
            Shape::Path(path) => {
                let first = path.segments.first().unwrap();
                let last = path.segments.last().unwrap();
                let (Segment::Line { from, .. }, Segment::Line { to, .. }) = (first, last) else {
                    panic!("expected line segments");
                };
                assert!((from.x - 10.0).abs() < 1e-6);
                assert!(to.y > 9.9);
            }
            other => panic!("unexpected shape: {other:?}"),
        }
    }

    #[test]
    fn ellipse_entity_becomes_a_sampled_path() {
        let report =
            import_dxf_report(&dxf("0\nELLIPSE\n10\n0\n20\n0\n11\n10\n21\n0\n40\n0.5\n")).unwrap();
        assert_eq!(count_objects(&report.document), 1);
        assert_eq!(report.warnings.len(), 0);
        let Shape::Path(path) = &report.document.objects[0].shape else {
            panic!("expected a path");
        };
        assert!(path.closed);
        let points = path.to_points();
        let max_x = points.iter().map(|(x, _)| *x).fold(f64::MIN, f64::max);
        let max_y = points.iter().map(|(_, y)| *y).fold(f64::MIN, f64::max);
        assert!((max_x - 10.0).abs() < 0.1, "{max_x}");
        assert!((max_y - 5.0).abs() < 0.1, "{max_y}");
    }

    #[test]
    fn spline_entity_is_sampled_between_its_end_points() {
        let report = import_dxf_report(&dxf(
            "0\nSPLINE\n71\n3\n10\n0\n20\n0\n10\n10\n20\n0\n10\n10\n20\n10\n10\n0\n20\n10\n",
        ))
        .unwrap();
        assert_eq!(count_objects(&report.document), 1);
        assert_eq!(report.warnings.len(), 0);
        let Shape::Path(path) = &report.document.objects[0].shape else {
            panic!("expected a path");
        };
        let points = path.to_points();
        assert!((points[0].0).abs() < 1e-9 && (points[0].1).abs() < 1e-9);
        let last = points[points.len() - 1];
        assert!((last.0).abs() < 1e-9 && (last.1 - 10.0).abs() < 1e-9);
        for (x, y) in &points {
            assert!((0.0..=10.0).contains(x), "{x}");
            assert!((0.0..=10.0).contains(y), "{y}");
        }
    }

    #[test]
    fn spline_weights_are_reported_as_ignored() {
        let report = import_dxf_report(&dxf(
            "0\nSPLINE\n71\n3\n10\n0\n20\n0\n10\n10\n20\n0\n10\n10\n20\n10\n10\n0\n20\n10\n41\n2\n",
        ))
        .unwrap();
        assert!(
            report
                .warnings
                .iter()
                .any(|warning| warning.contains("weights")),
            "{:?}",
            report.warnings
        );
    }

    #[test]
    fn a_single_cubic_spline_matches_its_bezier() {
        let control = [
            Point2::new(0.0, 0.0),
            Point2::new(0.0, 10.0),
            Point2::new(10.0, 10.0),
            Point2::new(10.0, 0.0),
        ];
        let points = sample_spline(&control, 3, &[], 8);
        assert!(points[4].0 > 4.9 && points[4].0 < 5.1, "{:?}", points[4]);
        assert!(points[4].1 > 7.0, "{:?}", points[4]);
    }

    #[test]
    fn insert_expands_its_block_with_position() {
        let src = "0\nSECTION\n2\nBLOCKS\n0\nBLOCK\n2\nB1\n10\n0\n20\n0\n0\nLINE\n10\n0\n20\n0\n11\n10\n21\n0\n0\nENDBLK\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n0\nINSERT\n2\nB1\n10\n20\n20\n30\n0\nENDSEC\n0\nEOF\n";
        let document = import_dxf(src).unwrap();
        assert_eq!(count_objects(&document), 1);
        let path = document.objects[0].world_path();
        let first = path.segments.first().unwrap();
        let last = path.segments.last().unwrap();
        let (Segment::Line { from, .. }, Segment::Line { to, .. }) = (first, last) else {
            panic!("expected lines");
        };
        assert!((from.x - 20.0).abs() < 1e-9 && (from.y - 30.0).abs() < 1e-9);
        assert!((to.x - 30.0).abs() < 1e-9 && (to.y - 30.0).abs() < 1e-9);
    }

    #[test]
    fn insert_applies_scale_and_rotation() {
        let src = "0\nSECTION\n2\nBLOCKS\n0\nBLOCK\n2\nB1\n10\n0\n20\n0\n0\nLINE\n10\n0\n20\n0\n11\n10\n21\n0\n0\nENDBLK\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n0\nINSERT\n2\nB1\n10\n0\n20\n0\n41\n2\n42\n2\n0\nENDSEC\n0\nEOF\n";
        let document = import_dxf(src).unwrap();
        let path = document.objects[0].world_path();
        let Segment::Line { to, .. } = path.segments.last().unwrap() else {
            panic!("expected a line");
        };
        assert!((to.x - 20.0).abs() < 1e-9, "{to:?}");
    }

    #[test]
    fn block_content_is_not_imported_without_an_insert() {
        let src = "0\nSECTION\n2\nBLOCKS\n0\nBLOCK\n2\nB1\n10\n0\n20\n0\n0\nLINE\n10\n0\n20\n0\n11\n10\n21\n0\n0\nENDBLK\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n0\nENDSEC\n0\nEOF\n";
        let document = import_dxf(src).unwrap();
        assert_eq!(count_objects(&document), 0);
    }

    #[test]
    fn text_and_mtext_become_text_objects() {
        let document = import_dxf(&dxf(
            "0\nTEXT\n10\n5\n20\n6\n40\n3\n1\nHello\n0\nMTEXT\n10\n10\n20\n10\n40\n2\n71\n1\n1\nLine one\\PLine two\n",
        ))
        .unwrap();
        assert_eq!(count_objects(&document), 2);
        let Shape::Text(first) = &document.objects[0].shape else {
            panic!("expected text");
        };
        assert_eq!(first.content, "Hello");
        assert!((first.size - 3.0).abs() < 1e-9);
        let Shape::Text(second) = &document.objects[1].shape else {
            panic!("expected text");
        };
        assert_eq!(second.content, "Line one\nLine two");
    }

    #[test]
    fn hatch_boundary_becomes_a_closed_path() {
        let report = import_dxf_report(&dxf(
            "0\nHATCH\n91\n1\n92\n1\n93\n3\n72\n1\n10\n0\n20\n0\n11\n10\n21\n0\n72\n1\n10\n10\n20\n0\n11\n0\n21\n10\n72\n1\n10\n0\n20\n10\n11\n0\n21\n0\n",
        ))
        .unwrap();
        assert_eq!(count_objects(&report.document), 1);
        let Shape::Path(path) = &report.document.objects[0].shape else {
            panic!("expected a path");
        };
        assert!(path.closed);
        assert!(
            report
                .warnings
                .iter()
                .any(|warning| warning.contains("fill")),
            "{:?}",
            report.warnings
        );
    }

    #[test]
    fn true_colour_sets_the_layer_colour() {
        let document = import_dxf(&dxf(
            "0\nLINE\n8\nRed\n420\n16711680\n10\n0\n20\n0\n11\n1\n21\n0\n",
        ))
        .unwrap();
        assert_eq!(document.layers[1].color, "#ff0000");
    }

    #[test]
    fn indexed_colours_beyond_seven_are_approximated_once() {
        let report =
            import_dxf_report(&dxf("0\nLINE\n8\nA\n62\n250\n10\n0\n20\n0\n11\n1\n21\n0\n"))
                .unwrap();
        assert!(
            report
                .warnings
                .iter()
                .any(|warning| warning.contains("approximated")),
            "{:?}",
            report.warnings
        );
    }

    #[test]
    fn unsupported_entity_is_reported_once() {
        let report =
            import_dxf_report(&dxf("0\nPOINT\n10\n0\n20\n0\n0\nPOINT\n10\n1\n20\n1\n")).unwrap();
        assert_eq!(count_objects(&report.document), 0);
        assert_eq!(report.warnings.len(), 1);
        assert!(report.warnings[0].contains("POINT"));
    }

    #[test]
    fn invalid_geometry_is_reported() {
        let report = import_dxf_report(&dxf("0\nCIRCLE\n10\n0\n20\n0\n40\n-1\n")).unwrap();
        assert_eq!(count_objects(&report.document), 0);
        assert_eq!(report.warnings.len(), 1);
    }
}
