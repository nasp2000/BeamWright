#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::too_many_lines,
    clippy::similar_names
)]

use std::collections::HashMap;

use quick_xml::Reader;
use quick_xml::events::Event;

use crate::color::normalize_hex_color;
use crate::document::Document;
use crate::document::model::Object;
use crate::document::shape::{Shape, Style};
use crate::errors::{BeamError, Result};
use crate::geometry::{CubicBezier, Path, Point2, Transform2, path::Segment};
use crate::objects::{ImageObject, TextObject};

const DPI: f64 = 96.0;
const PX_TO_MM: f64 = 25.4 / DPI;

#[derive(Debug, Clone)]
pub struct ImportReport {
    pub document: Document,
    pub warnings: Vec<String>,
}

/// Import an SVG document. Unsupported elements are skipped and reported by
/// [`import_svg_report`].
pub fn import_svg(src: &str) -> Result<Document> {
    Ok(import_svg_report(src)?.document)
}

pub fn import_svg_report(src: &str) -> Result<ImportReport> {
    let root = parse_tree(src)?;
    if root.name != "svg" {
        return Err(BeamError::InvalidInput(
            "SVG root element is not <svg>".to_string(),
        ));
    }

    let mut warnings = Vec::new();
    let mut doc = Document::default();
    doc.layers[0].name = "Imported".to_string();

    let width = root
        .attr("width")
        .and_then(parse_length)
        .and_then(Length::to_mm);
    let height = root
        .attr("height")
        .and_then(parse_length)
        .and_then(Length::to_mm);
    let view_box = root.attr("viewBox").and_then(parse_view_box);

    if let Some((_, _, vb_width, vb_height)) = view_box {
        doc.width_mm = width.unwrap_or(vb_width);
        doc.height_mm = height.unwrap_or(vb_height);
    } else {
        doc.width_mm = width.unwrap_or(300.0);
        doc.height_mm = height.unwrap_or(200.0);
    }
    if !doc.width_mm.is_finite() || doc.width_mm <= 0.0 {
        doc.width_mm = 300.0;
    }
    if !doc.height_mm.is_finite() || doc.height_mm <= 0.0 {
        doc.height_mm = 200.0;
    }

    let root_transform = root_transform(&root, view_box, doc.width_mm, doc.height_mm);
    let root_style = SvgStyle::from_node(&root);
    let layer_id = doc.layers[0].id.clone();

    for child in &root.children {
        process_node(
            child,
            root_transform,
            &root_style,
            &mut doc,
            &layer_id,
            &mut warnings,
        );
    }

    doc.validate()
        .map_err(|error| BeamError::InvalidInput(format!("imported SVG is invalid: {error}")))?;
    Ok(ImportReport {
        document: doc,
        warnings,
    })
}

#[derive(Debug, Default, Clone)]
struct Node {
    name: String,
    attrs: HashMap<String, String>,
    children: Vec<Node>,
    text: String,
}

impl Node {
    fn attr(&self, name: &str) -> Option<&str> {
        self.attrs.get(name).map(String::as_str)
    }

    fn number(&self, name: &str) -> Option<f64> {
        self.attr(name)
            .and_then(|value| value.trim().parse::<f64>().ok())
    }
}

