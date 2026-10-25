use crate::document::Document;
use crate::document::model::{DocumentObject, ObjectKind};
use crate::errors::Result;

/// Minimal SVG importer (Phase 6, P0). Parses a subset of SVG: paths, rects,
/// circles, ellipses, lines, polygons, polylines. Returns a Document.
#[allow(clippy::unnecessary_wraps)]
#[allow(clippy::too_many_lines)]
pub fn import_svg(src: &str) -> Result<Document> {
    let mut doc = Document::default();
    // width/height from svg root if present
    if let Some(w) = attr(src, "width") {
        if let Ok(v) = w.trim_end_matches("px").parse::<f64>() {
            doc.width_mm = v;
        }
    }
    if let Some(h) = attr(src, "height") {
        if let Ok(v) = h.trim_end_matches("px").parse::<f64>() {
            doc.height_mm = v;
        }
    }
    let layer_id = doc.layers[0].id.clone();
    if layer_id.is_empty() {
        doc.layers[0].id = "layer-1".to_string();
    }
    let lid = doc.layers[0].id.clone();

    // rects
    for tag in find_tags(src, "rect") {
        if let (Some(x), Some(y), Some(w), Some(h)) = (
            attr(&tag, "x").and_then(|v| v.parse::<f64>().ok()),
            attr(&tag, "y").and_then(|v| v.parse::<f64>().ok()),
            attr(&tag, "width").and_then(|v| v.parse::<f64>().ok()),
            attr(&tag, "height").and_then(|v| v.parse::<f64>().ok()),
        ) {
            doc.objects.push(DocumentObject {
                id: format!("obj-{}", doc.objects.len()),
                kind: ObjectKind::Rectangle,
                layer_id: lid.clone(),
                operation_id: None,
            });
            let _ = (x, y, w, h);
        }
    }
    // circles
    for tag in find_tags(src, "circle") {
        if attr(&tag, "cx").is_some() {
            doc.objects.push(DocumentObject {
                id: format!("obj-{}", doc.objects.len()),
                kind: ObjectKind::Ellipse,
                layer_id: lid.clone(),
                operation_id: None,
            });
        }
    }
    // ellipses
    for tag in find_tags(src, "ellipse") {
        if attr(&tag, "cx").is_some() {
            doc.objects.push(DocumentObject {
                id: format!("obj-{}", doc.objects.len()),
                kind: ObjectKind::Ellipse,
                layer_id: lid.clone(),
                operation_id: None,
            });
        }
    }
    // lines
    for tag in find_tags(src, "line") {
        if attr(&tag, "x1").is_some() {
            doc.objects.push(DocumentObject {
                id: format!("obj-{}", doc.objects.len()),
                kind: ObjectKind::Line,
                layer_id: lid.clone(),
                operation_id: None,
            });
        }
    }
    // polygons / polylines
    for tag in find_tags(src, "polygon")
        .into_iter()
        .chain(find_tags(src, "polyline"))
    {
        if attr(&tag, "points").is_some() {
            doc.objects.push(DocumentObject {
                id: format!("obj-{}", doc.objects.len()),
                kind: ObjectKind::Path,
                layer_id: lid.clone(),
                operation_id: None,
            });
        }
    }
    // paths
    for tag in find_tags(src, "path") {
        if attr(&tag, "d").is_some() {
            doc.objects.push(DocumentObject {
                id: format!("obj-{}", doc.objects.len()),
                kind: ObjectKind::Path,
                layer_id: lid.clone(),
                operation_id: None,
            });
        }
    }
    // text
    for _tag in find_tags(src, "text") {
        doc.objects.push(DocumentObject {
            id: format!("obj-{}", doc.objects.len()),
            kind: ObjectKind::Text,
            layer_id: lid.clone(),
            operation_id: None,
        });
    }
    // images
    for tag in find_tags(src, "image") {
        if attr(&tag, "href").is_some() || attr(&tag, "xlink:href").is_some() {
            doc.objects.push(DocumentObject {
                id: format!("obj-{}", doc.objects.len()),
                kind: ObjectKind::Image,
                layer_id: lid.clone(),
                operation_id: None,
            });
        }
    }
    // groups
    for _ in find_tags(src, "g") {
        // groups are flattened in this minimal importer; count as no object
    }
    if doc.layers[0].id.is_empty() {
        doc.layers[0].id = "layer-1".to_string();
    }
    Ok(doc)
}

