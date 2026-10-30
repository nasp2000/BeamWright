#![allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]

use egui::{Align2, Color32, FontId, Painter, Pos2, Rect, Stroke, Vec2};

use crate::color::parse_hex_color;
use crate::document::{Object, Shape};
use crate::editor::EditorState;
use crate::geometry::{Path, Point2};

/// A shape being dragged out with a drawing tool, in document millimetres.
#[derive(Debug, Clone, Copy)]
pub enum PreviewShape {
    Rect { start: Point2, end: Point2 },
    Ellipse { start: Point2, end: Point2 },
    Line { start: Point2, end: Point2 },
}

#[derive(Debug, Clone, Copy)]
pub struct Theme {
    pub dark: bool,
}

impl Theme {
    pub fn canvas_background(self) -> Color32 {
        if self.dark {
            Color32::from_gray(28)
        } else {
            Color32::from_gray(210)
        }
    }

    pub fn page_background(self) -> Color32 {
        if self.dark {
            Color32::from_gray(45)
        } else {
            Color32::WHITE
        }
    }

    pub fn page_border(self) -> Color32 {
        if self.dark {
            Color32::from_gray(90)
        } else {
            Color32::from_gray(140)
        }
    }

    pub fn grid(self) -> Color32 {
        if self.dark {
            Color32::from_gray(58)
        } else {
            Color32::from_gray(228)
        }
    }

    pub fn ruler(self) -> Color32 {
        if self.dark {
            Color32::from_gray(110)
        } else {
            Color32::from_gray(130)
        }
    }

    pub fn selection(self) -> Color32 {
        Color32::from_rgb(0, 140, 255)
    }

    pub fn preview(self) -> Color32 {
        Color32::from_rgb(0, 170, 90)
    }

    pub fn object_color(self, color: Option<&str>, fallback: Color32) -> Color32 {
        color
            .and_then(parse_hex_color)
            .map_or(fallback, |(r, g, b)| Color32::from_rgb(r, g, b))
    }
}

pub fn screen_pos(rect: Rect, editor: &EditorState, point: Point2) -> Pos2 {
    let (x, y) = editor.canvas.doc_to_screen(point.x, point.y);
    Pos2::new(rect.min.x + x as f32, rect.min.y + y as f32)
}

pub fn paint_canvas(
    painter: &Painter,
    rect: Rect,
    editor: &EditorState,
    theme: Theme,
    preview: Option<PreviewShape>,
) {
    painter.rect_filled(rect, 0.0, theme.canvas_background());
    paint_page(painter, rect, editor, theme);
    if editor.canvas.grid_visible {
        paint_grid(painter, rect, editor, theme);
    }
    paint_objects(painter, rect, editor, theme);
    if let Some(preview) = preview {
        paint_preview(painter, rect, editor, theme, preview);
    }
    paint_selection(painter, rect, editor, theme);
}

fn page_screen_rect(rect: Rect, editor: &EditorState) -> Rect {
    let (x0, y0) = editor.canvas.doc_to_screen(0.0, editor.document.height_mm);
    let (x1, y1) = editor.canvas.doc_to_screen(editor.document.width_mm, 0.0);
    Rect::from_min_max(
        Pos2::new(rect.min.x + x0 as f32, rect.min.y + y0 as f32),
        Pos2::new(rect.min.x + x1 as f32, rect.min.y + y1 as f32),
    )
}

fn paint_page(painter: &Painter, rect: Rect, editor: &EditorState, theme: Theme) {
    let page = page_screen_rect(rect, editor).intersect(rect);
    if page.is_positive() {
        painter.rect_filled(page, 0.0, theme.page_background());
        painter.rect_stroke(
            page,
            0.0,
            Stroke::new(1.0, theme.page_border()),
            egui::StrokeKind::Inside,
        );
    }
}

