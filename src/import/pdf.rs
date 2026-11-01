//! Permissive PDF import for vector artwork.
//!
//! The importer reads the first page's content streams, follows the graphics
//! state (`q`/`Q`/`cm`), understands the path operators (`m`, `l`, `c`, `v`,
//! `y`, `h`, `re`) and the painting operators. Text and images are skipped and
//! reported. Streams compressed with Flate are decompressed; other filters are
//! reported instead of guessed.

use std::collections::HashMap;
use std::io::Read as _;

use flate2::read::ZlibDecoder;

use crate::document::Document;
use crate::document::model::{Object, Shape};
use crate::errors::{BeamError, Result};
use crate::geometry::{Path, Point2, Transform2, path::Segment};

/// Millimetres per PDF point.
const MM_PER_POINT: f64 = 25.4 / 72.0;

/// Result of a PDF import.
#[derive(Debug, Clone, PartialEq)]
pub struct PdfImport {
    pub document: Document,
    pub warnings: Vec<String>,
}

/// Import the first page of a PDF as vector paths.
pub fn import_pdf(bytes: &[u8]) -> Result<Document> {
    Ok(import_pdf_report(bytes)?.document)
}

pub fn import_pdf_report(bytes: &[u8]) -> Result<PdfImport> {
    if !bytes.starts_with(b"%PDF-") && !bytes.windows(5).any(|window| window == b"%PDF-") {
        return Err(BeamError::InvalidInput(
            "not a PDF file: the header is missing".to_string(),
        ));
    }
    let mut warnings = Vec::new();
    let (page_width_pt, page_height_pt) = media_box(bytes).unwrap_or((595.0, 842.0));
    let objects = object_bodies(bytes);
    let mut streams = page_streams(&objects);
    if streams.is_empty() {
        streams = every_content_stream(&objects, &mut warnings);
        if !streams.is_empty() {
            warnings.push(
                "the page structure could not be resolved; every content stream was imported"
                    .to_string(),
            );
        }
    }

    let mut document = Document {
        width_mm: (page_width_pt * MM_PER_POINT).max(10.0),
        height_mm: (page_height_pt * MM_PER_POINT).max(10.0),
        ..Document::default()
    };
    let scale = Transform2::scale_uniform(MM_PER_POINT);

    let mut saw_text = false;
    let mut saw_xobject = false;
    let mut filters: Vec<String> = Vec::new();
    let mut count = 0;
    for stream in streams {
        let content = match decode_stream(&stream.dictionary, &stream.data) {
            Ok(content) => content,
            Err(filter) => {
                if !filters.contains(&filter) {
                    filters.push(filter);
                }
                continue;
            }
        };
        let mut parser = ContentParser::new(&content);
        let subpaths = parser.parse(&mut warnings);
        saw_text |= parser.saw_text;
        saw_xobject |= parser.saw_xobject;
        for subpath in subpaths {
            if subpath.path.segments.is_empty() {
                continue;
            }
            let mut object = Object::new(document.layers[0].id.clone(), Shape::Path(subpath.path))
                .with_transform(scale);
            object.name = "PDF path".to_string();
            object.style.stroke = subpath.stroke;
            object.style.fill = subpath.fill;
            if let Some(width) = subpath.line_width_pt {
                object.style.stroke_width_mm = (width * MM_PER_POINT).max(0.01);
            }
            document.add_object(object);
            count += 1;
        }
    }
    if count == 0 {
        return Err(BeamError::InvalidInput(
            "the PDF has no vector paths that can be imported".to_string(),
        ));
    }
    if saw_text {
        warnings.push("PDF text is not imported; convert it to outlines first".to_string());
    }
    if saw_xobject {
        warnings.push("PDF images and form objects are not imported".to_string());
    }
    for filter in filters {
        warnings.push(format!(
            "a PDF stream filtered with {filter} was skipped; only Flate and raw streams are read"
        ));
    }
    document
        .validate()
        .map_err(|error| BeamError::InvalidInput(format!("imported PDF is invalid: {error}")))?;
    Ok(PdfImport { document, warnings })
}

struct PdfStream {
    dictionary: Vec<u8>,
    data: Vec<u8>,
}

