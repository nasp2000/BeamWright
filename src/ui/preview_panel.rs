use crate::preview::PreviewData;
use egui::Ui;

#[allow(clippy::cast_possible_truncation)]
pub fn show_preview(ui: &mut Ui, preview: &PreviewData) {
    let rect = ui.available_rect_before_wrap();
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, egui::Color32::from_gray(20));
    for seg in &preview.segments {
        painter.line_segment(
            [
                egui::pos2(rect.min.x + seg.x1 as f32, rect.min.y + seg.y1 as f32),
                egui::pos2(rect.min.x + seg.x2 as f32, rect.min.y + seg.y2 as f32),
            ],
            egui::Stroke::new(1.0, egui::Color32::from_rgb(0, 200, 100)),
        );
    }
    if preview.is_empty() {
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "No preview",
            egui::FontId::default(),
            egui::Color32::GRAY,
        );
    }
}

#[cfg(test)]
mod tests {
    // no egui tests here (requires context)
}