fn paint_grid(painter: &Painter, rect: Rect, editor: &EditorState, theme: Theme) {
    let spacing = editor.canvas.effective_grid_spacing();
    if spacing <= 0.0 {
        return;
    }
    let page = page_screen_rect(rect, editor);
    let page = page.intersect(rect);
    if !page.is_positive() {
        return;
    }
    let stroke = Stroke::new(1.0, theme.grid());
    let mut x = 0.0;
    while x <= editor.document.width_mm + 1e-9 {
        let (sx, _) = editor.canvas.doc_to_screen(x, 0.0);
        let sx = rect.min.x + sx as f32;
        if sx >= page.min.x && sx <= page.max.x {
            painter.line_segment(
                [Pos2::new(sx, page.min.y), Pos2::new(sx, page.max.y)],
                stroke,
            );
        }
        x += spacing;
    }
    let mut y = 0.0;
    while y <= editor.document.height_mm + 1e-9 {
        let (_, sy) = editor.canvas.doc_to_screen(0.0, y);
        let sy = rect.min.y + sy as f32;
        if sy >= page.min.y && sy <= page.max.y {
            painter.line_segment(
                [Pos2::new(page.min.x, sy), Pos2::new(page.max.x, sy)],
                stroke,
            );
        }
        y += spacing;
    }
}

fn paint_objects(painter: &Painter, rect: Rect, editor: &EditorState, theme: Theme) {
    let document = &editor.document;
    for layer in &document.layers {
        if !layer.visible {
            continue;
        }
        let layer_color = Color32::from_rgb(layer.rgb().0, layer.rgb().1, layer.rgb().2);
        for object in document
            .objects
            .iter()
            .filter(|object| object.layer_id == layer.id && object.visible)
        {
            paint_object(painter, rect, editor, theme, object, layer_color);
        }
    }
}

fn paint_object(
    painter: &Painter,
    rect: Rect,
    editor: &EditorState,
    theme: Theme,
    object: &Object,
    layer_color: Color32,
) {
    let stroke_color = theme.object_color(object.style.stroke.as_deref(), layer_color);
    let width = (object.style.stroke_width_mm * editor.canvas.zoom) as f32;
    let stroke = Stroke::new(width.max(1.0), stroke_color);
    let fill_color = object
        .style
        .fill
        .as_deref()
        .and_then(parse_hex_color)
        .map(|(r, g, b)| Color32::from_rgb(r, g, b));

    match &object.shape {
        Shape::Text(text) => {
            let world = object.transform.apply(Point2::new(text.x, text.y));
            let scale = f64::midpoint(
                object.transform.a.hypot(object.transform.b),
                object.transform.c.hypot(object.transform.d),
            );
            let size = (text.size * scale * editor.canvas.zoom) as f32;
            if size >= 3.0 {
                painter.text(
                    screen_pos(rect, editor, world),
                    Align2::LEFT_BOTTOM,
                    &text.content,
                    FontId::proportional(size),
                    fill_color.unwrap_or(stroke_color),
                );
            }
        }
        Shape::Image(image) => {
            let Some((min, max)) = object.bounds() else {
                return;
            };
            let p0 = screen_pos(rect, editor, Point2::new(min.x, max.y));
            let p1 = screen_pos(rect, editor, Point2::new(max.x, min.y));
            let image_rect = Rect::from_two_pos(p0, p1);
            painter.rect_stroke(
                image_rect,
                0.0,
                Stroke::new(1.0, stroke_color),
                egui::StrokeKind::Inside,
            );
            painter.line_segment(
                [image_rect.min, image_rect.max],
                Stroke::new(1.0, stroke_color),
            );
            painter.line_segment(
                [
                    Pos2::new(image_rect.max.x, image_rect.min.y),
                    Pos2::new(image_rect.min.x, image_rect.max.y),
                ],
                Stroke::new(1.0, stroke_color),
            );
            let _ = image;
        }
        _ => {
            if let Some((min, max)) = object.bounds() {
                if let Some(fill) = fill_color {
                    if is_axis_aligned(&object.transform) {
                        let p0 = screen_pos(rect, editor, Point2::new(min.x, max.y));
                        let p1 = screen_pos(rect, editor, Point2::new(max.x, min.y));
                        let fill_rect = Rect::from_two_pos(p0, p1);
                        if matches!(object.shape, Shape::Rect { .. }) {
                            painter.rect_filled(fill_rect, 0.0, fill);
                        } else if matches!(object.shape, Shape::Ellipse { .. }) {
                            painter.add(egui::Shape::ellipse_filled(
                                fill_rect.center(),
                                fill_rect.size() / 2.0,
                                fill,
                            ));
                        }
                    }
                }
            }
            let path = object.world_path();
            paint_path(painter, rect, editor, &path, stroke);
        }
    }
}

