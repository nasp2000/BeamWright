use std::fmt::Write as _;

use crate::document::{Document, Object, Shape, Style};
use crate::errors::Result;
use crate::geometry::{Path, Point2, Transform2, path::Segment};

/// Export visible document content as SVG.
///
/// Internal coordinates are millimetres with `y` up; SVG uses `y` down, so
/// geometry is flipped around the page height while it is written.
pub fn export_svg(doc: &Document) -> Result<String> {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}mm\" height=\"{h}mm\" viewBox=\"0 0 {w} {h}\">",
        w = fmt_num(doc.width_mm),
        h = fmt_num(doc.height_mm)
    );
    for layer in &doc.layers {
        if !layer.visible {
            continue;
        }
        let _ = writeln!(
            out,
            "  <g id=\"{}\" data-name=\"{}\">",
            escape(&layer.id),
            escape(&layer.name)
        );
        for object in doc
            .objects
            .iter()
            .filter(|object| object.layer_id == layer.id && object.visible)
        {
            write_object(&mut out, object, layer.rgb(), doc.height_mm);
        }
        out.push_str("  </g>\n");
    }
    out.push_str("</svg>\n");
    Ok(out)
}

fn write_object(out: &mut String, object: &Object, layer_rgb: (u8, u8, u8), page_height: f64) {
    let style = &object.style;
    let (stroke, fill) = effective_colors(style, layer_rgb);
    match &object.shape {
        Shape::Text(text) => {
            let world = object.transform.apply(Point2::new(text.x, text.y));
            let scale = transform_scale(&object.transform);
            let _ = writeln!(
                out,
                "    <text x=\"{}\" y=\"{}\" font-family=\"{}\" font-size=\"{}\" fill=\"{}\" stroke=\"{}\" stroke-width=\"{}\">{}</text>",
                fmt_num(world.x),
                fmt_num(page_height - world.y),
                escape(&text.font),
                fmt_num(text.size * scale),
                fill,
                stroke,
                fmt_num(style.stroke_width_mm),
                escape(&text.content)
            );
        }
        Shape::Image(image) => {
            let Some((min, max)) = object.bounds() else {
                return;
            };
            let _ = writeln!(
                out,
                "    <image href=\"{}\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"/>",
                escape(&image.path),
                fmt_num(min.x),
                fmt_num(page_height - max.y),
                fmt_num(max.x - min.x),
                fmt_num(max.y - min.y)
            );
        }
        _ => {
            let flip = Transform2 {
                a: 1.0,
                b: 0.0,
                c: 0.0,
                d: -1.0,
                tx: 0.0,
                ty: page_height,
            };
            let path = object.world_path().transform(&flip);
            let d = path_to_svg(&path);
            if d.is_empty() {
                return;
            }
            let _ = writeln!(
                out,
                "    <path d=\"{}\" fill=\"{}\" stroke=\"{}\" stroke-width=\"{}\"/>",
                d,
                fill,
                stroke,
                fmt_num(style.stroke_width_mm)
            );
        }
    }
}

fn effective_colors(style: &Style, layer_rgb: (u8, u8, u8)) -> (String, String) {
    let fallback = crate::color::to_hex(layer_rgb.0, layer_rgb.1, layer_rgb.2);
    let stroke = style.stroke.clone().unwrap_or(fallback);
    let fill = style.fill.clone().unwrap_or_else(|| "none".to_string());
    (stroke, fill)
}

fn path_to_svg(path: &Path) -> String {
    let mut out = String::new();
    let mut current: Option<Point2> = None;
    for segment in &path.segments {
        match segment {
            Segment::Line { from, to } => {
                if current != Some(*from) {
                    let _ = write!(out, "M{} {} ", fmt_num(from.x), fmt_num(from.y));
                }
                let _ = write!(out, "L{} {} ", fmt_num(to.x), fmt_num(to.y));
                current = Some(*to);
            }
            Segment::Cubic(c) => {
                if current != Some(c.p0) {
                    let _ = write!(out, "M{} {} ", fmt_num(c.p0.x), fmt_num(c.p0.y));
                }
                let _ = write!(
                    out,
                    "C{} {} {} {} {} {} ",
                    fmt_num(c.p1.x),
                    fmt_num(c.p1.y),
                    fmt_num(c.p2.x),
                    fmt_num(c.p2.y),
                    fmt_num(c.p3.x),
                    fmt_num(c.p3.y)
                );
                current = Some(c.p3);
            }
        }
    }
    if path.closed && !path.segments.is_empty() {
        out.push('Z');
    }
    out.trim_end().to_string()
}

fn fmt_num(value: f64) -> String {
    if value.abs() < 1e-9 {
        return "0".to_string();
    }
    let mut text = format!("{value:.6}");
    while text.contains('.') && text.ends_with('0') {
        text.pop();
    }
    if text.ends_with('.') {
        text.pop();
    }
    text
}

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn transform_scale(transform: &Transform2) -> f64 {
    let sx = (transform.a * transform.a + transform.b * transform.b).sqrt();
    let sy = (transform.c * transform.c + transform.d * transform.d).sqrt();
    f64::midpoint(sx, sy).max(1e-9)
}

/// Apply a transform to a path (helper used by callers that need baked
/// coordinates without exporting).
pub fn bake_transform(path: &Path, transform: &Transform2) -> Path {
    path.transform(transform)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{Object, Shape};

    #[test]
    fn empty_document() {
        let doc = Document::default();
        let svg = export_svg(&doc).unwrap();
        assert!(svg.contains("<svg"));
        assert!(svg.contains("</svg>"));
    }

    #[test]
    fn exports_rectangle_geometry_flipped_for_svg() {
        let mut doc = Document::default();
        let layer_id = doc.layers[0].id.clone();
        let object = Object::new(
            layer_id,
            Shape::Rect {
                width: 10.0,
                height: 5.0,
                corner_radius: 0.0,
            },
        )
        .with_transform(Transform2::translate(2.0, 3.0));
        doc.add_object(object);
        let svg = export_svg(&doc).unwrap();
        // Page height is 200 mm; y=3 becomes y=197 in SVG space.
        assert!(svg.contains("M2 197"));
        assert!(svg.contains("L12 197"));
        assert!(svg.contains('Z'));
    }

    #[test]
    fn escapes_text() {
        let mut doc = Document::default();
        let layer_id = doc.layers[0].id.clone();
        let object = Object::new(
            layer_id,
            Shape::Text(crate::objects::TextObject {
                content: "a<b&c".to_string(),
                font: "sans".to_string(),
                size: 5.0,
                x: 0.0,
                y: 0.0,
            }),
        );
        doc.add_object(object);
        let svg = export_svg(&doc).unwrap();
        assert!(svg.contains("a&lt;b&amp;c"));
    }

    #[test]
    fn hidden_layers_are_not_exported() {
        let mut doc = Document::default();
        doc.layers[0].visible = false;
        let layer_id = doc.layers[0].id.clone();
        doc.add_object(Object::new(
            layer_id,
            Shape::Rect {
                width: 1.0,
                height: 1.0,
                corner_radius: 0.0,
            },
        ));
        let svg = export_svg(&doc).unwrap();
        assert!(!svg.contains("<path"));
    }
}
