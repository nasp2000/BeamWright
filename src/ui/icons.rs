use egui::Ui;

/// Icon set rendered with egui's painter (vector shapes, consistent across platforms).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    File,
    FolderOpen,
    Save,
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    Delete,
    Draw,
    Arrange,
    Operations,
    Materials,
    View,
    ZoomIn,
    ZoomOut,
    Grid,
    Select,
    Line,
    Rect,
    Ellipse,
    Pan,
}

impl Icon {
    pub fn label(self) -> &'static str {
        match self {
            Icon::File => "File",
            Icon::FolderOpen => "Open",
            Icon::Save => "Save",
            Icon::Undo => "Undo",
            Icon::Redo => "Redo",
            Icon::Cut => "Cut",
            Icon::Copy => "Copy",
            Icon::Paste => "Paste",
            Icon::Delete => "Delete",
            Icon::Draw => "Draw",
            Icon::Arrange => "Arrange",
            Icon::Operations => "Operations",
            Icon::Materials => "Materials",
            Icon::View => "View",
            Icon::ZoomIn => "Zoom in",
            Icon::ZoomOut => "Zoom out",
            Icon::Grid => "Grid",
            Icon::Select => "Select",
            Icon::Line => "Line",
            Icon::Rect => "Rectangle",
            Icon::Ellipse => "Ellipse",
            Icon::Pan => "Pan",
        }
    }

    #[allow(clippy::too_many_lines)]
    #[allow(clippy::cast_precision_loss)]
    /// Draw this icon into `rect` using `painter`.
    pub fn paint(self, painter: &egui::Painter, rect: egui::Rect, color: egui::Color32) {
        let s = rect.width().min(rect.height()) * 0.85;
        let c = rect.center();
        let sw = (s / 12.0).max(1.0);
        let stroke = egui::Stroke::new(sw, color);
        match self {
            Icon::File => {
                let r = egui::Rect::from_center_size(c, egui::vec2(s * 0.7, s));
                painter.rect_stroke(r, 1.0, stroke, egui::StrokeKind::Outside);
                // folded corner
                let tr = r.right_top();
                painter.line_segment([tr, egui::pos2(tr.x - s * 0.25, tr.y + s * 0.25)], stroke);
                painter.line_segment(
                    [
                        egui::pos2(tr.x - s * 0.25, tr.y + s * 0.25),
                        egui::pos2(tr.x - s * 0.25, tr.y),
                    ],
                    stroke,
                );
            }
            Icon::FolderOpen => {
                let r = egui::Rect::from_center_size(c, egui::vec2(s, s * 0.7));
                painter.rect_stroke(r, 2.0, stroke, egui::StrokeKind::Outside);
                painter.line_segment(
                    [
                        egui::pos2(r.left() + s * 0.1, r.top()),
                        egui::pos2(r.left() + s * 0.4, r.top() - s * 0.2),
                    ],
                    stroke,
                );
                painter.line_segment(
                    [
                        egui::pos2(r.left() + s * 0.4, r.top() - s * 0.2),
                        egui::pos2(r.right() - s * 0.1, r.top() - s * 0.2),
                    ],
                    stroke,
                );
            }
            Icon::Save => {
                let r = egui::Rect::from_center_size(c, egui::vec2(s * 0.9, s * 0.9));
                painter.rect_stroke(r, 1.0, stroke, egui::StrokeKind::Outside);
                let inner = egui::Rect::from_center_size(c, egui::vec2(s * 0.4, s * 0.3));
                painter.rect_stroke(inner, 0.0, stroke, egui::StrokeKind::Outside);
            }
            Icon::Undo => {
                draw_arc(
                    painter,
                    c,
                    s * 0.4,
                    std::f32::consts::PI * 0.25,
                    std::f32::consts::PI * 1.75,
                    sw,
                    color,
                );
                // arrow head
                let tip = egui::pos2(c.x - s * 0.35, c.y - s * 0.15);
                painter.line_segment([tip, egui::pos2(tip.x + s * 0.15, tip.y - s * 0.1)], stroke);
                painter.line_segment([tip, egui::pos2(tip.x + s * 0.1, tip.y + s * 0.15)], stroke);
            }
            Icon::Redo => {
                draw_arc(
                    painter,
                    c,
                    s * 0.4,
                    std::f32::consts::PI * 0.75,
                    std::f32::consts::PI * 2.25,
                    sw,
                    color,
                );
                let tip = egui::pos2(c.x + s * 0.35, c.y - s * 0.15);
                painter.line_segment([tip, egui::pos2(tip.x - s * 0.15, tip.y - s * 0.1)], stroke);
                painter.line_segment([tip, egui::pos2(tip.x - s * 0.1, tip.y + s * 0.15)], stroke);
            }
            Icon::Cut => {
                // two blades
                painter.line_segment(
                    [
                        egui::pos2(c.x - s * 0.4, c.y - s * 0.4),
                        egui::pos2(c.x + s * 0.4, c.y + s * 0.1),
                    ],
                    stroke,
                );
                painter.line_segment(
                    [
                        egui::pos2(c.x - s * 0.4, c.y + s * 0.4),
                        egui::pos2(c.x + s * 0.4, c.y - s * 0.1),
                    ],
                    stroke,
                );
                // handles
                painter.circle_stroke(egui::pos2(c.x - s * 0.35, c.y - s * 0.35), s * 0.12, stroke);
                painter.circle_stroke(egui::pos2(c.x - s * 0.35, c.y + s * 0.35), s * 0.12, stroke);
            }
            Icon::Copy => {
                let r1 = egui::Rect::from_center_size(
                    egui::pos2(c.x - s * 0.1, c.y - s * 0.1),
                    egui::vec2(s * 0.6, s * 0.6),
                );
                let r2 = egui::Rect::from_center_size(
                    egui::pos2(c.x + s * 0.1, c.y + s * 0.1),
                    egui::vec2(s * 0.6, s * 0.6),
                );
                painter.rect_stroke(r1, 1.0, stroke, egui::StrokeKind::Outside);
                painter.rect_stroke(r2, 1.0, stroke, egui::StrokeKind::Outside);
            }
            Icon::Paste => {
                let r = egui::Rect::from_center_size(c, egui::vec2(s * 0.7, s * 0.8));
                painter.rect_stroke(r, 1.0, stroke, egui::StrokeKind::Outside);
                let clip = egui::Rect::from_center_size(
                    egui::pos2(c.x, r.top() - s * 0.05),
                    egui::vec2(s * 0.3, s * 0.15),
                );
                painter.rect_stroke(clip, 1.0, stroke, egui::StrokeKind::Outside);
            }
            Icon::Delete => {
                // trash can
                let r = egui::Rect::from_center_size(
                    egui::pos2(c.x, c.y + s * 0.05),
                    egui::vec2(s * 0.6, s * 0.65),
                );
                painter.rect_stroke(r, 1.0, stroke, egui::StrokeKind::Outside);
                painter.line_segment(
                    [
                        egui::pos2(r.left(), r.top() - s * 0.15),
                        egui::pos2(r.right(), r.top() - s * 0.15),
                    ],
                    stroke,
                );
                painter.line_segment(
                    [
                        egui::pos2(c.x - s * 0.15, r.top() - s * 0.25),
                        egui::pos2(c.x + s * 0.15, r.top() - s * 0.25),
                    ],
                    stroke,
                );
            }
            Icon::Draw => {
                painter.line_segment(
                    [
                        egui::pos2(c.x - s * 0.35, c.y + s * 0.35),
                        egui::pos2(c.x + s * 0.35, c.y - s * 0.35),
                    ],
                    stroke,
                );
                painter.circle_filled(egui::pos2(c.x + s * 0.35, c.y - s * 0.35), sw * 1.5, color);
            }
            Icon::Arrange => {
                painter.rect_stroke(
                    egui::Rect::from_center_size(
                        egui::pos2(c.x - s * 0.15, c.y - s * 0.15),
                        egui::vec2(s * 0.4, s * 0.4),
                    ),
                    0.0,
                    stroke,
                    egui::StrokeKind::Outside,
                );
                painter.rect_stroke(
                    egui::Rect::from_center_size(
                        egui::pos2(c.x + s * 0.15, c.y + s * 0.15),
                        egui::vec2(s * 0.4, s * 0.4),
                    ),
                    0.0,
                    stroke,
                    egui::StrokeKind::Outside,
                );
            }
            Icon::Operations => {
                painter.circle_stroke(c, s * 0.35, stroke);
                for i in 0..6 {
                    let a = i as f32 * std::f32::consts::PI / 3.0;
                    let p1 = egui::pos2(c.x + a.cos() * s * 0.35, c.y + a.sin() * s * 0.35);
                    let p2 = egui::pos2(c.x + a.cos() * s * 0.48, c.y + a.sin() * s * 0.48);
                    painter.line_segment([p1, p2], stroke);
                }
            }
            Icon::Materials => {
                let r = egui::Rect::from_center_size(c, egui::vec2(s * 0.8, s * 0.6));
                painter.rect_stroke(r, 2.0, stroke, egui::StrokeKind::Outside);
                painter.line_segment(
                    [
                        egui::pos2(r.left() + s * 0.1, r.center().y),
                        egui::pos2(r.right() - s * 0.1, r.center().y),
                    ],
                    stroke,
                );
            }
            Icon::View => {
                painter.circle_stroke(c, s * 0.25, stroke);
                // eye shape
                painter.line_segment(
                    [
                        egui::pos2(c.x - s * 0.45, c.y),
                        egui::pos2(c.x, c.y - s * 0.3),
                    ],
                    stroke,
                );
                painter.line_segment(
                    [
                        egui::pos2(c.x, c.y - s * 0.3),
                        egui::pos2(c.x + s * 0.45, c.y),
                    ],
                    stroke,
                );
                painter.line_segment(
                    [
                        egui::pos2(c.x + s * 0.45, c.y),
                        egui::pos2(c.x, c.y + s * 0.3),
                    ],
                    stroke,
                );
                painter.line_segment(
                    [
                        egui::pos2(c.x, c.y + s * 0.3),
                        egui::pos2(c.x - s * 0.45, c.y),
                    ],
                    stroke,
                );
            }
            Icon::ZoomIn => {
                painter.circle_stroke(egui::pos2(c.x - s * 0.1, c.y - s * 0.1), s * 0.3, stroke);
                painter.line_segment(
                    [
                        egui::pos2(c.x + s * 0.15, c.y + s * 0.15),
                        egui::pos2(c.x + s * 0.4, c.y + s * 0.4),
                    ],
                    stroke,
                );
                painter.line_segment(
                    [
                        egui::pos2(c.x - s * 0.1, c.y - s * 0.25),
                        egui::pos2(c.x - s * 0.1, c.y + s * 0.05),
                    ],
                    stroke,
                );
                painter.line_segment(
                    [
                        egui::pos2(c.x - s * 0.25, c.y - s * 0.1),
                        egui::pos2(c.x + s * 0.05, c.y - s * 0.1),
                    ],
                    stroke,
                );
            }
            Icon::ZoomOut => {
                painter.circle_stroke(egui::pos2(c.x - s * 0.1, c.y - s * 0.1), s * 0.3, stroke);
                painter.line_segment(
                    [
                        egui::pos2(c.x + s * 0.15, c.y + s * 0.15),
                        egui::pos2(c.x + s * 0.4, c.y + s * 0.4),
                    ],
                    stroke,
                );
                painter.line_segment(
                    [
                        egui::pos2(c.x - s * 0.25, c.y - s * 0.1),
                        egui::pos2(c.x + s * 0.05, c.y - s * 0.1),
                    ],
                    stroke,
                );
            }
            Icon::Grid => {
                for i in 0..4 {
                    let x = c.x - s * 0.4 + i as f32 * s * 0.27;
                    painter.line_segment(
                        [egui::pos2(x, c.y - s * 0.4), egui::pos2(x, c.y + s * 0.4)],
                        stroke,
                    );
                }
                for i in 0..4 {
                    let y = c.y - s * 0.4 + i as f32 * s * 0.27;
                    painter.line_segment(
                        [egui::pos2(c.x - s * 0.4, y), egui::pos2(c.x + s * 0.4, y)],
                        stroke,
                    );
                }
            }
            Icon::Select => {
                // arrow cursor
                painter.line_segment(
                    [
                        egui::pos2(c.x - s * 0.3, c.y - s * 0.4),
                        egui::pos2(c.x - s * 0.3, c.y + s * 0.3),
                    ],
                    stroke,
                );
                painter.line_segment(
                    [
                        egui::pos2(c.x - s * 0.3, c.y - s * 0.4),
                        egui::pos2(c.x + s * 0.2, c.y - s * 0.1),
                    ],
                    stroke,
                );
                painter.line_segment(
                    [
                        egui::pos2(c.x + s * 0.2, c.y - s * 0.1),
                        egui::pos2(c.x - s * 0.05, c.y - s * 0.05),
                    ],
                    stroke,
                );
                painter.line_segment(
                    [
                        egui::pos2(c.x - s * 0.05, c.y - s * 0.05),
                        egui::pos2(c.x - s * 0.3, c.y + s * 0.3),
                    ],
                    stroke,
                );
            }
            Icon::Line => {
                painter.line_segment(
                    [
                        egui::pos2(c.x - s * 0.4, c.y + s * 0.3),
                        egui::pos2(c.x + s * 0.4, c.y - s * 0.3),
                    ],
                    stroke,
                );
            }
            Icon::Rect => {
                painter.rect_stroke(
                    egui::Rect::from_center_size(c, egui::vec2(s * 0.7, s * 0.5)),
                    1.0,
                    stroke,
                    egui::StrokeKind::Outside,
                );
            }
            Icon::Ellipse => {
                draw_ellipse(painter, c, s * 0.4, s * 0.25, sw, color);
            }
            Icon::Pan => {
                // hand-ish: open circle with arrows
                painter.circle_stroke(c, s * 0.25, stroke);
                for a in [
                    0.0f32,
                    std::f32::consts::FRAC_PI_2,
                    std::f32::consts::PI,
                    std::f32::consts::PI + std::f32::consts::FRAC_PI_2,
                ] {
                    let p = egui::pos2(c.x + a.cos() * s * 0.4, c.y + a.sin() * s * 0.4);
                    painter.circle_filled(p, sw * 1.2, color);
                }
            }
        }
    }
}