/// Objects in the file, keyed by their number.
fn object_bodies(bytes: &[u8]) -> HashMap<u32, Vec<u8>> {
    let mut objects = HashMap::new();
    let mut search = 0;
    while let Some(found) = find(bytes, b"obj", search) {
        // The header is `<number> <generation> obj`; walk back over both.
        let mut cursor = found;
        while cursor > 0 && bytes[cursor - 1].is_ascii_whitespace() {
            cursor -= 1;
        }
        while cursor > 0 && bytes[cursor - 1].is_ascii_digit() {
            cursor -= 1;
        }
        while cursor > 0 && bytes[cursor - 1].is_ascii_whitespace() {
            cursor -= 1;
        }
        let number_end = cursor;
        while cursor > 0 && bytes[cursor - 1].is_ascii_digit() {
            cursor -= 1;
        }
        let number: u32 = std::str::from_utf8(&bytes[cursor..number_end])
            .ok()
            .and_then(|text| text.parse().ok())
            .unwrap_or(0);
        let body_start = (found + 3).min(bytes.len());
        let body_end = find(bytes, b"endobj", body_start).unwrap_or(bytes.len());
        if number > 0 {
            objects
                .entry(number)
                .or_insert_with(|| bytes[body_start..body_end].to_vec());
        }
        search = (body_end + 6).min(bytes.len());
    }
    objects
}

fn find(bytes: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    if from >= bytes.len() {
        return None;
    }
    bytes[from..]
        .windows(needle.len())
        .position(|window| window == needle)
        .map(|index| index + from)
}

fn media_box(bytes: &[u8]) -> Option<(f64, f64)> {
    let found = find(bytes, b"/MediaBox", 0)?;
    let open = find(bytes, b"[", found)?;
    let close = find(bytes, b"]", open)?;
    let text = std::str::from_utf8(&bytes[open + 1..close]).ok()?;
    let values: Vec<f64> = text
        .split_whitespace()
        .filter_map(|value| value.parse().ok())
        .collect();
    if values.len() != 4 {
        return None;
    }
    Some(((values[2] - values[0]).abs(), (values[3] - values[1]).abs()))
}

/// Content streams referenced by the first page.
fn page_streams(objects: &HashMap<u32, Vec<u8>>) -> Vec<PdfStream> {
    let mut best: Option<(usize, u32)> = None;
    for (number, body) in objects {
        let Some(position) = find(body, b"/Type", 0) else {
            continue;
        };
        let rest = &body[position + 5..];
        let rest = rest.strip_prefix(b" ").unwrap_or(rest);
        if rest.starts_with(b"/Page") && !rest.starts_with(b"/Pages") {
            let order = position;
            if best.is_none_or(|(best_order, _)| order < best_order) {
                best = Some((order, *number));
            }
        }
    }
    let Some((_, page_number)) = best else {
        return Vec::new();
    };
    let Some(body) = objects.get(&page_number) else {
        return Vec::new();
    };
    let Some(contents) = find(body, b"/Contents", 0) else {
        return Vec::new();
    };
    let mut result = Vec::new();
    let after = &body[contents + 9..];
    let (start, end) = if let Some(open) = after.iter().position(|byte| *byte == b'[') {
        let close = after
            .iter()
            .position(|byte| *byte == b']')
            .unwrap_or(after.len());
        (open + 1, close)
    } else {
        (0, after.len().min(64))
    };
    let text = String::from_utf8_lossy(&after[start..end]);
    let mut words = text.split_whitespace().peekable();
    while let Some(word) = words.next() {
        if let Ok(number) = word.parse::<u32>() {
            if words.peek() == Some(&"0") {
                words.next();
                if words.peek() == Some(&"R") {
                    words.next();
                }
                if let Some(stream) = stream_of(objects.get(&number)) {
                    result.push(stream);
                }
            }
        }
    }
    result
}

fn every_content_stream(
    objects: &HashMap<u32, Vec<u8>>,
    warnings: &mut Vec<String>,
) -> Vec<PdfStream> {
    let mut result = Vec::new();
    for body in objects.values() {
        if find(body, b"/Subtype /Image", 0).is_some() || find(body, b"/Type /Font", 0).is_some() {
            continue;
        }
        if let Some(stream) = stream_of(Some(body)) {
            result.push(stream);
        }
    }
    if result.is_empty() {
        warnings.push("the PDF has no readable content streams".to_string());
    }
    result
}