fn is_axis_aligned(transform: &crate::geometry::Transform2) -> bool {
    transform.b.abs() < 1e-9 && transform.c.abs() < 1e-9
}

fn paint_path(painter: &Painter, rect: Rect, editor: &EditorState, path: &Path, stroke: Stroke) {
    let points = path.flatten(0.15);
    if points.len() < 2 {
        return;
    }
    let screen: Vec<Pos2> = points
        .iter()
        .map(|point| screen_pos(rect, editor, *point))
        .collect();
    if path.closed {
        painter.add(egui::Shape::closed_line(screen, stroke));
    } else {
        painter.add(egui::Shape::line(screen, stroke));
    }
}

fn paint_selection(painter: &Painter, rect: Rect, editor: &EditorState, theme: Theme) {
    if editor.selection.is_empty() {
        return;
    }
    let color = theme.selection();
    let stroke = Stroke::new(1.5, color);
    if let Some((min, max)) = editor.selected_bounds() {
        let p0 = screen_pos(rect, editor, Point2::new(min.x, max.y));
        let p1 = screen_pos(rect, editor, Point2::new(max.x, min.y));
        let bounds = Rect::from_two_pos(p0, p1).expand(2.0);
        painter.rect_stroke(bounds, 0.0, stroke, egui::StrokeKind::Outside);
        for corner in [
            bounds.left_top(),
            bounds.right_top(),
            bounds.right_bottom(),
            bounds.left_bottom(),
        ] {
            painter.rect_filled(Rect::from_center_size(corner, Vec2::splat(6.0)), 1.0, color);
        }
    }
    // Show every selected object's outline, not just the group bounds.
    for id in editor.selection.ids() {
        let Some(object) = editor.document.object(id) else {
            continue;
        };
        if matches!(object.shape, Shape::Text(_) | Shape::Image(_)) {
            if let Some((min, max)) = object.bounds() {
                let p0 = screen_pos(rect, editor, Point2::new(min.x, max.y));
                let p1 = screen_pos(rect, editor, Point2::new(max.x, min.y));
                painter.rect_stroke(
                    Rect::from_two_pos(p0, p1),
                    0.0,
                    Stroke::new(1.0, color),
                    egui::StrokeKind::Outside,
                );
            }
        } else {
            paint_path(
                painter,
                rect,
                editor,
                &object.world_path(),
                Stroke::new(1.0, color),
            );
        }
    }
}

fn paint_preview(
    painter: &Painter,
    rect: Rect,
    editor: &EditorState,
    theme: Theme,
    preview: PreviewShape,
) {
    let stroke = Stroke::new(1.5, theme.preview());
    match preview {
        PreviewShape::Rect { start, end } | PreviewShape::Ellipse { start, end } => {
            let p0 = screen_pos(
                rect,
                editor,
                Point2::new(start.x.min(end.x), start.y.max(end.y)),
            );
            let p1 = screen_pos(
                rect,
                editor,
                Point2::new(start.x.max(end.x), start.y.min(end.y)),
            );
            let preview_rect = Rect::from_two_pos(p0, p1);
            if matches!(preview, PreviewShape::Rect { .. }) {
                painter.rect_stroke(preview_rect, 0.0, stroke, egui::StrokeKind::Middle);
            } else {
                painter.add(egui::Shape::ellipse_stroke(
                    preview_rect.center(),
                    preview_rect.size() / 2.0,
                    stroke,
                ));
            }
        }
        PreviewShape::Line { start, end } => {
            painter.line_segment(
                [
                    screen_pos(rect, editor, start),
                    screen_pos(rect, editor, end),
                ],
                stroke,
            );
        }
    }
}

/// Toolbar colour swatch helper.
pub fn color_swatch(ui: &mut egui::Ui, color: Color32) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(18.0), egui::Sense::hover());
    ui.painter().rect_filled(rect, 2.0, color);
    ui.painter().rect_stroke(
        rect,
        2.0,
        Stroke::new(1.0, Color32::from_gray(100)),
        egui::StrokeKind::Inside,
    );
    response
}

/// Cursor label in millimetres for the status bar.
pub fn format_mm(value: f64) -> String {
    if value.abs() >= 100.0 {
        format!("{value:.1}")
    } else {
        format!("{value:.2}")
    }
}
