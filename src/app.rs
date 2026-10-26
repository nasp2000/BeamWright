#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::too_many_lines)]

use egui::Ui;

use crate::document::{Document, serialization};
use crate::editor::{CanvasState, Selection, Tool};
use crate::history::UndoStack;
use crate::messages::MessageCatalog;
use crate::ui::icons::{Icon, icon_button};

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
                let bar = crate::ui::menus::default_menu_bar();
                for menu in &bar.menus {
                    crate::ui::icons::icon_label(ui, menu.icon, "");
                    ui.menu_button(menu.title, |ui| {
                        for item in &menu.items {
                            if !item.enabled {
                                continue;
                            }
                            let icon = item.action.icon();
                            let label = item.action.label();
                            if crate::ui::icons::icon_button(ui, icon, label).clicked() {
                                match item.action {
                                    crate::ui::menus::MenuAction::New => {
                                        self.document = Document::default();
                                        self.selection.clear();
                                        self.history.clear();
                                        self.status = "New document".to_string();
                                    }
                                    crate::ui::menus::MenuAction::Undo => {
                                        self.history.undo();
                                    }
                                    crate::ui::menus::MenuAction::Redo => {
                                        self.history.redo();
                                    }
                                    crate::ui::menus::MenuAction::ZoomIn => {
                                        self.canvas.zoom_in();
                                    }
                                    crate::ui::menus::MenuAction::ZoomOut => {
                                        self.canvas.zoom_out();
                                    }
                                    crate::ui::menus::MenuAction::Grid => {
                                        self.canvas.grid_visible = !self.canvas.grid_visible;
                                    }
                                    crate::ui::menus::MenuAction::Open
                                        if !self.load_path.is_empty() =>
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
                                    crate::ui::menus::MenuAction::Save
                                        if !self.save_path.is_empty() =>
                                    {
                                        match serialization::save_to_file(
                                            &self.document,
                                            std::path::Path::new(&self.save_path),
                                        ) {
                                            Ok(()) => {
                                                self.status =
                                                    format!("Saved to {}", self.save_path);
                                            }
                                            Err(e) => self.status = format!("Save error: {e}"),
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                    });
                }
                ui.label(format!("Tool: {}", self.tool.label()));
                ui.label(format!("Zoom: {:.2}", self.canvas.zoom));
            });
        });

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