fn stream_of(body: Option<&Vec<u8>>) -> Option<PdfStream> {
    let body = body?;
    let start = find(body, b"stream", 0)?;
    let dictionary = body[..start].to_vec();
    let mut data_start = start + 6;
    if body.get(data_start) == Some(&b'\r') {
        data_start += 1;
    }
    if body.get(data_start) == Some(&b'\n') {
        data_start += 1;
    }
    let end = find(body, b"endstream", data_start)?;
    let mut data_end = end;
    while data_end > data_start && matches!(body[data_end - 1], b'\r' | b'\n' | b' ') {
        data_end -= 1;
    }
    Some(PdfStream {
        dictionary,
        data: body[data_start..data_end].to_vec(),
    })
}

fn decode_stream(dictionary: &[u8], data: &[u8]) -> std::result::Result<Vec<u8>, String> {
    let filters = find(dictionary, b"/Filter", 0);
    let Some(filters) = filters else {
        return Ok(data.to_vec());
    };
    let text = String::from_utf8_lossy(&dictionary[filters..]);
    if text.contains("/FlateDecode") {
        let mut decoder = ZlibDecoder::new(data);
        let mut output = Vec::new();
        decoder
            .read_to_end(&mut output)
            .map_err(|_| "FlateDecode (damaged)".to_string())?;
        Ok(output)
    } else {
        let name = text
            .split_whitespace()
            .find(|word| word.starts_with('/') && word.len() > 1)
            .unwrap_or("/Filter")
            .to_string();
        Err(name)
    }
}

#[derive(Debug, Clone, PartialEq)]
struct Painted {
    path: Path,
    stroke: Option<String>,
    fill: Option<String>,
    line_width_pt: Option<f64>,
}

struct ContentParser<'a> {
    tokens: Vec<Token>,
    cursor: usize,
    ctm: Transform2,
    stack: Vec<Transform2>,
    stroke: Option<String>,
    fill: Option<String>,
    line_width: Option<f64>,
    saw_text: bool,
    saw_xobject: bool,
    _source: std::marker::PhantomData<&'a ()>,
}

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Number(f64),
    Operator(String),
    Other,
}

impl<'a> ContentParser<'a> {
    fn new(source: &'a [u8]) -> Self {
        Self {
            tokens: tokenize(source),
            cursor: 0,
            ctm: Transform2::identity(),
            stack: Vec::new(),
            stroke: None,
            fill: None,
            line_width: None,
            saw_text: false,
            saw_xobject: false,
            _source: std::marker::PhantomData,
        }
    }