fn parse_tree(src: &str) -> Result<Node> {
    let mut reader = Reader::from_str(src);
    reader.config_mut().trim_text(true);
    let mut stack: Vec<Node> = Vec::new();
    let mut root: Option<Node> = None;

    loop {
        match reader.read_event() {
            Ok(Event::Start(event)) => {
                let node = make_node(&event)?;
                stack.push(node);
            }
            Ok(Event::Empty(event)) => {
                let node = make_node(&event)?;
                attach(&mut stack, &mut root, node);
            }
            Ok(Event::End(_)) => {
                if let Some(node) = stack.pop() {
                    attach(&mut stack, &mut root, node);
                }
            }
            Ok(Event::Text(event)) => {
                if let Some(current) = stack.last_mut() {
                    let text = event.xml_content().map_err(|error| {
                        BeamError::InvalidInput(format!("invalid SVG text: {error}"))
                    })?;
                    current.text.push_str(&text);
                }
            }
            Ok(Event::CData(event)) => {
                if let Some(current) = stack.last_mut() {
                    let text = event.decode().map_err(|error| {
                        BeamError::InvalidInput(format!("invalid SVG CDATA: {error}"))
                    })?;
                    current.text.push_str(&text);
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => {
                return Err(BeamError::InvalidInput(format!("invalid SVG XML: {error}")));
            }
        }
    }

    root.ok_or_else(|| BeamError::InvalidInput("empty SVG document".to_string()))
}

fn make_node(event: &quick_xml::events::BytesStart<'_>) -> Result<Node> {
    let name = String::from_utf8_lossy(event.local_name().as_ref()).into_owned();
    let mut attrs = HashMap::new();
    for attribute in event.attributes().flatten() {
        let key = String::from_utf8_lossy(attribute.key.local_name().as_ref()).into_owned();
        let value = attribute
            .unescape_value()
            .map_err(|error| BeamError::InvalidInput(format!("invalid SVG attribute: {error}")))?
            .into_owned();
        attrs.insert(key, value);
    }
    Ok(Node {
        name,
        attrs,
        children: Vec::new(),
        text: String::new(),
    })
}

fn attach(stack: &mut [Node], root: &mut Option<Node>, node: Node) {
    if let Some(parent) = stack.last_mut() {
        parent.children.push(node);
    } else if root.is_none() {
        *root = Some(node);
    }
}

fn parse_view_box(value: &str) -> Option<(f64, f64, f64, f64)> {
    let values: Vec<f64> = value
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|part| !part.is_empty())
        .filter_map(|part| part.parse::<f64>().ok())
        .collect();
    if values.len() == 4 && values[2] > 0.0 && values[3] > 0.0 {
        Some((values[0], values[1], values[2], values[3]))
    } else {
        None
    }
}

fn root_transform(
    root: &Node,
    view_box: Option<(f64, f64, f64, f64)>,
    width_mm: f64,
    height_mm: f64,
) -> Transform2 {
    if let Some((vb_x, vb_y, vb_width, vb_height)) = view_box {
        let sx = width_mm / vb_width;
        let sy = height_mm / vb_height;
        let preserve = root
            .attr("preserveAspectRatio")
            .unwrap_or("xMidYMid meet")
            .trim();
        if preserve.eq_ignore_ascii_case("none") {
            return Transform2 {
                a: sx,
                b: 0.0,
                c: 0.0,
                d: -sy,
                tx: -vb_x * sx,
                ty: height_mm + vb_y * sy,
            };
        }
        let scale = sx.min(sy);
        let offset_x = (width_mm - vb_width * scale) / 2.0;
        let offset_y = (height_mm - vb_height * scale) / 2.0;
        return Transform2 {
            a: scale,
            b: 0.0,
            c: 0.0,
            d: -scale,
            tx: offset_x - vb_x * scale,
            ty: height_mm - offset_y + vb_y * scale,
        };
    }
    Transform2 {
        a: PX_TO_MM,
        b: 0.0,
        c: 0.0,
        d: -PX_TO_MM,
        tx: 0.0,
        ty: height_mm,
    }
}

fn process_node(
    node: &Node,
    parent_transform: Transform2,
    parent_style: &SvgStyle,
    doc: &mut Document,
    layer_id: &str,
    warnings: &mut Vec<String>,
) {
    let local = node
        .attr("transform")
        .map(parse_transform)
        .unwrap_or_default();
    let transform = parent_transform.compose(&local);
    let style = parent_style.inherit(node);

    match node.name.as_str() {
        "g" | "a" | "svg" => {
            for child in &node.children {
                process_node(child, transform, &style, doc, layer_id, warnings);
            }
        }
        "defs" | "clipPath" | "mask" | "symbol" | "pattern" | "marker" | "metadata" | "title"
        | "desc" | "style" => {}
        "use" => warnings.push("<use> references are not expanded".to_string()),
        "rect" => import_rect(node, transform, &style, doc, layer_id, warnings),
        "circle" => import_circle(node, transform, &style, doc, layer_id, warnings),
        "ellipse" => import_ellipse(node, transform, &style, doc, layer_id, warnings),
        "line" => import_line(node, transform, &style, doc, layer_id),
        "polyline" => import_poly(node, transform, &style, doc, layer_id, false, warnings),
        "polygon" => import_poly(node, transform, &style, doc, layer_id, true, warnings),
        "path" => import_path(node, transform, &style, doc, layer_id, warnings),
        "text" => import_text(node, transform, &style, doc, layer_id),
        "image" => import_image(node, transform, &style, doc, layer_id),
        other => warnings.push(format!("unsupported SVG element: <{other}>")),
    }
}

fn import_rect(
    node: &Node,
    transform: Transform2,
    style: &SvgStyle,
    doc: &mut Document,
    layer_id: &str,
    warnings: &mut Vec<String>,
) {
    let x = node.number("x").unwrap_or(0.0);
    let y = node.number("y").unwrap_or(0.0);
    let width = node.number("width").unwrap_or(0.0);
    let height = node.number("height").unwrap_or(0.0);
    if width <= 0.0 || height <= 0.0 {
        warnings.push("skipped a rectangle with non-positive size".to_string());
        return;
    }
    let corner_radius = node
        .number("rx")
        .or_else(|| node.number("ry"))
        .unwrap_or(0.0);
    let shape = Shape::Rect {
        width,
        height,
        corner_radius,
    };
    let object = Object::new(layer_id, shape)
        .with_transform(transform.compose(&Transform2::translate(x, y)))
        .with_style(style.to_object_style(&transform));
    doc.add_object(object);
}

fn import_circle(
    node: &Node,
    transform: Transform2,
    style: &SvgStyle,
    doc: &mut Document,
    layer_id: &str,
    warnings: &mut Vec<String>,
) {
    let radius = node.number("r").unwrap_or(0.0);
    if radius <= 0.0 {
        warnings.push("skipped a circle with non-positive radius".to_string());
        return;
    }
    let cx = node.number("cx").unwrap_or(0.0);
    let cy = node.number("cy").unwrap_or(0.0);
    let object = Object::new(
        layer_id,
        Shape::Ellipse {
            rx: radius,
            ry: radius,
        },
    )
    .with_transform(transform.compose(&Transform2::translate(cx, cy)))
    .with_style(style.to_object_style(&transform));
    doc.add_object(object);
}

fn import_ellipse(
    node: &Node,
    transform: Transform2,
    style: &SvgStyle,
    doc: &mut Document,
    layer_id: &str,
    warnings: &mut Vec<String>,
) {
    let rx = node.number("rx").unwrap_or(0.0);
    let ry = node.number("ry").unwrap_or(0.0);
    if rx <= 0.0 || ry <= 0.0 {
        warnings.push("skipped an ellipse with non-positive radii".to_string());
        return;
    }
    let cx = node.number("cx").unwrap_or(0.0);
    let cy = node.number("cy").unwrap_or(0.0);
    let object = Object::new(layer_id, Shape::Ellipse { rx, ry })
        .with_transform(transform.compose(&Transform2::translate(cx, cy)))
        .with_style(style.to_object_style(&transform));
    doc.add_object(object);
}

fn import_line(
    node: &Node,
    transform: Transform2,
    style: &SvgStyle,
    doc: &mut Document,
    layer_id: &str,
) {
    let x1 = node.number("x1").unwrap_or(0.0);
    let y1 = node.number("y1").unwrap_or(0.0);
    let x2 = node.number("x2").unwrap_or(0.0);
    let y2 = node.number("y2").unwrap_or(0.0);
    let path = Path {
        segments: vec![Segment::Line {
            from: Point2::new(x1, y1),
            to: Point2::new(x2, y2),
        }],
        closed: false,
    };
    let object = Object::new(layer_id, Shape::Path(path))
        .with_transform(transform)
        .with_style(style.to_object_style(&transform));
    doc.add_object(object);
}

fn import_poly(
    node: &Node,
    transform: Transform2,
    style: &SvgStyle,
    doc: &mut Document,
    layer_id: &str,
    closed: bool,
    warnings: &mut Vec<String>,
) {
    let Some(points) = node.attr("points") else {
        return;
    };
    let values: Vec<f64> = points
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|part| !part.is_empty())
        .filter_map(|part| part.parse::<f64>().ok())
        .collect();
    if values.len() < 4 || values.len() % 2 != 0 {
        warnings.push("skipped a polyline/polygon with invalid points".to_string());
        return;
    }
    let pairs: Vec<(f64, f64)> = values
        .chunks_exact(2)
        .map(|chunk| (chunk[0], chunk[1]))
        .collect();
    let path = Path::from_points(&pairs, closed);
    let object = Object::new(layer_id, Shape::Path(path))
        .with_transform(transform)
        .with_style(style.to_object_style(&transform));
    doc.add_object(object);
}