/// Clickable button with an icon and a text label.
pub fn icon_button(ui: &mut Ui, icon: Icon, label: &str) -> egui::Response {
    let icon_size = 14.0f32;
    let spacing = 6.0f32;
    let text_color = ui.visuals().text_color();
    let font = egui::FontId::default();
    let galley = ui.fonts_mut(|f| f.layout_no_wrap(label.to_string(), font, text_color));
    let text_w = galley.size().x;
    let text_h = galley.size().y;
    let total_w = icon_size + spacing + text_w;
    let h = text_h.max(icon_size);
    let (rect, response) = ui.allocate_exact_size(egui::vec2(total_w, h), egui::Sense::click());
    if ui.is_rect_visible(rect) {
        let painter = ui.painter_at(rect);
        if response.hovered() {
            painter.rect_filled(rect, 2.0, ui.visuals().widgets.hovered.bg_fill);
        }
        let ic_rect = egui::Rect::from_min_size(
            egui::pos2(rect.min.x, rect.center().y - icon_size * 0.5),
            egui::vec2(icon_size, icon_size),
        );
        icon.paint(&painter, ic_rect, text_color);
        let text_pos = egui::pos2(
            rect.min.x + icon_size + spacing,
            rect.center().y - text_h * 0.5,
        );
        painter.galley(text_pos, galley, text_color);
    }
    response
}