    #[allow(clippy::too_many_lines)]
    fn parse(&mut self, _warnings: &mut Vec<String>) -> Vec<Painted> {
        let mut painted = Vec::new();
        let mut current: Vec<Path> = Vec::new();
        let mut start = Point2::origin();
        let mut position = Point2::origin();
        let tokens = self.tokens.clone();
        let mut index = 0;
        while index < tokens.len() {
            let Token::Operator(operator) = &tokens[index] else {
                index += 1;
                continue;
            };
            let operands = numbers_before(&tokens, index);
            index += 1;
            match operator.as_str() {
                "q" => self.stack.push(self.ctm),
                "Q" => {
                    if let Some(ctm) = self.stack.pop() {
                        self.ctm = ctm;
                    }
                }
                "cm" => {
                    if let Some([a, b, c, d, e, f]) = array6(&operands) {
                        let matrix = Transform2 {
                            a,
                            b,
                            c,
                            d,
                            tx: e,
                            ty: f,
                        };
                        self.ctm = self.ctm.compose(&matrix);
                    }
                }
                "m" => {
                    if let Some([x, y]) = array2(&operands) {
                        start = self.point(x, y);
                        position = start;
                        current.push(Path::new());
                    }
                }
                "l" => {
                    if let Some([x, y]) = array2(&operands) {
                        let to = self.point(x, y);
                        if let Some(path) = current.last_mut() {
                            path.push(Segment::Line { from: position, to });
                        }
                        position = to;
                    }
                }
                "c" => {
                    if let Some([x1, y1, x2, y2, x3, y3]) = array6(&operands) {
                        let c1 = self.point(x1, y1);
                        let c2 = self.point(x2, y2);
                        let to = self.point(x3, y3);
                        if let Some(path) = current.last_mut() {
                            path.push(Segment::Cubic(crate::geometry::bezier::CubicBezier {
                                p0: position,
                                p1: c1,
                                p2: c2,
                                p3: to,
                            }));
                        }
                        position = to;
                    }
                }
                "v" => {
                    if let Some([x2, y2, x3, y3]) = array4(&operands) {
                        let c2 = self.point(x2, y2);
                        let to = self.point(x3, y3);
                        if let Some(path) = current.last_mut() {
                            path.push(Segment::Cubic(crate::geometry::bezier::CubicBezier {
                                p0: position,
                                p1: position,
                                p2: c2,
                                p3: to,
                            }));
                        }
                        position = to;
                    }
                }
                "y" => {
                    if let Some([x1, y1, x3, y3]) = array4(&operands) {
                        let c1 = self.point(x1, y1);
                        let to = self.point(x3, y3);
                        if let Some(path) = current.last_mut() {
                            path.push(Segment::Cubic(crate::geometry::bezier::CubicBezier {
                                p0: position,
                                p1: c1,
                                p2: to,
                                p3: to,
                            }));
                        }
                        position = to;
                    }
                }
                "h" => {
                    if let Some(path) = current.last_mut() {
                        path.closed = true;
                    }
                    position = start;
                }
                "re" => {
                    if let Some([x, y, w, h]) = array4(&operands) {
                        let corner = self.point(x, y);
                        let opposite = self.point(x + w, y + h);
                        let mut path = Path::new();
                        path.push(Segment::Line {
                            from: corner,
                            to: Point2::new(opposite.x, corner.y),
                        });
                        path.push(Segment::Line {
                            from: Point2::new(opposite.x, corner.y),
                            to: opposite,
                        });
                        path.push(Segment::Line {
                            from: opposite,
                            to: Point2::new(corner.x, opposite.y),
                        });
                        path.push(Segment::Line {
                            from: Point2::new(corner.x, opposite.y),
                            to: corner,
                        });
                        path.closed = true;
                        current.push(path);
                        start = corner;
                        position = corner;
                    }
                }
                "n" => current.clear(),
                "S" | "s" | "f" | "F" | "f*" | "B" | "B*" | "b" | "b*" => {
                    let close = matches!(operator.as_str(), "s" | "b" | "b*");
                    let stroke = matches!(operator.as_str(), "S" | "s" | "B" | "B*" | "b" | "b*");
                    let fill = matches!(
                        operator.as_str(),
                        "f" | "F" | "f*" | "B" | "B*" | "b" | "b*"
                    );
                    for path in current.drain(..) {
                        let mut path = path;
                        if close {
                            path.closed = true;
                        }
                        painted.push(Painted {
                            path,
                            stroke: if stroke {
                                Some(self.stroke.clone().unwrap_or_else(|| "#000000".into()))
                            } else {
                                None
                            },
                            fill: if fill {
                                Some(self.fill.clone().unwrap_or_else(|| "#000000".into()))
                            } else {
                                None
                            },
                            line_width_pt: self.line_width,
                        });
                    }
                }
                "RG" => {
                    if let Some([r, g, b]) = array3(&operands) {
                        self.stroke = Some(pdf_color(r, g, b));
                    }
                }
                "rg" => {
                    if let Some([r, g, b]) = array3(&operands) {
                        self.fill = Some(pdf_color(r, g, b));
                    }
                }
                "G" => {
                    if let Some([value]) = array1(&operands) {
                        self.stroke = Some(pdf_color(value, value, value));
                    }
                }
                "g" => {
                    if let Some([value]) = array1(&operands) {
                        self.fill = Some(pdf_color(value, value, value));
                    }
                }
                "K" => {
                    if let Some([c, m, y, k]) = array4(&operands) {
                        self.stroke = Some(pdf_color(
                            (1.0 - c) * (1.0 - k),
                            (1.0 - m) * (1.0 - k),
                            (1.0 - y) * (1.0 - k),
                        ));
                    }
                }
                "k" => {
                    if let Some([c, m, y, k]) = array4(&operands) {
                        self.fill = Some(pdf_color(
                            (1.0 - c) * (1.0 - k),
                            (1.0 - m) * (1.0 - k),
                            (1.0 - y) * (1.0 - k),
                        ));
                    }
                }
                "w" => {
                    if let Some([value]) = array1(&operands) {
                        self.line_width = Some(value.max(0.01));
                    }
                }
                "BT" | "ET" | "Tj" | "TJ" | "Td" | "TD" | "Tm" | "T*" | "Tf" | "Tc" | "Tw"
                | "TL" | "Ts" | "Tr" | "Tz" => {
                    self.saw_text = true;
                }
                "Do" => self.saw_xobject = true,
                _ => {}
            }
        }
        painted
    }

