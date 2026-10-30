#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::similar_names,
    clippy::struct_excessive_bools,
    clippy::too_many_lines,
    clippy::unused_self
)]

use std::path::{Path, PathBuf};

use egui::{Color32, Key, Modifiers, Pos2, Rect, Sense, Ui};

use crate::color::{normalize_hex_color, parse_hex_color, to_hex};
use crate::document::{Document, Shape, Unit};
use crate::editor::renderer::{PreviewShape, Theme, format_mm, paint_canvas};
use crate::editor::{EditorState, Tool};
use crate::geometry::Point2;
use crate::ui::file_dialog::{DialogResult, FileDialog, FileMode};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PendingAction {
    New,
    Open,
    Quit,
}

#[derive(Debug, Clone, Copy)]
enum DragState {
    Move {
        last: Point2,
        recorded: bool,
    },
    Draw {
        tool: Tool,
        start: Point2,
        end: Point2,
    },
    Marquee {
        start: Point2,
        end: Point2,
        additive: bool,
    },
    Pan {
        last: (f32, f32),
    },
}

pub struct BeamWrightApp {
    editor: EditorState,
    dark_mode: bool,
    current_path: Option<PathBuf>,
    saved_snapshot: Option<Document>,
    status: String,
    dialog: Option<FileDialog>,
    confirm: Option<PendingAction>,
    about_open: bool,
    drag: Option<DragState>,
    cursor_doc: Option<Point2>,
    needs_fit: bool,
    quit_requested: bool,
    job: Option<crate::gcode::JobReport>,
    active_operation: Option<String>,
    profiles: crate::profiles::ProfileLibrary,
    profiles_path: PathBuf,
    profiles_open: bool,
    profile_search: String,
    selected_material: Option<String>,
    selected_operation: Option<String>,
    profile_message: String,
    profile_io_path: String,
    import_warnings: Vec<String>,
}

impl Default for BeamWrightApp {
    fn default() -> Self {
        Self::new(None)
    }
}

impl BeamWrightApp {
    pub fn new(file: Option<PathBuf>) -> Self {
        let document = Document::default();
        let mut app = Self {
            editor: EditorState::new(document.clone()),
            dark_mode: false,
            current_path: None,
            saved_snapshot: Some(document),
            status: "Ready".to_string(),
            dialog: None,
            confirm: None,
            about_open: false,
            drag: None,
            cursor_doc: None,
            needs_fit: true,
            quit_requested: false,
            job: None,
            active_operation: None,
            profiles: crate::profiles::load_or_default(&crate::profiles::default_path())
                .unwrap_or_else(|_| crate::profiles::ProfileLibrary::with_builtins()),
            profiles_path: crate::profiles::default_path(),
            profiles_open: false,
            profile_search: String::new(),
            selected_material: None,
            selected_operation: None,
            profile_message: String::new(),
            profile_io_path: crate::profiles::default_path()
                .to_string_lossy()
                .to_string(),
            import_warnings: Vec::new(),
        };
        if let Some(path) = file {
            app.open_path(&path);
        }
        app
    }

    fn is_dirty(&self) -> bool {
        self.saved_snapshot.as_ref() != Some(&self.editor.document)
    }

    fn title(&self) -> String {
        let name = self
            .current_path
            .as_ref()
            .and_then(|path| path.file_name())
            .and_then(|name| name.to_str())
            .map_or_else(|| self.editor.document.name.clone(), str::to_string);
        if self.is_dirty() {
            format!("{name} *")
        } else {
            name
        }
    }

    fn request(&mut self, action: PendingAction) {
        if self.is_dirty() {
            self.confirm = Some(action);
        } else {
            self.perform(action);
        }
    }

    fn perform(&mut self, action: PendingAction) {
        match action {
            PendingAction::New => self.new_document(),
            PendingAction::Open => self.open_dialog(FileMode::OpenProject),
            PendingAction::Quit => self.quit_requested = true,
        }
    }

    fn new_document(&mut self) {
        let document = Document::default();
        self.editor.set_document(document.clone());
        self.saved_snapshot = Some(document);
        self.current_path = None;
        self.needs_fit = true;
        self.drag = None;
        self.status = "New document".to_string();
    }

    fn open_dialog(&mut self, mode: FileMode) {
        let stem = sanitize_name(&self.editor.document.name);
        let suggested = match mode {
            FileMode::ImportFile => "drawing.svg".to_string(),
            FileMode::ExportDxf => format!("{stem}.dxf"),
            FileMode::ExportSvg => format!("{stem}.svg"),
            FileMode::ExportGcode => format!("{stem}.nc"),
            FileMode::OpenProject | FileMode::SaveProject => format!("{stem}.bwp"),
        };
        self.dialog = Some(FileDialog::new(
            mode,
            self.current_path.as_deref(),
            &suggested,
        ));
    }

    fn handle_file(&mut self, mode: FileMode, path: &Path) {
        match mode {
            FileMode::OpenProject | FileMode::ImportFile => self.open_path(path),
            FileMode::SaveProject => self.save_to(path),
            FileMode::ExportDxf => match crate::export::export_dxf(&self.editor.document) {
                Ok(dxf) => match std::fs::write(path, dxf) {
                    Ok(()) => self.status = format!("Exported DXF to {}", path.display()),
                    Err(error) => self.status = format!("Export failed: {error}"),
                },
                Err(error) => self.status = format!("Export failed: {error}"),
            },
            FileMode::ExportGcode => {
                match crate::gcode::export_job(
                    &self.editor.document,
                    path,
                    &crate::gcode::JobOptions::default(),
                ) {
                    Ok(report) => {
                        self.status = format!(
                            "Exported G-code to {} ({} paths, {} lines, {:.1} s){}",
                            path.display(),
                            report.paths,
                            report.lines,
                            report.seconds,
                            if report.warnings.is_empty() {
                                String::new()
                            } else {
                                format!(" — {} warning(s)", report.warnings.len())
                            }
                        );
                    }
                    Err(error) => self.status = format!("Export failed: {error}"),
                }
            }
            FileMode::ExportSvg => match crate::export::export_svg(&self.editor.document) {
                Ok(svg) => match std::fs::write(path, svg) {
                    Ok(()) => self.status = format!("Exported SVG to {}", path.display()),
                    Err(error) => self.status = format!("Export failed: {error}"),
                },
                Err(error) => self.status = format!("Export failed: {error}"),
            },
        }
    }

    fn open_path(&mut self, path: &Path) {
        match crate::import::format::import_with_report(path) {
            Ok(outcome) => {
                let mut document = outcome.document;
                self.import_warnings = outcome.warnings;
                let is_project = path
                    .extension()
                    .and_then(|value| value.to_str())
                    .is_some_and(|value| value.eq_ignore_ascii_case("bwp"));
                if !is_project {
                    if let Some(stem) = path.file_stem().and_then(|value| value.to_str()) {
                        document.name = stem.to_string();
                    }
                }
                self.editor.set_document(document.clone());
                self.current_path = if is_project {
                    Some(path.to_path_buf())
                } else {
                    None
                };
                self.saved_snapshot = if is_project { Some(document) } else { None };
                self.needs_fit = true;
                self.drag = None;
                self.status = if self.import_warnings.is_empty() {
                    format!("Opened {}", path.display())
                } else {
                    format!(
                        "Opened {} with {} warning(s)",
                        path.display(),
                        self.import_warnings.len()
                    )
                };
            }
            Err(error) => self.status = format!("Open failed: {error}"),
        }
    }