fn import_path(
    node: &Node,
    transform: Transform2,
    style: &SvgStyle,
    doc: &mut Document,
    layer_id: &str,
    warnings: &mut Vec<String>,
) {
    let Some(data) = node.attr("d") else {
        return;
    };
    let bez = match kurbo::BezPath::from_svg(data) {
        Ok(bez) => bez,
        Err(error) => {
            warnings.push(format!("skipped an invalid path: {error}"));
            return;
        }
    };
    for path in bezpath_to_paths(&bez) {
        let object = Object::new(layer_id, Shape::Path(path))
            .with_transform(transform)
            .with_style(style.to_object_style(&transform));
        doc.add_object(object);
    }
}

fn import_text(
    node: &Node,
    transform: Transform2,
    style: &SvgStyle,
    doc: &mut Document,
    layer_id: &str,
) {
    let content = node.text.trim();
    if content.is_empty() {
        return;
    }
    let x = node.number("x").unwrap_or(0.0);
    let y = node.number("y").unwrap_or(0.0);
    let size = node
        .attr("font-size")
        .and_then(parse_length)
        .map_or(12.0, Length::numeric);
    let font = node
        .attr("font-family")
        .unwrap_or("sans")
        .trim()
        .to_string();
    let world = transform.apply(Point2::new(x, y));
    let scale = transform_scale(&transform);
    let align = match node.attr("text-anchor") {
        Some("middle") => crate::objects::TextAlign::Center,
        Some("end") => crate::objects::TextAlign::Right,
        _ => crate::objects::TextAlign::Left,
    };
    let letter_spacing = node
        .attr("letter-spacing")
        .and_then(parse_length)
        .map_or(0.0, |length| length.numeric() * scale);
    let text = TextObject {
        content: content.to_string(),
        font,
        size: (size * scale).max(0.1),
        x: world.x,
        y: world.y,
        letter_spacing,
        align,
        ..TextObject::default()
    };
    let mut object = Object::new(layer_id, Shape::Text(text));
    object.style = style.to_object_style(&transform);
    if object.style.fill.is_none() && object.style.stroke.is_none() {
        object.style.fill = Some("#000000".to_string());
    }
    doc.add_object(object);
}

