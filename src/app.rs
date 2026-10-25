#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::too_many_lines)]

use egui::Ui;

use crate::document::{Document, serialization};
use crate::editor::{CanvasState, Selection, Tool};
use crate::history::UndoStack;
use crate::messages::MessageCatalog;

#[derive(Default)]
pub struct BeamWrightApp {
    document: Document,
    canvas: CanvasState,
    tool: Tool,
    selection: Selection,
    history: UndoStack,
    messages: MessageCatalog,
    // File I/O state
    save_path: String,
    load_path: String,
    status: String,
}

impl eframe::App for BeamWrightApp {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        // Top menu
        egui::Panel::top("menu_bar").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("New").clicked() {
                        self.document = Document::default();
                        self.selection.clear();
                        self.history.clear();
                        self.status = "New document".to_string();
                    }
                    if ui.button("Save").clicked() && !self.save_path.is_empty() {
                        match serialization::save_to_file(
                            &self.document,
                            std::path::Path::new(&self.save_path),
                        ) {
                            Ok(()) => self.status = format!("Saved to {}", self.save_path),
                            Err(e) => self.status = format!("Save error: {e}"),
                        }
                    }
                    if ui.button("Load").clicked() && !self.load_path.is_empty() {
                        match serialization::load_from_file(std::path::Path::new(&self.load_path)) {
                            Ok(d) => {
                                self.document = d;
                                self.status = format!("Loaded {}", self.load_path);
                            }
                            Err(e) => self.status = format!("Load error: {e}"),
                        }
                    }
                });
                ui.menu_button("Edit", |ui| {
                    if ui.button("Undo").clicked() {
                        self.history.undo();
                    }
                    if ui.button("Redo").clicked() {
                        self.history.redo();
                    }
                });
                ui.menu_button("Tools", |ui| {
                    for t in [
                        Tool::Select,
                        Tool::Line,
                        Tool::Rect,
                        Tool::Ellipse,
                        Tool::Pan,
                    ] {
                        if ui.button(t.label()).clicked() {
                            self.tool = t;
                        }
                    }
                });
                ui.label(format!("Tool: {}", self.tool.label()));
                ui.label(format!("Zoom: {:.2}", self.canvas.zoom));
            });
        });

        // Paths panel
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
        });

        // Toolbar
        egui::Panel::top("toolbar").show(ui, |ui| {
            ui.horizontal(|ui| {
                if ui.button("Zoom in").clicked() {
                    self.canvas.zoom_in();
                }
                if ui.button("Zoom out").clicked() {
                    self.canvas.zoom_out();
                }
                if ui.button("Reset view").clicked() {
                    self.canvas.reset_zoom();
                    self.canvas.reset_pan();
                }
                if ui.button("Grid").clicked() {
                    self.canvas.grid_visible = !self.canvas.grid_visible;
                }
            });
        });

        // Central canvas
        egui::CentralPanel::default().show(ui, |ui| {
            let rect = ui.available_rect_before_wrap();
            let painter = ui.painter_at(rect);
            painter.rect_filled(rect, 0.0, egui::Color32::from_gray(30));

            // Mouse handling
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
            // Grid
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
            // Axes
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