    fn save(&mut self) {
        if let Some(path) = self.current_path.clone() {
            self.save_to(&path);
        } else {
            self.open_dialog(FileMode::SaveProject);
        }
    }

    fn save_to(&mut self, path: &Path) {
        if self.editor.document.name.trim().is_empty()
            || self.editor.document.name.eq_ignore_ascii_case("untitled")
        {
            if let Some(stem) = path.file_stem().and_then(|value| value.to_str()) {
                self.editor.document.name = stem.to_string();
            }
        }
        match self.editor.save(path) {
            Ok(()) => {
                self.current_path = Some(path.to_path_buf());
                self.saved_snapshot = Some(self.editor.document.clone());
                self.status = format!("Saved {}", path.display());
            }
            Err(error) => self.status = format!("Save failed: {error}"),
        }
    }

    fn undo(&mut self) {
        if self.editor.undo() {
            self.status = "Undo".to_string();
        }
    }

    fn redo(&mut self) {
        if self.editor.redo() {
            self.status = "Redo".to_string();
        }
    }

    fn shortcuts(&mut self, ui: &Ui) {
        let ctx = ui.ctx();
        if ctx.text_edit_focused() || self.dialog.is_some() || self.confirm.is_some() {
            return;
        }
        let (
            new_doc,
            open,
            save,
            save_as,
            undo,
            redo,
            redo_shift,
            select_all,
            duplicate,
            delete,
            escape,
            fit,
            grid,
            zoom_in,
            zoom_out,
            left,
            right,
            up,
            down,
        ) = ctx.input_mut(|input| {
            (
                input.consume_key(Modifiers::CTRL, Key::N),
                input.consume_key(Modifiers::CTRL, Key::O),
                input.consume_key(Modifiers::CTRL, Key::S),
                input.consume_key(Modifiers::CTRL | Modifiers::SHIFT, Key::S),
                input.consume_key(Modifiers::CTRL, Key::Z),
                input.consume_key(Modifiers::CTRL, Key::Y),
                input.consume_key(Modifiers::CTRL | Modifiers::SHIFT, Key::Z),
                input.consume_key(Modifiers::CTRL, Key::A),
                input.consume_key(Modifiers::CTRL, Key::D),
                input.consume_key(Modifiers::NONE, Key::Delete)
                    || input.consume_key(Modifiers::NONE, Key::Backspace),
                input.consume_key(Modifiers::NONE, Key::Escape),
                input.consume_key(Modifiers::NONE, Key::F),
                input.consume_key(Modifiers::NONE, Key::G),
                input.consume_key(Modifiers::NONE, Key::Plus)
                    || input.consume_key(Modifiers::NONE, Key::Equals),
                input.consume_key(Modifiers::NONE, Key::Minus),
                input.consume_key(Modifiers::NONE, Key::ArrowLeft),
                input.consume_key(Modifiers::NONE, Key::ArrowRight),
                input.consume_key(Modifiers::NONE, Key::ArrowUp),
                input.consume_key(Modifiers::NONE, Key::ArrowDown),
            )
        });
        if new_doc {
            self.request(PendingAction::New);
        }
        if open {
            self.request(PendingAction::Open);
        }
        if save_as {
            self.open_dialog(FileMode::SaveProject);
        } else if save {
            self.save();
        }
        if undo {
            self.undo();
        }
        if redo || redo_shift {
            self.redo();
        }
        if select_all {
            self.editor.select_all();
        }
        if duplicate {
            self.editor.duplicate_selection();
        }
        if delete {
            self.editor.delete_selection();
        }
        if escape {
            self.drag = None;
            self.editor.selection.clear();
        }
        if fit {
            self.needs_fit = true;
        }
        if grid {
            self.editor.canvas.grid_visible = !self.editor.canvas.grid_visible;
        }
        if zoom_in {
            self.editor.canvas.zoom_in();
        }
        if zoom_out {
            self.editor.canvas.zoom_out();
        }
        let step = if ctx.input(|input| input.modifiers.shift) {
            0.1
        } else {
            1.0
        };
        if left {
            self.editor.nudge_selection(-step, 0.0);
        }
        if right {
            self.editor.nudge_selection(step, 0.0);
        }
        if up {
            self.editor.nudge_selection(0.0, step);
        }
        if down {
            self.editor.nudge_selection(0.0, -step);
        }
    }