fn import_image(
    node: &Node,
    transform: Transform2,
    style: &SvgStyle,
    doc: &mut Document,
    layer_id: &str,
) {
    let Some(href) = node.attr("href").map(str::to_string) else {
        return;
    };
    if href.is_empty() {
        return;
    }
    let x = node.number("x").unwrap_or(0.0);
    let y = node.number("y").unwrap_or(0.0);
    let width = node.number("width").unwrap_or(1.0).max(0.1);
    let height = node.number("height").unwrap_or(1.0).max(0.1);
    let scale = transform_scale(&transform);
    let width_mm = width * scale;
    let height_mm = height * scale;
    let image = ImageObject::new(
        href,
        (width_mm * DPI / 25.4).round().max(1.0) as u32,
        (height_mm * DPI / 25.4).round().max(1.0) as u32,
        DPI,
    );
    let world = transform.apply(Point2::new(x, y));
    let object = Object::new(layer_id, Shape::Image(image))
        .with_transform(Transform2::translate(world.x, world.y))
        .with_style(style.to_object_style(&transform));
    doc.add_object(object);
}

fn bezpath_to_paths(bez: &kurbo::BezPath) -> Vec<Path> {
    let mut paths = Vec::new();
    let mut current = Path::new();
    let mut last: Option<Point2> = None;

    for element in bez.elements() {
        match *element {
            kurbo::PathEl::MoveTo(point) => {
                if !current.segments.is_empty() {
                    paths.push(current);
                    current = Path::new();
                }
                last = Some(Point2::new(point.x, point.y));
            }
            kurbo::PathEl::LineTo(point) => {
                let to = Point2::new(point.x, point.y);
                if let Some(from) = last {
                    current.push(Segment::Line { from, to });
                }
                last = Some(to);
            }
            kurbo::PathEl::QuadTo(control, point) => {
                let to = Point2::new(point.x, point.y);
                if let Some(from) = last {
                    let control = Point2::new(control.x, control.y);
                    current.push(Segment::Cubic(CubicBezier {
                        p0: from,
                        p1: from.add(control.sub(from).scale(2.0 / 3.0)),
                        p2: to.add(control.sub(to).scale(2.0 / 3.0)),
                        p3: to,
                    }));
                }
                last = Some(to);
            }
            kurbo::PathEl::CurveTo(control_a, control_b, point) => {
                let to = Point2::new(point.x, point.y);
                if let Some(from) = last {
                    current.push(Segment::Cubic(CubicBezier {
                        p0: from,
                        p1: Point2::new(control_a.x, control_a.y),
                        p2: Point2::new(control_b.x, control_b.y),
                        p3: to,
                    }));
                }
                last = Some(to);
            }
            kurbo::PathEl::ClosePath => {
                current.closed = true;
                if !current.segments.is_empty() {
                    paths.push(current);
                    current = Path::new();
                }
                last = None;
            }
        }
    }
    if !current.segments.is_empty() {
        paths.push(current);
    }
    paths
}