    fn point(&self, x: f64, y: f64) -> Point2 {
        self.ctm.apply(Point2::new(x, y))
    }
}

fn tokenize(source: &[u8]) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut index = 0;
    while index < source.len() {
        let byte = source[index];
        if byte.is_ascii_whitespace() {
            index += 1;
            continue;
        }
        if byte == b'%' {
            while index < source.len() && source[index] != b'\n' {
                index += 1;
            }
            continue;
        }
        if byte == b'(' {
            index += 1;
            let mut depth = 1;
            while index < source.len() && depth > 0 {
                match source[index] {
                    b'\\' => index += 2,
                    b'(' => {
                        depth += 1;
                        index += 1;
                    }
                    b')' => {
                        depth -= 1;
                        index += 1;
                    }
                    _ => index += 1,
                }
            }
            tokens.push(Token::Other);
            continue;
        }
        if byte == b'<' {
            if source.get(index + 1) == Some(&b'<') {
                tokens.push(Token::Other);
                index += 2;
                continue;
            }
            while index < source.len() && source[index] != b'>' {
                index += 1;
            }
            index += 1;
            tokens.push(Token::Other);
            continue;
        }
        if byte == b'/' {
            index += 1;
            while index < source.len() && !is_delimiter(source[index]) {
                index += 1;
            }
            tokens.push(Token::Other);
            continue;
        }
        if byte.is_ascii_digit() || matches!(byte, b'.' | b'-' | b'+') {
            let start = index;
            index += 1;
            while index < source.len()
                && (source[index].is_ascii_digit()
                    || matches!(source[index], b'.' | b'-' | b'+' | b'e' | b'E'))
            {
                index += 1;
            }
            let text = std::str::from_utf8(&source[start..index]).unwrap_or("0");
            tokens.push(Token::Number(text.parse().unwrap_or(0.0)));
            continue;
        }
        if byte.is_ascii_alphabetic() || matches!(byte, b'*' | b'\'' | b'"') {
            let start = index;
            index += 1;
            while index < source.len()
                && (source[index].is_ascii_alphabetic() || source[index] == b'*')
            {
                index += 1;
            }
            let text = std::str::from_utf8(&source[start..index]).unwrap_or("");
            tokens.push(Token::Operator(text.to_string()));
            continue;
        }
        tokens.push(Token::Other);
        index += 1;
    }
    tokens
}

fn is_delimiter(byte: u8) -> bool {
    byte.is_ascii_whitespace()
        || matches!(
            byte,
            b'(' | b')' | b'<' | b'>' | b'[' | b']' | b'{' | b'}' | b'/' | b'%'
        )
}

/// The numbers immediately before the operator at `index`.
fn numbers_before(tokens: &[Token], index: usize) -> Vec<f64> {
    let mut values = Vec::new();
    let mut cursor = index;
    while cursor > 0 {
        cursor -= 1;
        match &tokens[cursor] {
            Token::Number(value) => values.push(*value),
            Token::Other => {}
            Token::Operator(_) => break,
        }
    }
    values.reverse();
    values
}