    fn menu_bar(&mut self, ui: &mut Ui) {
        egui::Panel::top("menu_bar").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("New").clicked() {
                        self.request(PendingAction::New);
                        ui.close();
                    }
                    if ui.button("Open...").clicked() {
                        self.request(PendingAction::Open);
                        ui.close();
                    }
                    if ui.button("Save").clicked() {
                        self.save();
                        ui.close();
                    }
                    if ui.button("Save as...").clicked() {
                        self.open_dialog(FileMode::SaveProject);
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("Import SVG or DXF...").clicked() {
                        self.open_dialog(FileMode::ImportFile);
                        ui.close();
                    }
                    if ui.button("Export SVG...").clicked() {
                        self.open_dialog(FileMode::ExportSvg);
                        ui.close();
                    }
                    if ui.button("Export DXF...").clicked() {
                        self.open_dialog(FileMode::ExportDxf);
                        ui.close();
                    }
                    if ui.button("Export G-code...").clicked() {
                        self.open_dialog(FileMode::ExportGcode);
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("Quit").clicked() {
                        self.request(PendingAction::Quit);
                        ui.close();
                    }
                });
                ui.menu_button("Edit", |ui| {
                    if ui
                        .add_enabled(self.editor.history.can_undo(), egui::Button::new("Undo"))
                        .clicked()
                    {
                        self.undo();
                        ui.close();
                    }
                    if ui
                        .add_enabled(self.editor.history.can_redo(), egui::Button::new("Redo"))
                        .clicked()
                    {
                        self.redo();
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("Select all").clicked() {
                        self.editor.select_all();
                        ui.close();
                    }
                    if ui
                        .add_enabled(
                            !self.editor.selection.is_empty(),
                            egui::Button::new("Duplicate"),
                        )
                        .clicked()
                    {
                        self.editor.duplicate_selection();
                        ui.close();
                    }
                    if ui
                        .add_enabled(
                            !self.editor.selection.is_empty(),
                            egui::Button::new("Delete"),
                        )
                        .clicked()
                    {
                        self.editor.delete_selection();
                        ui.close();
                    }
                });
                ui.menu_button("Arrange", |ui| {
                    let enabled = !self.editor.selection.is_empty();
                    if ui
                        .add_enabled(enabled, egui::Button::new("Rotate 90° left"))
                        .clicked()
                    {
                        self.editor.rotate_selection(std::f64::consts::FRAC_PI_2);
                        ui.close();
                    }
                    if ui
                        .add_enabled(enabled, egui::Button::new("Rotate 90° right"))
                        .clicked()
                    {
                        self.editor.rotate_selection(-std::f64::consts::FRAC_PI_2);
                        ui.close();
                    }
                    ui.separator();
                    if ui
                        .add_enabled(enabled, egui::Button::new("Flip horizontal"))
                        .clicked()
                    {
                        self.editor.flip_selection(true);
                        ui.close();
                    }
                    if ui
                        .add_enabled(enabled, egui::Button::new("Flip vertical"))
                        .clicked()
                    {
                        self.editor.flip_selection(false);
                        ui.close();
                    }
                    ui.separator();
                    if ui
                        .add_enabled(enabled, egui::Button::new("Scale up 10 %"))
                        .clicked()
                    {
                        self.editor.scale_selection(1.1, 1.1);
                        ui.close();
                    }
                    if ui
                        .add_enabled(enabled, egui::Button::new("Scale down 10 %"))
                        .clicked()
                    {
                        self.editor.scale_selection(1.0 / 1.1, 1.0 / 1.1);
                        ui.close();
                    }
                });
                ui.menu_button("Profiles", |ui| {
                    if ui.button("Materials and operations...").clicked() {
                        self.profiles_open = true;
                        ui.close();
                    }
                });
                ui.menu_button("View", |ui| {
                    if ui.button("Zoom in").clicked() {
                        self.editor.canvas.zoom_in();
                        ui.close();
                    }
                    if ui.button("Zoom out").clicked() {
                        self.editor.canvas.zoom_out();
                        ui.close();
                    }
                    if ui.button("Fit page").clicked() {
                        self.needs_fit = true;
                        ui.close();
                    }
                    ui.separator();
                    ui.checkbox(&mut self.editor.canvas.grid_visible, "Show grid");
                    ui.checkbox(&mut self.editor.snap_enabled, "Snap to 1 mm");
                    ui.checkbox(&mut self.dark_mode, "Dark mode");
                });
                ui.menu_button("Help", |ui| {
                    if ui.button("About BeamWright").clicked() {
                        self.about_open = true;
                        ui.close();
                    }
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(self.title());
                });
            });
        });
    }

    fn toolbar(&mut self, ui: &mut Ui) {
        egui::Panel::top("toolbar").show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                for tool in [
                    Tool::Select,
                    Tool::Line,
                    Tool::Rect,
                    Tool::Ellipse,
                    Tool::Pan,
                ] {
                    if ui
                        .selectable_label(self.editor.tool == tool, tool.label())
                        .clicked()
                    {
                        self.editor.tool = tool;
                    }
                }
                ui.separator();
                if ui
                    .add_enabled(self.editor.history.can_undo(), egui::Button::new("Undo"))
                    .clicked()
                {
                    self.undo();
                }
                if ui
                    .add_enabled(self.editor.history.can_redo(), egui::Button::new("Redo"))
                    .clicked()
                {
                    self.redo();
                }
                if ui
                    .add_enabled(
                        !self.editor.selection.is_empty(),
                        egui::Button::new("Duplicate"),
                    )
                    .clicked()
                {
                    self.editor.duplicate_selection();
                }
                if ui
                    .add_enabled(
                        !self.editor.selection.is_empty(),
                        egui::Button::new("Delete"),
                    )
                    .clicked()
                {
                    self.editor.delete_selection();
                }
                ui.separator();
                if ui.button("-").clicked() {
                    self.editor.canvas.zoom_out();
                }
                ui.label(format!("{:.0}%", self.editor.canvas.zoom * 100.0));
                if ui.button("+").clicked() {
                    self.editor.canvas.zoom_in();
                }
                if ui.button("Fit").clicked() {
                    self.needs_fit = true;
                }
                ui.separator();
                ui.checkbox(&mut self.editor.canvas.grid_visible, "Grid");
                ui.checkbox(&mut self.editor.snap_enabled, "Snap");
                ui.separator();
                if ui.button("Open").clicked() {
                    self.request(PendingAction::Open);
                }
                if ui.button("Save").clicked() {
                    self.save();
                }
            });
        });
    }

    fn layers_panel(&mut self, ui: &mut Ui) {
        egui::Panel::left("layers_panel")
            .default_size(210.0)
            .resizable(true)
            .show(ui, |ui| {
                ui.heading("Layers");
                let before = self.editor.document.clone();
                let mut message: Option<String> = None;
                ui.horizontal(|ui| {
                    if ui.button("+ Layer").clicked() {
                        let name = format!("Layer {}", self.editor.document.layers.len() + 1);
                        let layer = self.editor.document.add_layer(name);
                        self.editor.active_layer_id.clone_from(&layer.id);
                    }
                    if ui.button("- Layer").clicked() {
                        let id = self.editor.active_layer_id.clone();
                        match self.editor.document.remove_layer(&id) {
                            Ok(()) => {
                                self.editor.active_layer_id = self
                                    .editor
                                    .document
                                    .layers
                                    .last()
                                    .map(|layer| layer.id.clone())
                                    .unwrap_or_default();
                            }
                            Err(error) => message = Some(error),
                        }
                    }
                });
                ui.separator();
                let layers: Vec<_> = self
                    .editor
                    .document
                    .layers
                    .iter()
                    .map(|layer| {
                        (
                            layer.id.clone(),
                            layer.name.clone(),
                            layer.visible,
                            layer.locked,
                            layer.color.clone(),
                        )
                    })
                    .collect();
                for (id, name, visible, locked, color) in layers.iter().rev() {
                    ui.horizontal(|ui| {
                        let mut is_visible = *visible;
                        if ui.checkbox(&mut is_visible, "").changed() {
                            if let Some(layer) = self.editor.document.layer_mut(id) {
                                layer.visible = is_visible;
                            }
                        }
                        let mut is_locked = *locked;
                        if ui.checkbox(&mut is_locked, "L").changed() {
                            if let Some(layer) = self.editor.document.layer_mut(id) {
                                layer.locked = is_locked;
                            }
                        }
                        let rgb = parse_hex_color(color).unwrap_or((0, 0, 0));
                        let mut srgb = [rgb.0, rgb.1, rgb.2];
                        if ui.color_edit_button_srgb(&mut srgb).changed() {
                            if let Some(layer) = self.editor.document.layer_mut(id) {
                                layer.color = to_hex(srgb[0], srgb[1], srgb[2]);
                            }
                        }
                        let selected = self.editor.active_layer_id == *id;
                        if ui.selectable_label(selected, name).clicked() {
                            self.editor.active_layer_id.clone_from(id);
                        }
                        let count = self.editor.document.objects_on_layer(id).len();
                        ui.weak(format!("({count})"));
                    });
                }
                ui.separator();
                if let Some(layer) = self.editor.document.layer_mut(&self.editor.active_layer_id) {
                    ui.label("Layer name");
                    ui.text_edit_singleline(&mut layer.name);
                }
                if let Some(message) = message {
                    ui.colored_label(Color32::from_rgb(200, 60, 60), message);
                }
                if self.editor.document != before {
                    self.editor.history.record(&before);
                }
                ui.separator();
                ui.label(format!(
                    "{} objects on {} layers",
                    self.editor.document.objects.len(),
                    self.editor.document.layers.len()
                ));
                ui.separator();
                ui.heading("Job");
                ui.horizontal(|ui| {
                    if ui.button("Estimate").clicked() {
                        match crate::gcode::render_job(
                            &self.editor.document,
                            &crate::gcode::JobOptions::default(),
                        ) {
                            Ok((_, report)) => self.job = Some(report),
                            Err(error) => {
                                self.job = None;
                                self.status = format!("Job error: {error}");
                            }
                        }
                    }
                    if ui.button("Export G-code...").clicked() {
                        self.open_dialog(FileMode::ExportGcode);
                    }
                });
                if let Some(report) = &self.job {
                    ui.label(format!("Paths: {}", report.paths));
                    ui.label(format!("Lines: {} ({} bytes)", report.lines, report.bytes));
                    ui.label(format!("Estimated: {:.1} s", report.seconds));
                    for warning in &report.warnings {
                        ui.colored_label(Color32::from_rgb(180, 120, 0), warning);
                    }
                }
                if !self.import_warnings.is_empty() {
                    ui.separator();
                    ui.heading("Import warnings");
                    for warning in &self.import_warnings {
                        ui.colored_label(Color32::from_rgb(180, 120, 0), warning);
                    }
                    if ui.button("Dismiss").clicked() {
                        self.import_warnings.clear();
                    }
                }
                ui.separator();
                self.operations_section(ui);
            });
    }

    fn operations_section(&mut self, ui: &mut Ui) {
        use crate::operations::OperationType;

        ui.heading("Operations");
        let before = self.editor.document.clone();
        let operations: Vec<_> = self
            .editor
            .document
            .operations
            .iter()
            .map(|operation| {
                (
                    operation.id.clone(),
                    operation.name.clone(),
                    operation.op_type,
                    operation.enabled,
                )
            })
            .collect();
        ui.horizontal(|ui| {
            if ui.button("+ Operation").clicked() {
                let id = self.editor.add_operation(
                    format!("Operation {}", operations.len() + 1),
                    OperationType::OutlineCutting,
                );
                self.active_operation = Some(id);
            }
            let active = self.active_operation.clone().unwrap_or_default();
            if ui
                .add_enabled(
                    operations.iter().any(|(id, ..)| *id == active),
                    egui::Button::new("- Operation"),
                )
                .clicked()
            {
                self.editor.remove_operation(&active);
                self.active_operation = None;
            }
        });
        for (id, name, op_type, enabled) in &operations {
            ui.horizontal(|ui| {
                let mut is_enabled = *enabled;
                if ui.checkbox(&mut is_enabled, "").changed() {
                    if let Some(operation) = self
                        .editor
                        .document
                        .operations
                        .iter_mut()
                        .find(|operation| operation.id == *id)
                    {
                        operation.enabled = is_enabled;
                    }
                }
                let selected = self.active_operation.as_deref() == Some(id.as_str());
                if ui
                    .selectable_label(selected, format!("{name} ({})", op_type.label()))
                    .clicked()
                {
                    self.active_operation = Some(id.clone());
                }
                let used = self
                    .editor
                    .document
                    .objects
                    .iter()
                    .filter(|object| object.operation_id.as_deref() == Some(id.as_str()))
                    .count();
                ui.weak(format!("({used})"));
            });
        }
        if let Some(active) = self.active_operation.clone() {
            if let Some(operation) = self
                .editor
                .document
                .operations
                .iter_mut()
                .find(|operation| operation.id == active)
            {
                ui.label("Name");
                ui.text_edit_singleline(&mut operation.name);
                let mut op_type = operation.op_type;
                egui::ComboBox::from_label("Type")
                    .selected_text(op_type.label())
                    .show_ui(ui, |ui| {
                        for candidate in OperationType::all() {
                            ui.selectable_value(&mut op_type, candidate, candidate.label());
                        }
                    });
                operation.op_type = op_type;
                ui.horizontal(|ui| {
                    ui.label("Speed");
                    ui.add(
                        egui::DragValue::new(&mut operation.params.speed)
                            .speed(10.0)
                            .range(1.0..=20000.0)
                            .suffix(" mm/min"),
                    );
                });
                ui.horizontal(|ui| {
                    ui.label("Power");
                    ui.add(
                        egui::DragValue::new(&mut operation.params.power)
                            .speed(1.0)
                            .range(0.0..=100.0)
                            .suffix(" %"),
                    );
                });
                ui.horizontal(|ui| {
                    ui.label("Passes");
                    ui.add(
                        egui::DragValue::new(&mut operation.params.passes)
                            .speed(1.0)
                            .range(1..=100),
                    );
                    ui.label("Interval");
                    ui.add(
                        egui::DragValue::new(&mut operation.params.line_interval_mm)
                            .speed(0.01)
                            .range(0.01..=10.0)
                            .suffix(" mm"),
                    );
                });
                if matches!(
                    operation.op_type,
                    OperationType::Fill
                        | OperationType::FillWithOutline
                        | OperationType::FillWithOffset
                ) {
                    ui.weak("Fill spacing is the line interval above.");
                }
            }
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(
                        !self.editor.selection.is_empty(),
                        egui::Button::new("Assign to selection"),
                    )
                    .clicked()
                {
                    let changed = self.editor.assign_operation(Some(&active));
                    self.status = format!("Assigned operation to {changed} object(s)");
                }
                if ui
                    .add_enabled(
                        !self.editor.selection.is_empty(),
                        egui::Button::new("Clear assignment"),
                    )
                    .clicked()
                {
                    let changed = self.editor.assign_operation(None);
                    self.status = format!("Cleared operation on {changed} object(s)");
                }
            });
        }
        if self.editor.document != before {
            self.editor.history.record(&before);
        }
    }

    fn properties_panel(&mut self, ui: &mut Ui) {
        egui::Panel::right("properties_panel")
            .default_size(280.0)
            .resizable(true)
            .show(ui, |ui| {
                ui.heading("Properties");
                let before = self.editor.document.clone();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    if self.editor.selection.is_empty() {
                        self.document_properties(ui);
                    } else if self.editor.selection.len() == 1 {
                        let id = self.editor.selection.ids()[0].clone();
                        self.object_properties(ui, &id);
                    } else {
                        self.multi_selection_properties(ui);
                    }
                });
                if self.editor.document != before {
                    self.editor.history.record(&before);
                }
            });
    }

    fn document_properties(&mut self, ui: &mut Ui) {
        ui.label("Document");
        egui::Grid::new("document_properties")
            .num_columns(2)
            .spacing([8.0, 6.0])
            .show(ui, |ui| {
                ui.label("Name");
                ui.text_edit_singleline(&mut self.editor.document.name);
                ui.end_row();

                ui.label("Width (mm)");
                let mut width = self.editor.document.width_mm;
                if ui
                    .add(
                        egui::DragValue::new(&mut width)
                            .speed(0.5)
                            .range(0.1..=10000.0),
                    )
                    .changed()
                {
                    self.editor.document.width_mm = width;
                }
                ui.end_row();

                ui.label("Height (mm)");
                let mut height = self.editor.document.height_mm;
                if ui
                    .add(
                        egui::DragValue::new(&mut height)
                            .speed(0.5)
                            .range(0.1..=10000.0),
                    )
                    .changed()
                {
                    self.editor.document.height_mm = height;
                }
                ui.end_row();

                ui.label("Unit");
                egui::ComboBox::from_id_salt("document_unit")
                    .selected_text(self.editor.document.unit.label())
                    .show_ui(ui, |ui| {
                        for unit in [
                            Unit::Millimeters,
                            Unit::Centimeters,
                            Unit::Inches,
                            Unit::Pixels,
                        ] {
                            ui.selectable_value(&mut self.editor.document.unit, unit, unit.label());
                        }
                    });
                ui.end_row();
            });
        ui.separator();
        ui.label("No object selected.");
        ui.label("Drag a rectangle, ellipse or line on the canvas.");
    }

    fn object_properties(&mut self, ui: &mut Ui, id: &str) {
        let layers: Vec<(String, String)> = self
            .editor
            .document
            .layers
            .iter()
            .map(|layer| (layer.id.clone(), layer.name.clone()))
            .collect();
        let Some(object) = self.editor.document.object_mut(id) else {
            return;
        };
        ui.label(object.shape.kind_name());
        egui::Grid::new("object_properties")
            .num_columns(2)
            .spacing([8.0, 6.0])
            .show(ui, |ui| {
                ui.label("Name");
                ui.text_edit_singleline(&mut object.name);
                ui.end_row();

                ui.label("Layer");
                let current = layers
                    .iter()
                    .find(|(layer_id, _)| layer_id == &object.layer_id)
                    .map_or("?", |(_, name)| name.as_str());
                egui::ComboBox::from_id_salt("object_layer")
                    .selected_text(current)
                    .show_ui(ui, |ui| {
                        for (layer_id, name) in &layers {
                            ui.selectable_value(&mut object.layer_id, layer_id.clone(), name);
                        }
                    });
                ui.end_row();

                ui.label("Visible");
                ui.checkbox(&mut object.visible, "");
                ui.end_row();

                ui.label("Locked");
                ui.checkbox(&mut object.locked, "");
                ui.end_row();
            });
        ui.separator();

        if let Some((min, max)) = object.bounds() {
            ui.label("Position (mm)");
            egui::Grid::new("object_position")
                .num_columns(2)
                .spacing([8.0, 6.0])
                .show(ui, |ui| {
                    ui.label("X");
                    let mut x = min.x;
                    if ui.add(egui::DragValue::new(&mut x).speed(0.1)).changed() {
                        object.translate(x - min.x, 0.0);
                    }
                    ui.end_row();
                    ui.label("Y");
                    let mut y = min.y;
                    if ui.add(egui::DragValue::new(&mut y).speed(0.1)).changed() {
                        object.translate(0.0, y - min.y);
                    }
                    ui.end_row();
                    ui.label("Width");
                    ui.weak(format!("{:.2}", max.x - min.x));
                    ui.end_row();
                    ui.label("Height");
                    ui.weak(format!("{:.2}", max.y - min.y));
                    ui.end_row();
                });
            ui.separator();
        }

        match &mut object.shape {
            Shape::Rect {
                width,
                height,
                corner_radius,
            } => {
                ui.label("Rectangle");
                egui::Grid::new("object_rect")
                    .num_columns(2)
                    .spacing([8.0, 6.0])
                    .show(ui, |ui| {
                        ui.label("Width");
                        ui.add(
                            egui::DragValue::new(width)
                                .speed(0.1)
                                .range(0.01..=100_000.0),
                        );
                        ui.end_row();
                        ui.label("Height");
                        ui.add(
                            egui::DragValue::new(height)
                                .speed(0.1)
                                .range(0.01..=100_000.0),
                        );
                        ui.end_row();
                        ui.label("Corner radius");
                        ui.add(
                            egui::DragValue::new(corner_radius)
                                .speed(0.1)
                                .range(0.0..=100_000.0),
                        );
                        ui.end_row();
                    });
            }
            Shape::Ellipse { rx, ry } => {
                ui.label("Ellipse");
                egui::Grid::new("object_ellipse")
                    .num_columns(2)
                    .spacing([8.0, 6.0])
                    .show(ui, |ui| {
                        ui.label("Radius X");
                        ui.add(egui::DragValue::new(rx).speed(0.1).range(0.005..=100_000.0));
                        ui.end_row();
                        ui.label("Radius Y");
                        ui.add(egui::DragValue::new(ry).speed(0.1).range(0.005..=100_000.0));
                        ui.end_row();
                    });
            }
            Shape::Path(path) => {
                ui.label(format!("Path ({} segments)", path.segment_count()));
            }
            Shape::Text(text) => {
                ui.label("Text");
                egui::Grid::new("object_text")
                    .num_columns(2)
                    .spacing([8.0, 6.0])
                    .show(ui, |ui| {
                        ui.label("Content");
                        ui.text_edit_singleline(&mut text.content);
                        ui.end_row();
                        ui.label("Font");
                        ui.text_edit_singleline(&mut text.font);
                        ui.end_row();
                        ui.label("Size");
                        ui.add(
                            egui::DragValue::new(&mut text.size)
                                .speed(0.1)
                                .range(0.1..=10000.0),
                        );
                        ui.end_row();
                    });
            }
            Shape::Image(image) => {
                ui.label("Image");
                ui.label(&image.path);
                ui.weak(format!(
                    "{:.1} x {:.1} mm at {:.0} dpi",
                    image.width_mm(),
                    image.height_mm(),
                    image.dpi
                ));
            }
        }
        ui.separator();
        ui.label("Style");
        egui::Grid::new("object_style")
            .num_columns(2)
            .spacing([8.0, 6.0])
            .show(ui, |ui| {
                ui.label("Stroke width");
                ui.add(
                    egui::DragValue::new(&mut object.style.stroke_width_mm)
                        .speed(0.01)
                        .range(0.0..=100.0),
                );
                ui.end_row();
                ui.label("Stroke");
                edit_color(ui, &mut object.style.stroke);
                ui.end_row();
                ui.label("Fill");
                edit_color(ui, &mut object.style.fill);
                ui.end_row();
            });
    }

    fn multi_selection_properties(&mut self, ui: &mut Ui) {
        ui.label(format!("{} objects selected", self.editor.selection.len()));
        if let Some((min, max)) = self.editor.selected_bounds() {
            egui::Grid::new("multi_bounds")
                .num_columns(2)
                .spacing([8.0, 6.0])
                .show(ui, |ui| {
                    ui.label("Left");
                    ui.weak(format!("{:.2} mm", min.x));
                    ui.end_row();
                    ui.label("Bottom");
                    ui.weak(format!("{:.2} mm", min.y));
                    ui.end_row();
                    ui.label("Width");
                    ui.weak(format!("{:.2} mm", max.x - min.x));
                    ui.end_row();
                    ui.label("Height");
                    ui.weak(format!("{:.2} mm", max.y - min.y));
                    ui.end_row();
                });
        }
        ui.separator();
        ui.horizontal(|ui| {
            if ui.button("Duplicate").clicked() {
                self.editor.duplicate_selection();
            }
            if ui.button("Delete").clicked() {
                self.editor.delete_selection();
            }
        });
    }

    fn status_bar(&mut self, ui: &mut Ui) {
        egui::Panel::bottom("status_bar").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(format!("Tool: {}", self.editor.tool.label()));
                ui.separator();
                if let Some(point) = self.cursor_doc {
                    ui.label(format!(
                        "X {}  Y {} mm",
                        format_mm(point.x),
                        format_mm(point.y)
                    ));
                } else {
                    ui.label("X --  Y -- mm");
                }
                ui.separator();
                ui.label(format!("Zoom {:.0}%", self.editor.canvas.zoom * 100.0));
                ui.separator();
                ui.label(format!(
                    "{} x {} mm",
                    format_mm(self.editor.document.width_mm),
                    format_mm(self.editor.document.height_mm)
                ));
                ui.separator();
                ui.label(format!(
                    "{} selected / {} objects",
                    self.editor.selection.len(),
                    self.editor.document.objects.len()
                ));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if !self.status.is_empty() {
                        ui.label(&self.status);
                    }
                });
            });
        });
    }

    fn canvas(&mut self, ui: &mut Ui) {
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| {
                let (rect, response) =
                    ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
                if rect.width() < 8.0 || rect.height() < 8.0 {
                    return;
                }
                if self.needs_fit {
                    self.editor
                        .fit_view(f64::from(rect.width()), f64::from(rect.height()));
                    self.needs_fit = false;
                }
                let ctx = ui.ctx().clone();
                let modal = self.dialog.is_some() || self.confirm.is_some() || self.about_open;
                let pointer = if modal {
                    None
                } else {
                    ctx.input(|input| input.pointer.interact_pos())
                };
                let primary_pressed = !modal && ctx.input(|input| input.pointer.primary_pressed());
                let primary_released =
                    !modal && ctx.input(|input| input.pointer.primary_released());
                let shift = ctx.input(|input| input.modifiers.shift);

                if let Some(pos) = pointer {
                    let (x, y) = self.editor.canvas.screen_to_doc(
                        f64::from(pos.x - rect.min.x),
                        f64::from(pos.y - rect.min.y),
                    );
                    self.cursor_doc = Some(Point2::new(x, y));
                }

                let pointer_in_canvas = pointer.is_some_and(|pos| rect.contains(pos));
                if pointer_in_canvas {
                    let scroll = ctx.input(|input| input.smooth_scroll_delta.y);
                    if scroll.abs() > 0.0 {
                        if let Some(pos) = pointer {
                            let factor = if scroll > 0.0 { 1.15 } else { 1.0 / 1.15 };
                            self.editor.canvas.zoom_at(
                                f64::from(pos.x - rect.min.x),
                                f64::from(pos.y - rect.min.y),
                                factor,
                            );
                        }
                    }
                }

                if pointer_in_canvas
                    && ctx.input(|input| input.pointer.button_down(egui::PointerButton::Middle))
                {
                    let delta = ctx.input(|input| input.pointer.delta());
                    self.editor
                        .canvas
                        .pan(f64::from(delta.x), f64::from(delta.y));
                }

                if primary_pressed && pointer_in_canvas {
                    response.request_focus();
                    if let Some(pos) = pointer {
                        let point = self.doc_point(rect, pos);
                        match self.editor.tool {
                            Tool::Select => self.begin_select(point, shift),
                            Tool::Rect | Tool::Ellipse | Tool::Line => {
                                if self.editor.layer_locked(&self.editor.active_layer_id)
                                    || !self.editor.layer_visible(&self.editor.active_layer_id)
                                {
                                    self.status =
                                        "Cannot draw on a hidden or locked layer".to_string();
                                } else {
                                    self.drag = Some(DragState::Draw {
                                        tool: self.editor.tool,
                                        start: point,
                                        end: point,
                                    });
                                }
                            }
                            Tool::Pan => {
                                self.drag = Some(DragState::Pan {
                                    last: (pos.x, pos.y),
                                });
                            }
                        }
                    }
                }

                if let Some(pos) = pointer {
                    if self.drag.is_some() {
                        self.update_drag(rect, pos);
                    }
                }

                if primary_released {
                    if let Some(drag) = self.drag.take() {
                        self.finish_drag(drag);
                    }
                }

                let preview = match &self.drag {
                    Some(DragState::Draw { tool, start, end }) => Some(match tool {
                        Tool::Rect => PreviewShape::Rect {
                            start: *start,
                            end: *end,
                        },
                        Tool::Ellipse => PreviewShape::Ellipse {
                            start: *start,
                            end: *end,
                        },
                        _ => PreviewShape::Line {
                            start: *start,
                            end: *end,
                        },
                    }),
                    Some(DragState::Marquee { start, end, .. }) => Some(PreviewShape::Rect {
                        start: *start,
                        end: *end,
                    }),
                    _ => None,
                };
                let theme = Theme {
                    dark: self.dark_mode,
                };
                let painter = ui.painter_at(rect);
                paint_canvas(&painter, rect, &self.editor, theme, preview);

                if pointer_in_canvas {
                    let cursor = match self.editor.tool {
                        Tool::Pan => egui::CursorIcon::Grab,
                        Tool::Select => egui::CursorIcon::Default,
                        _ => egui::CursorIcon::Crosshair,
                    };
                    ctx.set_cursor_icon(cursor);
                }
                // Canvas edits are applied after the side panels have already
                // been painted; request one more frame so they show the result.
                if primary_pressed || primary_released || self.drag.is_some() {
                    ctx.request_repaint();
                }
            });
    }

    fn doc_point(&self, rect: Rect, pos: Pos2) -> Point2 {
        let (x, y) = self
            .editor
            .canvas
            .screen_to_doc(f64::from(pos.x - rect.min.x), f64::from(pos.y - rect.min.y));
        self.editor.snap(Point2::new(x, y))
    }

    fn begin_select(&mut self, point: Point2, shift: bool) {
        let tolerance = 5.0 / self.editor.canvas.zoom.max(0.01);
        let hit = self.editor.object_at(point, tolerance);
        if let Some(id) = hit {
            if shift {
                self.editor.selection.toggle(id);
            } else {
                if !self.editor.selection.contains(&id) {
                    self.editor.selection.clear();
                    self.editor.selection.add(id);
                }
                self.drag = Some(DragState::Move {
                    last: point,
                    recorded: false,
                });
            }
        } else {
            if !shift {
                self.editor.selection.clear();
            }
            self.drag = Some(DragState::Marquee {
                start: point,
                end: point,
                additive: shift,
            });
        }
    }

    fn update_drag(&mut self, rect: Rect, pos: Pos2) {
        let point = self.doc_point(rect, pos);
        let Some(drag) = &mut self.drag else {
            return;
        };
        match drag {
            DragState::Move { last, recorded } => {
                let dx = point.x - last.x;
                let dy = point.y - last.y;
                if dx.abs() > 1e-9 || dy.abs() > 1e-9 {
                    if !*recorded {
                        self.editor.record();
                        *recorded = true;
                    }
                    self.editor.move_selection(dx, dy);
                    *last = point;
                }
            }
            DragState::Draw { end, .. } | DragState::Marquee { end, .. } => {
                *end = point;
            }
            DragState::Pan { last } => {
                let dx = f64::from(pos.x - last.0);
                let dy = f64::from(pos.y - last.1);
                self.editor.canvas.pan(dx, dy);
                *last = (pos.x, pos.y);
            }
        }
    }

    fn finish_drag(&mut self, drag: DragState) {
        match drag {
            DragState::Draw { tool, start, end } => match tool {
                Tool::Rect => {
                    self.editor.add_rectangle(start, end);
                }
                Tool::Ellipse => {
                    self.editor.add_ellipse(start, end);
                }
                Tool::Line => {
                    self.editor.add_line(start, end);
                }
                _ => {}
            },
            DragState::Marquee {
                start,
                end,
                additive,
            } => {
                let min = Point2::new(start.x.min(end.x), start.y.min(end.y));
                let max = Point2::new(start.x.max(end.x), start.y.max(end.y));
                if (max.x - min.x) > 1e-6 || (max.y - min.y) > 1e-6 {
                    self.editor.select_in_rect((min, max), additive);
                }
            }
            DragState::Move { .. } | DragState::Pan { .. } => {}
        }
    }

    fn save_profiles(&mut self) {
        match crate::profiles::save(&self.profiles, &self.profiles_path) {
            Ok(()) => self.profile_message = "Profiles saved".to_string(),
            Err(error) => self.profile_message = format!("Cannot save profiles: {error}"),
        }
    }

    fn profiles_window(&mut self, ctx: &egui::Context) {
        use crate::operations::OperationType;
        use crate::profiles::{MaterialProfile, OperationProfile};

        let mut open = self.profiles_open;
        let mut changed = false;
        egui::Window::new("Materials and operations")
            .open(&mut open)
            .default_size([560.0, 520.0])
            .show(ctx, |ui| {
                ui.label(
                    "Values are reference starting points, not guarantees. Test on your own material.",
                );
                ui.horizontal(|ui| {
                    ui.label("Search");
                    ui.text_edit_singleline(&mut self.profile_search);
                });
                let query = self.profile_search.trim().to_lowercase();
                ui.separator();
                ui.heading("Materials");
                let materials: Vec<_> = self
                    .profiles
                    .materials
                    .iter()
                    .filter(|profile| {
                        query.is_empty()
                            || format!("{} {} {}", profile.name, profile.material, profile.notes)
                                .to_lowercase()
                                .contains(&query)
                    })
                    .map(|profile| {
                        (
                            profile.id.clone(),
                            profile.name.clone(),
                            profile.builtin,
                        )
                    })
                    .collect();
                for (id, name, builtin) in &materials {
                    ui.horizontal(|ui| {
                        let selected = self.selected_material.as_deref() == Some(id.as_str());
                        if ui.selectable_label(selected, name).clicked() {
                            self.selected_material = Some(id.clone());
                        }
                        if *builtin {
                            ui.weak("built-in");
                        }
                    });
                }
                ui.horizontal(|ui| {
                    if ui.button("+ Material").clicked() {
                        let profile = self
                            .profiles
                            .add_material(MaterialProfile::new("New material", "Material", 3.0));
                        self.selected_material = Some(profile.id.clone());
                        changed = true;
                    }
                    if ui
                        .add_enabled(self.selected_material.is_some(), egui::Button::new("Duplicate"))
                        .clicked()
                    {
                        if let Some(id) = self.selected_material.clone() {
                            if let Some(copy) = self.profiles.duplicate_material(&id) {
                                self.selected_material = Some(copy);
                            }
                        }
                    }
                    let removable = self
                        .selected_material
                        .as_deref()
                        .and_then(|id| self.profiles.material(id))
                        .is_some_and(|profile| !profile.builtin);
                    if ui
                        .add_enabled(removable, egui::Button::new("Delete"))
                        .clicked()
                    {
                        if let Some(id) = self.selected_material.clone() {
                            match self.profiles.remove_material(&id) {
                                Ok(()) => {
                                    self.selected_material = None;
                                    changed = true;
                                }
                                Err(error) => self.profile_message = error,
                            }
                        }
                    }
                });
                if let Some(id) = self.selected_material.clone() {
                    if let Some(profile) = self
                        .profiles
                        .materials
                        .iter_mut()
                        .find(|profile| profile.id == id)
                    {
                        let editable = !profile.builtin;
                        ui.add_enabled_ui(editable, |ui| {
                            ui.horizontal(|ui| {
                                ui.label("Name");
                                changed |= ui.text_edit_singleline(&mut profile.name).changed();
                            });
                            ui.horizontal(|ui| {
                                ui.label("Material");
                                changed |=
                                    ui.text_edit_singleline(&mut profile.material).changed();
                            });
                            ui.horizontal(|ui| {
                                ui.label("Thickness");
                                changed |= ui
                                    .add(
                                        egui::DragValue::new(&mut profile.thickness_mm)
                                            .speed(0.1)
                                            .range(0.1..=100.0)
                                            .suffix(" mm"),
                                    )
                                    .changed();
                            });
                            ui.horizontal(|ui| {
                                ui.label("Notes");
                                changed |= ui.text_edit_singleline(&mut profile.notes).changed();
                            });
                        });
                        if !editable {
                            ui.weak("Built-in profile: duplicate it to edit.");
                        }
                    }
                }
                ui.separator();
                ui.heading("Operation profiles");
                let operations: Vec<_> = self
                    .profiles
                    .operations
                    .iter()
                    .filter(|profile| {
                        query.is_empty()
                            || format!("{} {}", profile.name, profile.notes)
                                .to_lowercase()
                                .contains(&query)
                    })
                    .map(|profile| (profile.id.clone(), profile.name.clone(), profile.builtin))
                    .collect();
                for (id, name, builtin) in &operations {
                    ui.horizontal(|ui| {
                        let selected = self.selected_operation.as_deref() == Some(id.as_str());
                        if ui.selectable_label(selected, name).clicked() {
                            self.selected_operation = Some(id.clone());
                        }
                        if *builtin {
                            ui.weak("built-in");
                        }
                    });
                }
                ui.horizontal(|ui| {
                    if ui.button("+ Operation profile").clicked() {
                        let profile = self.profiles.add_operation(OperationProfile::new(
                            "New operation",
                            OperationType::OutlineCutting,
                        ));
                        self.selected_operation = Some(profile.id.clone());
                        changed = true;
                    }
                    if ui
                        .add_enabled(
                            self.selected_operation.is_some(),
                            egui::Button::new("Duplicate"),
                        )
                        .clicked()
                    {
                        if let Some(id) = self.selected_operation.clone() {
                            if let Some(copy) = self.profiles.duplicate_operation(&id) {
                                self.selected_operation = Some(copy);
                            }
                        }
                    }
                    let removable = self
                        .selected_operation
                        .as_deref()
                        .and_then(|id| self.profiles.operation(id))
                        .is_some_and(|profile| !profile.builtin);
                    if ui
                        .add_enabled(removable, egui::Button::new("Delete"))
                        .clicked()
                    {
                        if let Some(id) = self.selected_operation.clone() {
                            match self.profiles.remove_operation(&id) {
                                Ok(()) => {
                                    self.selected_operation = None;
                                    changed = true;
                                }
                                Err(error) => self.profile_message = error,
                            }
                        }
                    }
                });
                if let Some(id) = self.selected_operation.clone() {
                    if let Some(profile) = self
                        .profiles
                        .operations
                        .iter_mut()
                        .find(|profile| profile.id == id)
                    {
                        let editable = !profile.builtin;
                        ui.add_enabled_ui(editable, |ui| {
                            ui.horizontal(|ui| {
                                ui.label("Name");
                                changed |= ui.text_edit_singleline(&mut profile.name).changed();
                            });
                            let mut op_type = profile.op_type;
                            egui::ComboBox::from_label("Type")
                                .selected_text(op_type.label())
                                .show_ui(ui, |ui| {
                                    for candidate in OperationType::all() {
                                        ui.selectable_value(
                                            &mut op_type,
                                            candidate,
                                            candidate.label(),
                                        );
                                    }
                                });
                            if op_type != profile.op_type {
                                profile.op_type = op_type;
                                changed = true;
                            }
                            ui.horizontal(|ui| {
                                ui.label("Speed");
                                changed |= ui
                                    .add(
                                        egui::DragValue::new(&mut profile.params.speed)
                                            .speed(10.0)
                                            .range(1.0..=20000.0)
                                            .suffix(" mm/min"),
                                    )
                                    .changed();
                            });
                            ui.horizontal(|ui| {
                                ui.label("Power");
                                changed |= ui
                                    .add(
                                        egui::DragValue::new(&mut profile.params.power)
                                            .speed(1.0)
                                            .range(0.0..=100.0)
                                            .suffix(" %"),
                                    )
                                    .changed();
                            });
                            ui.horizontal(|ui| {
                                ui.label("Passes");
                                changed |= ui
                                    .add(
                                        egui::DragValue::new(&mut profile.params.passes)
                                            .speed(1.0)
                                            .range(1..=100),
                                    )
                                    .changed();
                                ui.label("Interval");
                                changed |= ui
                                    .add(
                                        egui::DragValue::new(
                                            &mut profile.params.line_interval_mm,
                                        )
                                        .speed(0.01)
                                        .range(0.01..=10.0)
                                        .suffix(" mm"),
                                    )
                                    .changed();
                            });
                        });
                        if !editable {
                            ui.weak("Built-in profile: duplicate it to edit.");
                        }
                        let profile_clone = profile.clone();
                        if ui
                            .add_enabled(
                                !self.editor.selection.is_empty(),
                                egui::Button::new("Apply to selection"),
                            )
                            .clicked()
                        {
                            let count = self.editor.apply_operation_profile(&profile_clone);
                            self.status =
                                format!("Applied '{}' to {count} object(s)", profile_clone.name);
                        }
                    }
                }
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label("File");
                    ui.text_edit_singleline(&mut self.profile_io_path);
                });
                ui.horizontal(|ui| {
                    if ui.button("Save profiles").clicked() {
                        self.save_profiles();
                    }
                    if ui.button("Import").clicked() {
                        let path = std::path::PathBuf::from(self.profile_io_path.trim());
                        match crate::profiles::import_into(&mut self.profiles, &path) {
                            Ok(added) => {
                                self.profile_message = format!("Imported {added} profile(s)");
                                changed = true;
                            }
                            Err(error) => self.profile_message = format!("Import failed: {error}"),
                        }
                    }
                    if ui.button("Export").clicked() {
                        let path = std::path::PathBuf::from(self.profile_io_path.trim());
                        match crate::profiles::export(&self.profiles, &path) {
                            Ok(()) => {
                                self.profile_message =
                                    format!("Exported profiles to {}", path.display());
                            }
                            Err(error) => self.profile_message = format!("Export failed: {error}"),
                        }
                    }
                });
                if !self.profile_message.is_empty() {
                    ui.weak(&self.profile_message);
                }
            });
        self.profiles_open = open;
        if changed {
            self.save_profiles();
        }
    }

    fn dialogs(&mut self, ctx: &egui::Context) {
        if let Some(dialog) = &mut self.dialog {
            let result = dialog.show(ctx);
            match result {
                DialogResult::Pending => {}
                DialogResult::Cancelled => self.dialog = None,
                DialogResult::Accepted(path) => {
                    let mode = dialog.mode;
                    self.dialog = None;
                    self.handle_file(mode, &path);
                }
            }
        }
        if self.confirm.is_some() {
            let mut action = None;
            let mut cancel = false;
            egui::Window::new("Unsaved changes")
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label("This document has unsaved changes.");
                    ui.label("Discard them and continue?");
                    ui.horizontal(|ui| {
                        if ui.button("Discard").clicked() {
                            action = self.confirm;
                        }
                        if ui.button("Cancel").clicked() {
                            cancel = true;
                        }
                    });
                });
            if let Some(action) = action {
                self.confirm = None;
                self.perform(action);
            } else if cancel {
                self.confirm = None;
            }
        }
        if self.profiles_open {
            self.profiles_window(ctx);
        }
        if self.about_open {
            let mut open = true;
            egui::Window::new("About BeamWright")
                .collapsible(false)
                .resizable(false)
                .open(&mut open)
                .show(ctx, |ui| {
                    ui.heading("BeamWright");
                    ui.label(format!("Version {}", env!("CARGO_PKG_VERSION")));
                    ui.label("Vector drawing and laser job preparation.");
                    ui.label("This build prepares jobs only; it does not control machines.");
                    ui.label("MIT licensed.");
                });
            self.about_open = open;
        }
    }
}