#[derive(Debug, Clone, Default)]
#[allow(clippy::option_option)]
struct SvgStyle {
    stroke: Option<Option<String>>,
    fill: Option<Option<String>>,
    stroke_width: Option<f64>,
}

impl SvgStyle {
    fn from_node(node: &Node) -> Self {
        let mut style = Self::default();
        style.apply_attributes(node);
        style
    }

    fn inherit(&self, node: &Node) -> Self {
        let mut style = self.clone();
        style.apply_attributes(node);
        style
    }

    fn apply_attributes(&mut self, node: &Node) {
        if let Some(value) = node.attr("fill") {
            self.fill = paint(value);
        }
        if let Some(value) = node.attr("stroke") {
            self.stroke = paint(value);
        }
        if let Some(value) = node.attr("stroke-width") {
            self.stroke_width = parse_length(value).map(Length::numeric);
        }
        if let Some(value) = node.attr("style") {
            for declaration in value.split(';') {
                let Some((key, value)) = declaration.split_once(':') else {
                    continue;
                };
                match key.trim() {
                    "fill" => self.fill = paint(value),
                    "stroke" => self.stroke = paint(value),
                    "stroke-width" => {
                        self.stroke_width = parse_length(value).map(Length::numeric);
                    }
                    _ => {}
                }
            }
        }
    }

    fn to_object_style(&self, transform: &Transform2) -> Style {
        let scale = transform_scale(transform);
        let stroke = self
            .stroke
            .clone()
            .unwrap_or_else(|| Some("#000000".to_string()));
        let fill = self.fill.clone().unwrap_or(None);
        Style {
            stroke,
            fill,
            stroke_width_mm: (self.stroke_width.unwrap_or(1.0) * scale).max(0.01),
        }
    }
}

fn transform_scale(transform: &Transform2) -> f64 {
    let sx = (transform.a * transform.a + transform.b * transform.b).sqrt();
    let sy = (transform.c * transform.c + transform.d * transform.d).sqrt();
    f64::midpoint(sx, sy).max(1e-9)
}

#[allow(clippy::option_option)]
fn paint(value: &str) -> Option<Option<String>> {
    let value = value.trim();
    if value.eq_ignore_ascii_case("none") || value.eq_ignore_ascii_case("transparent") {
        return Some(None);
    }
    if let Some(color) = normalize_hex_color(value) {
        return Some(Some(color));
    }
    let named = match value.to_ascii_lowercase().as_str() {
        "black" | "currentcolor" => "#000000",
        "white" => "#ffffff",
        "red" => "#ff0000",
        "green" => "#008000",
        "blue" => "#0000ff",
        "yellow" => "#ffff00",
        "cyan" | "aqua" => "#00ffff",
        "magenta" | "fuchsia" => "#ff00ff",
        "gray" | "grey" => "#808080",
        "silver" => "#c0c0c0",
        "maroon" => "#800000",
        "olive" => "#808000",
        "lime" => "#00ff00",
        "navy" => "#000080",
        "teal" => "#008080",
        "purple" => "#800080",
        "orange" => "#ffa500",
        _ => return None,
    };
    Some(Some(named.to_string()))
}

#[derive(Debug, Clone, Copy)]
struct Length {
    value: f64,
    unit: LengthUnit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LengthUnit {
    None,
    Px,
    Mm,
    Cm,
    In,
    Pt,
    Pc,
    Percent,
}

impl Length {
    fn numeric(self) -> f64 {
        self.value
    }

