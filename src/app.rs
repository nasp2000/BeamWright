#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::too_many_lines)]

use egui::Ui;

use crate::document::{Document, serialization};
use crate::editor::{CanvasState, Selection, Tool};
use crate::history::UndoStack;
use crate::messages::MessageCatalog;
use crate::ui::icons::{Icon, icon_button, icon_label};

#[derive(Default)]
pub struct BeamWrightApp {
    document: Document,
    canvas: CanvasState,
    tool: Tool,
    selection: Selection,
    history: UndoStack,
    messages: MessageCatalog,
    dark_mode: bool,
    save_path: String,
    load_path: String,
    status: String,
}

impl eframe::App for BeamWrightApp {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        if self.dark_mode {
            ui.ctx().set_visuals(egui::Visuals::dark());
        } else {
            ui.ctx().set_visuals(egui::Visuals::light());
        }
        // Keyboard shortcuts
        if ui.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::Z)) {
            self.history.undo();
        }
        if ui.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::Y)) {
            self.history.redo();
        }
        egui::Panel::top("menu_bar").show(ui, |ui| {
            ui.horizontal(|ui| {
                icon_label(ui, Icon::File, "");
                ui.menu_button("File", |ui| {
                    if icon_button(ui, Icon::File, "New").clicked() {
                        self.document = Document::default();
                        self.selection.clear();
                        self.history.clear();
                        self.status = "New document".to_string();
                    }
                    if icon_button(ui, Icon::FolderOpen, "Open").clicked()
                        && !self.load_path.is_empty()
                    {
                        let path = std::path::Path::new(&self.load_path);
                        let res = if path
                            .extension()
                            .and_then(|e| e.to_str())
                            .is_some_and(|e| e.eq_ignore_ascii_case("bwp"))
                        {
                            serialization::load_from_file(path)
                        } else {
                            crate::import::format::import_by_extension(path)
                        };
                        match res {
                            Ok(d) => {
                                self.document = d;
                                self.status = format!("Loaded {}", self.load_path);
                            }
                            Err(e) => self.status = format!("Load error: {e}"),
                        }
                    }
                    if icon_button(ui, Icon::Save, "Save").clicked() && !self.save_path.is_empty() {
                        match serialization::save_to_file(
                            &self.document,
                            std::path::Path::new(&self.save_path),
                        ) {
                            Ok(()) => self.status = format!("Saved to {}", self.save_path),
                            Err(e) => self.status = format!("Save error: {e}"),
                        }
                    }
                });
                // menu title with icon via label before button? Use icon_label in horizontal
                icon_label(ui, Icon::Undo, "");
                ui.menu_button("Edit", |ui| {
                    if icon_button(ui, Icon::Undo, "Undo").clicked() {
                        self.history.undo();
                    }
                    if icon_button(ui, Icon::Redo, "Redo").clicked() {
                        self.history.redo();
                    }
                    let _ = icon_button(ui, Icon::Cut, "Cut");
                    let _ = icon_button(ui, Icon::Copy, "Copy");
                    let _ = icon_button(ui, Icon::Paste, "Paste");
                    let _ = icon_button(ui, Icon::Delete, "Delete");
                });
                icon_label(ui, Icon::Draw, "");
                ui.menu_button("Draw", |ui| {
                    for t in [
                        Tool::Select,
                        Tool::Line,
                        Tool::Rect,
                        Tool::Ellipse,
                        Tool::Pan,
                    ] {
                        if icon_button(ui, icon_for_tool(t), t.label()).clicked() {
                            self.tool = t;
                        }
                    }
                });
                icon_label(ui, Icon::Arrange, "");
                ui.menu_button("Arrange", |_ui| {});
                icon_label(ui, Icon::Operations, "");
                ui.menu_button("Operations", |_ui| {});
                icon_label(ui, Icon::Materials, "");
                ui.menu_button("Materials", |_ui| {});
                icon_label(ui, Icon::View, "");
                ui.menu_button("View", |ui| {
                    if icon_button(ui, Icon::ZoomIn, "Zoom in").clicked() {
                        self.canvas.zoom_in();
                    }
                    if icon_button(ui, Icon::ZoomOut, "Zoom out").clicked() {
                        self.canvas.zoom_out();
                    }
                    if icon_button(ui, Icon::Grid, "Grid").clicked() {
                        self.canvas.grid_visible = !self.canvas.grid_visible;
                    }
                });
                ui.label(format!("Tool: {}", self.tool.label()));
                ui.label(format!("Zoom: {:.2}", self.canvas.zoom));
            });
        });

        // Add icon to menu buttons titles by overriding: draw icons next to text using icon_label in a horizontal before menu_button is not possible easily.
        // Instead, prepend icon glyph text is removed; icons drawn in items only.

        egui::Panel::top("paths").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("Save path:");
                ui.text_edit_singleline(&mut self.save_path);
                ui.label("Load path:");
                ui.text_edit_singleline(&mut self.load_path);
            });
            if !self.status.is_empty() {
                ui.label(&self.status);
            }
            if !self.load_path.is_empty() {
                let fmt =
                    crate::import::format::detect_format(std::path::Path::new(&self.load_path));
                ui.label(format!("Detected format: {fmt}"));
            }
        });

        egui::Panel::top("toolbar").show(ui, |ui| {
            ui.horizontal(|ui| {
                if icon_button(ui, Icon::ZoomIn, "Zoom in").clicked() {
                    self.canvas.zoom_in();
                }
                if icon_button(ui, Icon::ZoomOut, "Zoom out").clicked() {
                    self.canvas.zoom_out();
                }
                if icon_button(ui, Icon::Grid, "Grid").clicked() {
                    self.canvas.grid_visible = !self.canvas.grid_visible;
                }
                if icon_button(ui, Icon::View, "Dark mode").clicked() {
                    self.dark_mode = !self.dark_mode;
                }
                if icon_button(ui, Icon::Undo, "Undo").clicked() {
                    self.history.undo();
                }
                if icon_button(ui, Icon::Redo, "Redo").clicked() {
                    self.history.redo();
                }
            });
        });

        egui::CentralPanel::default().show(ui, |ui| {
            let rect = ui.available_rect_before_wrap();
            let painter = ui.painter_at(rect);
            painter.rect_filled(rect, 0.0, egui::Color32::from_gray(30));

            let response = ui.interact(rect, ui.id().with("canvas"), egui::Sense::click_and_drag());
            if response.hovered() {
                let scroll = ui.input(|i| i.smooth_scroll_delta.y);
                if scroll != 0.0 {
                    if scroll > 0.0 {
                        self.canvas.zoom_in();
                    } else {
                        self.canvas.zoom_out();
                    }
                }
            }
            if response.dragged() {
                let delta = response.drag_delta();
                self.canvas.pan(f64::from(delta.x), f64::from(delta.y));
            }

            let origin = rect.min;
            if self.canvas.grid_visible {
                let spacing = (self.canvas.grid_spacing_mm * self.canvas.zoom) as f32;
                if spacing > 4.0 {
                    let mut x = origin.x + (self.canvas.pan_x as f32).rem_euclid(spacing);
                    while x < rect.max.x {
                        painter.line_segment(
                            [egui::pos2(x, rect.min.y), egui::pos2(x, rect.max.y)],
                            egui::Stroke::new(1.0, egui::Color32::from_gray(45)),
                        );
                        x += spacing;
                    }
                    let mut y = origin.y + (self.canvas.pan_y as f32).rem_euclid(spacing);
                    while y < rect.max.y {
                        painter.line_segment(
                            [egui::pos2(rect.min.x, y), egui::pos2(rect.max.x, y)],
                            egui::Stroke::new(1.0, egui::Color32::from_gray(45)),
                        );
                        y += spacing;
                    }
                }
            }
            let cx = origin.x + self.canvas.pan_x as f32;
            let cy = origin.y + self.canvas.pan_y as f32;
            painter.line_segment(
                [egui::pos2(cx, rect.min.y), egui::pos2(cx, rect.max.y)],
                egui::Stroke::new(1.0, egui::Color32::from_rgb(80, 80, 120)),
            );
            painter.line_segment(
                [egui::pos2(rect.min.x, cy), egui::pos2(rect.max.x, cy)],
                egui::Stroke::new(1.0, egui::Color32::from_rgb(80, 80, 120)),
            );

            ui.label(format!(
                "Doc: {}  {}x{} {}  |  Layers: {}  Objects: {}  |  Selection: {}  |  Undo: {}  Redo: {}",
                self.document.name,
                self.document.width_mm,
                self.document.height_mm,
                self.document.unit.label(),
                self.document.layers.len(),
                self.document.objects.len(),
                self.selection.len(),
                self.history.undo_len(),
                self.history.redo_len()
            ));
        });
    }
}

fn icon_for_tool(t: Tool) -> Icon {
    match t {
        Tool::Select => Icon::Select,
        Tool::Line => Icon::Line,
        Tool::Rect => Icon::Rect,
        Tool::Ellipse => Icon::Ellipse,
        Tool::Pan => Icon::Pan,
    }
}