/// Menu title with icon (non-clickable header style).
pub fn icon_label(ui: &mut Ui, icon: Icon, label: &str) {
    let icon_size = 14.0f32;
    let spacing = 6.0f32;
    let text_color = ui.visuals().text_color();
    let font = egui::FontId::default();
    let galley = ui.fonts_mut(|f| f.layout_no_wrap(label.to_string(), font, text_color));
    let text_w = galley.size().x;
    let text_h = galley.size().y;
    let total_w = if label.is_empty() {
        icon_size
    } else {
        icon_size + spacing + text_w
    };
    let h = text_h.max(icon_size);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(total_w, h), egui::Sense::hover());
    if ui.is_rect_visible(rect) {
        let painter = ui.painter_at(rect);
        let ic_rect = egui::Rect::from_min_size(
            egui::pos2(rect.min.x, rect.center().y - icon_size * 0.5),
            egui::vec2(icon_size, icon_size),
        );
        icon.paint(&painter, ic_rect, text_color);
        if !label.is_empty() {
            let text_pos = egui::pos2(
                rect.min.x + icon_size + spacing,
                rect.center().y - text_h * 0.5,
            );
            painter.galley(text_pos, galley, text_color);
        }
    }
}

#[allow(clippy::cast_precision_loss)]
fn draw_arc(
    painter: &egui::Painter,
    center: egui::Pos2,
    radius: f32,
    start: f32,
    end: f32,
    width: f32,
    color: egui::Color32,
) {
    let steps = 24;
    let mut prev: Option<egui::Pos2> = None;
    for i in 0..=steps {
        let t = start + (end - start) * (i as f32 / steps as f32);
        let pp = egui::pos2(center.x + t.cos() * radius, center.y + t.sin() * radius);
        if let Some(prev_p) = prev {
            painter.line_segment([prev_p, pp], egui::Stroke::new(width, color));
        }
        prev = Some(pp);
    }
}

#[allow(clippy::cast_precision_loss)]
fn draw_ellipse(
    painter: &egui::Painter,
    center: egui::Pos2,
    rx: f32,
    ry: f32,
    width: f32,
    color: egui::Color32,
) {
    let steps = 32;
    let mut prev: Option<egui::Pos2> = None;
    for i in 0..=steps {
        let t = i as f32 / steps as f32 * std::f32::consts::TAU;
        let pp = egui::pos2(center.x + t.cos() * rx, center.y + t.sin() * ry);
        if let Some(prev_p) = prev {
            painter.line_segment([prev_p, pp], egui::Stroke::new(width, color));
        }
        prev = Some(pp);
    }
}

#[cfg(test)]
mod tests {
    use super::Icon;

    #[test]
    fn labels_non_empty() {
        for i in [Icon::File, Icon::Save, Icon::Undo, Icon::Redo] {
            assert_ne!(i.label(), "");
        }
    }

    #[test]
    fn equality() {
        assert_eq!(Icon::File, Icon::File);
        assert_ne!(Icon::File, Icon::Save);
    }
}