    fn to_mm(self) -> Option<f64> {
        match self.unit {
            LengthUnit::None | LengthUnit::Px => Some(self.value * PX_TO_MM),
            LengthUnit::Mm => Some(self.value),
            LengthUnit::Cm => Some(self.value * 10.0),
            LengthUnit::In => Some(self.value * 25.4),
            LengthUnit::Pt => Some(self.value * 25.4 / 72.0),
            LengthUnit::Pc => Some(self.value * 25.4 / 6.0),
            LengthUnit::Percent => None,
        }
    }
}

fn parse_length(value: &str) -> Option<Length> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    let (number, unit) = if let Some(stripped) = value.strip_suffix("px") {
        (stripped, LengthUnit::Px)
    } else if let Some(stripped) = value.strip_suffix("mm") {
        (stripped, LengthUnit::Mm)
    } else if let Some(stripped) = value.strip_suffix("cm") {
        (stripped, LengthUnit::Cm)
    } else if let Some(stripped) = value.strip_suffix("in") {
        (stripped, LengthUnit::In)
    } else if let Some(stripped) = value.strip_suffix("pt") {
        (stripped, LengthUnit::Pt)
    } else if let Some(stripped) = value.strip_suffix("pc") {
        (stripped, LengthUnit::Pc)
    } else if let Some(stripped) = value.strip_suffix('%') {
        (stripped, LengthUnit::Percent)
    } else {
        (value, LengthUnit::None)
    };
    let value = number.trim().parse::<f64>().ok()?;
    Some(Length { value, unit })
}

fn parse_transform(value: &str) -> Transform2 {
    let mut result = Transform2::identity();
    let mut rest = value;
    while let Some(open) = rest.find('(') {
        let name = rest[..open].trim();
        let Some(close) = rest[open..].find(')') else {
            break;
        };
        let args_text = &rest[open + 1..open + close];
        let args: Vec<f64> = args_text
            .split(|c: char| c == ',' || c.is_whitespace())
            .filter(|part| !part.is_empty())
            .filter_map(|part| part.parse::<f64>().ok())
            .collect();
        let transform = match name {
            "matrix" if args.len() == 6 => Transform2 {
                a: args[0],
                b: args[1],
                c: args[2],
                d: args[3],
                tx: args[4],
                ty: args[5],
            },
            "translate" if !args.is_empty() => {
                Transform2::translate(args[0], args.get(1).copied().unwrap_or(0.0))
            }
            "scale" if !args.is_empty() => {
                Transform2::scale(args[0], args.get(1).copied().unwrap_or(args[0]))
            }
            "rotate" if !args.is_empty() => {
                let angle = args[0].to_radians();
                if args.len() >= 3 {
                    Transform2::rotate_around(angle, args[1], args[2])
                } else {
                    Transform2::rotate(angle)
                }
            }
            "skewX" if !args.is_empty() => Transform2 {
                a: 1.0,
                b: 0.0,
                c: args[0].to_radians().tan(),
                d: 1.0,
                tx: 0.0,
                ty: 0.0,
            },
            "skewY" if !args.is_empty() => Transform2 {
                a: 1.0,
                b: args[0].to_radians().tan(),
                c: 0.0,
                d: 1.0,
                tx: 0.0,
                ty: 0.0,
            },
            _ => Transform2::identity(),
        };
        result = result.compose(&transform);
        rest = &rest[open + close + 1..];
    }
    result
}

#[cfg(test)]
#[allow(clippy::assert_is_empty)]
mod tests {
    use super::*;

    const SVG_UNITS: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100mm" height="80mm" viewBox="0 0 100 80"><rect x="10" y="20" width="30" height="15"/></svg>"#;
    const SVG_GROUP: &str = r#"<svg width="100mm" height="100mm" viewBox="0 0 100 100"><g transform="translate(10,10) scale(2)"><rect x="0" y="0" width="10" height="10"/></g></svg>"#;
    const SVG_ARC: &str = r#"<svg width="100" height="100" viewBox="0 0 100 100"><path d="M10 10 L 50 10 A 20 20 0 0 1 50 50 Z"/></svg>"#;
    const SVG_SHAPES: &str = r#"<svg width="100" height="100" viewBox="0 0 100 100"><circle cx="50" cy="50" r="10"/><line x1="0" y1="0" x2="10" y2="10"/></svg>"#;
    const SVG_USE: &str = r##"<svg width="10" height="10"><use href="#x"/></svg>"##;
    const SVG_STYLE: &str = r##"<svg width="10mm" height="10mm" viewBox="0 0 10 10"><rect width="1" height="1" stroke="none" fill="#ff0000" stroke-width="0.3"/></svg>"##;
    const SVG_TEXT: &str = r#"<svg><text x="1" y="2" font-size="4">hello</text></svg>"#;