impl eframe::App for BeamWrightApp {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if ctx.input(|input| input.viewport().close_requested())
            && self.is_dirty()
            && !self.quit_requested
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.confirm = Some(PendingAction::Quit);
        }
        ui.ctx().set_visuals(if self.dark_mode {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        });
        self.shortcuts(ui);
        self.menu_bar(ui);
        self.toolbar(ui);
        self.status_bar(ui);
        self.layers_panel(ui);
        self.properties_panel(ui);
        self.canvas(ui);
        let ctx = ui.ctx().clone();
        self.dialogs(&ctx);
        if self.quit_requested {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let title = format!("BeamWright - {}", self.title());
        let current = ctx.input(|input| input.viewport().title.clone());
        if current.as_deref() != Some(title.as_str()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title));
        }
    }
}

fn edit_color(ui: &mut Ui, color: &mut Option<String>) {
    ui.horizontal(|ui| {
        let mut enabled = color.is_some();
        if ui.checkbox(&mut enabled, "").changed() {
            *color = if enabled {
                Some("#000000".to_string())
            } else {
                None
            };
        }
        let Some(value) = color.as_mut() else {
            return;
        };
        let rgb = parse_hex_color(value).unwrap_or((0, 0, 0));
        let mut srgb = [rgb.0, rgb.1, rgb.2];
        if ui.color_edit_button_srgb(&mut srgb).changed() {
            *value = to_hex(srgb[0], srgb[1], srgb[2]);
        }
        let response = ui.add(
            egui::TextEdit::singleline(value)
                .desired_width(76.0)
                .hint_text("#rrggbb"),
        );
        if response.lost_focus() {
            if let Some(normalized) = normalize_hex_color(value) {
                *value = normalized;
            }
        }
        if normalize_hex_color(value).is_none() {
            ui.colored_label(Color32::from_rgb(200, 60, 60), "?");
        }
    });
}

fn sanitize_name(name: &str) -> String {
    let cleaned: String = name
        .trim()
        .chars()
        .map(|character| {
            if character.is_alphanumeric() || character == '-' || character == '_' {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let cleaned = cleaned.trim_matches('-').to_string();
    if cleaned.is_empty() {
        "untitled".to_string()
    } else {
        cleaned
    }
}