fn array1(values: &[f64]) -> Option<[f64; 1]> {
    values.try_into().ok()
}
fn array2(values: &[f64]) -> Option<[f64; 2]> {
    values.try_into().ok()
}
fn array3(values: &[f64]) -> Option<[f64; 3]> {
    values.try_into().ok()
}
fn array4(values: &[f64]) -> Option<[f64; 4]> {
    values.try_into().ok()
}
fn array6(values: &[f64]) -> Option<[f64; 6]> {
    values.try_into().ok()
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn pdf_color(red: f64, green: f64, blue: f64) -> String {
    let channel = |value: f64| (value.clamp(0.0, 1.0) * 255.0).round() as u8;
    crate::color::to_hex(channel(red), channel(green), channel(blue))
}

#[cfg(test)]
mod tests {
    use super::{import_pdf, import_pdf_report};
    use crate::document::Shape;

    fn pdf(content: &str, media: &str) -> Vec<u8> {
        format!(
            "%PDF-1.4\n1 0 obj\n<< /Type /Page /MediaBox [{media}] /Contents 2 0 R >>\nendobj\n2 0 obj\n<< /Length {} >>\nstream\n{content}\nendstream\nendobj\n%%EOF\n",
            content.len()
        )
        .into_bytes()
    }

    #[test]
    fn a_stroked_path_is_imported_in_millimetres() {
        let bytes = pdf("0 0 m 100 0 l 100 100 l h S", "0 0 595 842");
        let report = import_pdf_report(&bytes).unwrap();
        assert_eq!(report.document.objects.len(), 1);
        assert!((report.document.width_mm - 209.902).abs() < 0.01);
        assert!((report.document.height_mm - 297.039).abs() < 0.01);
        let Shape::Path(path) = &report.document.objects[0].shape else {
            panic!("expected a path");
        };
        assert!(path.closed);
        assert_eq!(path.segments.len(), 2);
        assert!((report.document.objects[0].transform.a - super::MM_PER_POINT).abs() < 1e-12);
    }

    #[test]
    fn a_rectangle_operator_becomes_four_segments() {
        let bytes = pdf("10 10 50 20 re f", "0 0 200 200");
        let document = import_pdf(&bytes).unwrap();
        let Shape::Path(path) = &document.objects[0].shape else {
            panic!("expected a path");
        };
        assert_eq!(path.segments.len(), 4);
        assert!(path.closed);
        assert!(document.objects[0].style.fill.is_some());
    }

    #[test]
    fn transforms_are_followed() {
        let bytes = pdf("q 2 0 0 2 0 0 cm 0 0 m 10 0 l S Q", "0 0 100 100");
        let document = import_pdf(&bytes).unwrap();
        let Shape::Path(path) = &document.objects[0].shape else {
            panic!("expected a path");
        };
        let crate::geometry::path::Segment::Line { to, .. } = path.segments[0] else {
            panic!("expected a line");
        };
        assert!((to.x - 20.0).abs() < 1e-9, "{to:?}");
    }

    #[test]
    fn a_flate_stream_is_decompressed() {
        use flate2::Compression;
        use flate2::write::ZlibEncoder;
        use std::io::Write as _;
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(b"0 0 m 50 0 l S").unwrap();
        let compressed = encoder.finish().unwrap();
        let mut bytes = b"%PDF-1.4\n1 0 obj\n<< /Type /Page /MediaBox [0 0 200 200] /Contents 2 0 R >>\nendobj\n2 0 obj\n<< /Length 0 /Filter /FlateDecode >>\nstream\n".to_vec();
        bytes.extend_from_slice(&compressed);
        bytes.extend_from_slice(b"\nendstream\nendobj\n%%EOF\n");
        let document = import_pdf(&bytes).unwrap();
        assert_eq!(document.objects.len(), 1);
    }

    #[test]
    fn text_is_reported_and_skipped() {
        let bytes = pdf("BT /F1 12 Tf (hello) Tj ET", "0 0 200 200");
        assert!(import_pdf(&bytes).is_err());
        let report = import_pdf_report(&pdf(
            "BT /F1 12 Tf (hello) Tj ET 0 0 m 10 0 l S",
            "0 0 200 200",
        ))
        .unwrap();
        assert!(
            report
                .warnings
                .iter()
                .any(|warning| warning.contains("text")),
            "{:?}",
            report.warnings
        );
    }

    #[test]
    fn a_non_pdf_is_rejected() {
        assert!(import_pdf(b"not a pdf").is_err());
    }
}