    #[test]
    fn empty_svg() {
        let doc = import_svg("<svg></svg>").unwrap();
        assert!(doc.objects.is_empty());
    }

    #[test]
    fn imports_rect_with_units() {
        let doc = import_svg(SVG_UNITS).unwrap();
        assert_eq!(doc.objects.len(), 1);
        assert!((doc.width_mm - 100.0).abs() < 1e-6);
        assert!((doc.height_mm - 80.0).abs() < 1e-6);
        let (min, max) = doc.objects[0].bounds().unwrap();
        assert!((min.x - 10.0).abs() < 1e-6);
        assert!((max.x - 40.0).abs() < 1e-6);
        // SVG y is flipped: y=20..35 becomes 45..60 in an 80 mm document.
        assert!((min.y - 45.0).abs() < 1e-6);
        assert!((max.y - 60.0).abs() < 1e-6);
    }

    #[test]
    fn imports_group_transform() {
        let doc = import_svg(SVG_GROUP).unwrap();
        let (min, max) = doc.objects[0].bounds().unwrap();
        assert!((min.x - 10.0).abs() < 1e-6);
        assert!((max.x - 30.0).abs() < 1e-6);
    }

    #[test]
    fn imports_path_with_arc() {
        let doc = import_svg(SVG_ARC).unwrap();
        assert_eq!(doc.objects.len(), 1);
        assert!(doc.objects[0].world_path().segment_count() >= 3);
    }

    #[test]
    fn imports_text_alignment_and_spacing() {
        let svg = r#"<svg width="50" height="20"><text x="10" y="5" text-anchor="middle" letter-spacing="2" font-size="4">Hi</text></svg>"#;
        let report = import_svg_report(svg).unwrap();
        let object = report.document.objects.first().expect("text object");
        let crate::document::Shape::Text(text) = &object.shape else {
            panic!("expected text");
        };
        assert_eq!(text.align, crate::objects::TextAlign::Center);
        assert!(text.letter_spacing > 0.0);
    }

    #[test]
    fn imports_circle_and_line() {
        let doc = import_svg(SVG_SHAPES).unwrap();
        assert_eq!(doc.objects.len(), 2);
    }

    #[test]
    fn reports_unsupported_elements() {
        let report = import_svg_report(SVG_USE).unwrap();
        assert!(!report.warnings.is_empty());
    }

    #[test]
    fn invalid_xml_is_an_error() {
        assert!(import_svg("<svg><rect></svg>").is_err());
    }

    #[test]
    fn parse_transform_scale() {
        let transform = parse_transform("translate(10,20) scale(2)");
        let point = transform.apply(Point2::new(1.0, 1.0));
        assert!((point.x - 12.0).abs() < 1e-9);
        assert!((point.y - 22.0).abs() < 1e-9);
    }

    #[test]
    fn length_units() {
        assert!((parse_length("25.4mm").unwrap().to_mm().unwrap() - 25.4).abs() < 1e-9);
        assert!((parse_length("1in").unwrap().to_mm().unwrap() - 25.4).abs() < 1e-9);
        assert!((parse_length("96px").unwrap().to_mm().unwrap() - 25.4).abs() < 1e-9);
        assert!(parse_length("50%").unwrap().to_mm().is_none());
    }

    #[test]
    fn style_is_read() {
        let doc = import_svg(SVG_STYLE).unwrap();
        let style = &doc.objects[0].style;
        assert!(style.stroke.is_none());
        assert_eq!(style.fill.as_deref(), Some("#ff0000"));
        assert!((style.stroke_width_mm - 0.3).abs() < 1e-9);
    }

    #[test]
    fn text_element() {
        let doc = import_svg(SVG_TEXT).unwrap();
        match &doc.objects[0].shape {
            Shape::Text(text) => assert_eq!(text.content, "hello"),
            other => panic!("unexpected shape: {other:?}"),
        }
    }
}