fn attr(tag: &str, name: &str) -> Option<String> {
    // match name="value" or name='value'
    let needle = format!("{name}=");
    let idx = tag.find(&needle)?;
    let rest = &tag[idx + needle.len()..];
    let quote = rest.chars().next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }
    let end = rest[1..].find(quote)?;
    Some(rest[1..=end].to_string())
}

/// Parse SVG path `d` attribute into a list of (command, coords).
pub fn parse_path_commands(d: &str) -> Vec<(char, Vec<f64>)> {
    let mut out: Vec<(char, Vec<f64>)> = Vec::new();
    let mut current: Option<char> = None;
    let mut nums: Vec<f64> = Vec::new();
    let mut buf = String::new();
    let flush_num = |buf: &mut String, nums: &mut Vec<f64>| {
        if !buf.is_empty() {
            if let Ok(v) = buf.trim().parse::<f64>() {
                nums.push(v);
            }
            buf.clear();
        }
    };
    for c in d.chars() {
        if c.is_ascii_alphabetic() {
            flush_num(&mut buf, &mut nums);
            if let Some(cmd) = current {
                out.push((cmd, std::mem::take(&mut nums)));
            }
            current = Some(c);
        } else if c == ',' || c.is_whitespace() {
            flush_num(&mut buf, &mut nums);
        } else {
            buf.push(c);
        }
    }
    flush_num(&mut buf, &mut nums);
    if let Some(cmd) = current {
        out.push((cmd, nums));
    }
    out
}

fn find_tags(src: &str, name: &str) -> Vec<String> {
    let mut out = Vec::new();
    let open = format!("<{name}");
    let mut search = src;
    while let Some(i) = search.find(&open) {
        let rest = &search[i..];
        // find end of tag
        if let Some(end) = rest.find('>') {
            out.push(rest[..=end].to_string());
            search = &rest[end + 1..];
        } else {
            break;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_svg() {
        let d = import_svg("<svg></svg>").unwrap();
        assert!(d.objects.is_empty());
    }

    #[test]
    fn parse_rect() {
        let svg = r#"<svg width="100" height="50"><rect x="0" y="0" width="10" height="5"/></svg>"#;
        let d = import_svg(svg).unwrap();
        assert_eq!(d.objects.len(), 1);
        assert_eq!(d.width_mm, 100.0);
        assert_eq!(d.height_mm, 50.0);
    }

    #[test]
    fn parse_multiple() {
        let svg = r#"<svg><rect x="0" y="0" width="10" height="5"/><circle cx="5" cy="5" r="2"/><path d="M0 0 L10 10"/></svg>"#;
        let d = import_svg(svg).unwrap();
        assert_eq!(d.objects.len(), 3);
    }

    #[test]
    fn invalid_returns_ok_empty() {
        // minimal importer never fails on structure; empty input
        let d = import_svg("").unwrap();
        assert!(d.objects.is_empty());
    }

    #[test]
    fn parse_path_move_line() {
        let cmds = parse_path_commands("M 0 0 L 10 10");
        assert_eq!(cmds.len(), 2);
        assert_eq!(cmds[0].0, 'M');
        assert_eq!(cmds[0].1, vec![0.0, 0.0]);
        assert_eq!(cmds[1].0, 'L');
        assert_eq!(cmds[1].1, vec![10.0, 10.0]);
    }

    #[test]
    fn parse_path_cubic() {
        let cmds = parse_path_commands("M0 0 C 1 2 3 4 5 6");
        assert_eq!(cmds.len(), 2);
        assert_eq!(cmds[1].0, 'C');
        assert_eq!(cmds[1].1.len(), 6);
    }

    #[test]
    fn parse_path_empty() {
        let cmds = parse_path_commands("");
        assert_eq!(cmds, [] as [(char, Vec<f64>); 0]);
    }
}
