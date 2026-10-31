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
use crate::report::FileReport;
use crate::ui::file_dialog::{DialogResult, FileDialog, FileMode};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PendingAction {
    New,
    Open,
    Quit,
}

#[derive(Debug, Clone)]
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
    Sketch {
        points: Vec<Point2>,
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
    selected_image_profile: Option<String>,
    selected_project_profile: Option<String>,
    file_report: Option<FileReport>,
    report_open: bool,
    preview_toolpaths: bool,
    image_previews: std::collections::HashMap<String, ImagePreview>,
    preferences: crate::config::preferences::AppPreferences,
    preferences_path: PathBuf,
    preferences_open: bool,
    polygon_sides: usize,
    grid_array_open: bool,
    offset_open: bool,
    offset_distance: f64,
    grid_array: (usize, usize, f64, f64),
    search_query: String,
    preview_open: bool,
    playback: Option<crate::gcode::Playback>,
    playback_progress: f64,
    playback_playing: bool,
    playback_speed: f32,
    playback_error: Option<String>,
    gcode_text: String,
    preview_snapshot: Option<Document>,
}

struct ImagePreview {
    texture: egui::TextureHandle,
    path: String,
    settings: crate::objects::ImageSettings,
}

impl Default for BeamWrightApp {
    fn default() -> Self {
        Self::new(None)
    }
}

impl BeamWrightApp {
    pub fn new(file: Option<PathBuf>) -> Self {
        let document = Document::default();
        let preferences_path = crate::config::preferences::default_path();
        let preferences =
            crate::config::preferences::AppPreferences::load_or_default(&preferences_path)
                .unwrap_or_default();
        let mut editor = EditorState::new(document.clone());
        editor.canvas.grid_spacing_mm = preferences.drawing.grid_spacing_mm;
        editor.snap_step_mm = preferences.drawing.snap_step_mm;
        editor.snap_enabled = preferences.drawing.snap_enabled;
        let dark_mode = preferences.general.theme == "dark";
        let mut app = Self {
            editor,
            dark_mode,
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
            selected_image_profile: None,
            selected_project_profile: None,
            file_report: None,
            report_open: false,
            preview_toolpaths: false,
            image_previews: std::collections::HashMap::new(),
            preferences: preferences.clone(),
            preferences_path,
            preferences_open: false,
            polygon_sides: 6,
            grid_array_open: false,
            offset_open: false,
            offset_distance: 1.0,
            grid_array: (3, 2, 20.0, 20.0),
            search_query: String::new(),
            preview_open: false,
            playback: None,
            playback_progress: 0.0,
            playback_playing: false,
            playback_speed: 1.0,
            playback_error: None,
            gcode_text: String::new(),
            preview_snapshot: None,
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
            FileMode::ExportPng => format!("{stem}.png"),
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
            FileMode::ExportPng => {
                match crate::export::export_png(
                    &self.editor.document,
                    path,
                    &crate::export::PngOptions::default(),
                ) {
                    Ok(report) => {
                        let file_report = FileReport::new("Export PNG", path.display().to_string())
                            .with_summary("Width", format!("{} px", report.width_px))
                            .with_summary("Height", format!("{} px", report.height_px))
                            .with_summary("Resolution", "96 dpi")
                            .with_warnings(report.warnings);
                        self.status = file_report.headline();
                        self.show_report(file_report);
                    }
                    Err(error) => self.status = format!("Export failed: {error}"),
                }
            }
            FileMode::ExportDxf => match crate::export::export_dxf(&self.editor.document) {
                Ok(dxf) => match std::fs::write(path, dxf) {
                    Ok(()) => {
                        let report = FileReport::new("Export DXF", path.display().to_string())
                            .with_summary("Objects", self.editor.document.objects.len().to_string())
                            .with_summary("Format", "LWPOLYLINE subset");
                        self.status = report.headline();
                        self.show_report(report);
                    }
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
                        let file_report =
                            FileReport::new("Export G-code", path.display().to_string())
                                .with_summary("Paths", report.paths.to_string())
                                .with_summary("Lines", report.lines.to_string())
                                .with_summary("Bytes", report.bytes.to_string())
                                .with_summary("Estimated", format!("{:.1} s", report.seconds))
                                .with_warnings(report.warnings);
                        self.status = file_report.headline();
                        self.show_report(file_report);
                    }
                    Err(error) => self.status = format!("Export failed: {error}"),
                }
            }
            FileMode::ExportSvg => match crate::export::export_svg(&self.editor.document) {
                Ok(svg) => match std::fs::write(path, svg) {
                    Ok(()) => {
                        let report = FileReport::new("Export SVG", path.display().to_string())
                            .with_summary(
                                "Objects",
                                self.editor.document.objects.len().to_string(),
                            );
                        self.status = report.headline();
                        self.show_report(report);
                    }
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
                let object_count = document.objects.len();
                let layer_count = document.layers.len();
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
                let report = FileReport::new(
                    if is_project { "Open project" } else { "Import" },
                    path.display().to_string(),
                )
                .with_summary(
                    "Format",
                    crate::import::format::detect_format(path).to_uppercase(),
                )
                .with_summary("Objects", object_count.to_string())
                .with_summary("Layers", layer_count.to_string())
                .with_warnings(outcome.warnings);
                self.status = report.headline();
                self.show_report(report);
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
            group,
            ungroup,
            escape,
            fit,
            grid,
            polygon_tool,
            pencil_tool,
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
                input.consume_key(Modifiers::CTRL, Key::G),
                input.consume_key(Modifiers::CTRL | Modifiers::SHIFT, Key::G),
                input.consume_key(Modifiers::NONE, Key::Delete)
                    || input.consume_key(Modifiers::NONE, Key::Backspace),
                input.consume_key(Modifiers::NONE, Key::Escape),
                input.consume_key(Modifiers::NONE, Key::F),
                input.consume_key(Modifiers::NONE, Key::G),
                input.consume_key(Modifiers::NONE, Key::P),
                input.consume_key(Modifiers::NONE, Key::B),
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
        if group && self.editor.group_selection() {
            self.status = "Objects grouped".to_string();
        }
        if ungroup && self.editor.ungroup_selection() {
            self.status = "Objects ungrouped".to_string();
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
        if polygon_tool {
            self.editor.tool = Tool::Polygon;
        }
        if pencil_tool {
            self.editor.tool = Tool::Pencil;
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
                    if ui.button("Export PNG...").clicked() {
                        self.open_dialog(FileMode::ExportPng);
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
                    ui.separator();
                    if ui
                        .add_enabled(self.editor.selection.len() >= 2, egui::Button::new("Group"))
                        .clicked()
                    {
                        self.editor.group_selection();
                        ui.close();
                    }
                    if ui
                        .add_enabled(
                            !self.editor.selection.is_empty(),
                            egui::Button::new("Ungroup"),
                        )
                        .clicked()
                    {
                        self.editor.ungroup_selection();
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
                    ui.separator();
                    if ui
                        .add_enabled(enabled, egui::Button::new("Grid array..."))
                        .clicked()
                    {
                        self.grid_array_open = true;
                        ui.close();
                    }
                    ui.separator();
                    let can_combine = self.editor.selection.len() == 2;
                    ui.menu_button("Boolean", |ui| {
                        use crate::geometry::BooleanOp;
                        for op in BooleanOp::all() {
                            if ui
                                .add_enabled(can_combine, egui::Button::new(op.label()))
                                .clicked()
                            {
                                self.apply_boolean(op);
                                ui.close();
                            }
                        }
                    });
                    if ui
                        .add_enabled(enabled, egui::Button::new("Offset..."))
                        .clicked()
                    {
                        self.offset_open = true;
                        ui.close();
                    }
                });
                ui.menu_button("Align", |ui| {
                    use crate::editor::alignment::AlignMode;
                    let enabled = !self.editor.selection.is_empty();
                    for (mode, label) in [
                        (AlignMode::Left, "Align left"),
                        (AlignMode::CenterH, "Align centre horizontally"),
                        (AlignMode::Right, "Align right"),
                        (AlignMode::Top, "Align top"),
                        (AlignMode::CenterV, "Align centre vertically"),
                        (AlignMode::Bottom, "Align bottom"),
                    ] {
                        if ui.add_enabled(enabled, egui::Button::new(label)).clicked() {
                            self.editor.align_selection(mode);
                            ui.close();
                        }
                    }
                    ui.separator();
                    if ui
                        .add_enabled(
                            self.editor.selection.len() >= 3,
                            egui::Button::new("Distribute horizontally"),
                        )
                        .clicked()
                    {
                        self.editor.distribute_selection(true);
                        ui.close();
                    }
                    if ui
                        .add_enabled(
                            self.editor.selection.len() >= 3,
                            egui::Button::new("Distribute vertically"),
                        )
                        .clicked()
                    {
                        self.editor.distribute_selection(false);
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
                    ui.checkbox(&mut self.preview_toolpaths, "Show toolpaths");
                    if ui.button("Preview and G-code...").clicked() {
                        self.open_preview();
                        ui.close();
                    }
                    ui.checkbox(&mut self.dark_mode, "Dark mode");
                    ui.separator();
                    if ui.button("Preferences...").clicked() {
                        self.preferences_open = true;
                        ui.close();
                    }
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
                    Tool::Polygon,
                    Tool::Pencil,
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
                if self.editor.tool == Tool::Polygon {
                    ui.add(
                        egui::DragValue::new(&mut self.polygon_sides)
                            .speed(1.0)
                            .range(3..=64),
                    );
                    ui.label("sides");
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
                ui.checkbox(&mut self.preview_toolpaths, "Toolpaths");
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
                ui.heading("Search");
                ui.text_edit_singleline(&mut self.search_query);
                let hits = crate::editor::search::search_document(
                    &self.editor.document,
                    &self.search_query,
                );
                if !hits.is_empty() {
                    egui::ScrollArea::vertical()
                        .max_height(120.0)
                        .show(ui, |ui| {
                            for hit in &hits {
                                if ui.button(&hit.label).clicked() {
                                    match hit.kind {
                                        crate::editor::search::SearchKind::Object => {
                                            self.editor.selection.clear();
                                            self.editor.selection.add(hit.id.clone());
                                        }
                                        crate::editor::search::SearchKind::Layer => {
                                            self.editor.active_layer_id.clone_from(&hit.id);
                                        }
                                        crate::editor::search::SearchKind::Operation => {
                                            self.active_operation = Some(hit.id.clone());
                                        }
                                    }
                                }
                            }
                        });
                }
                ui.separator();
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
                if ui
                    .small_button("\u{2191}")
                    .on_hover_text("Run this operation earlier")
                    .clicked()
                {
                    self.editor.move_operation(id, -1);
                }
                if ui
                    .small_button("\u{2193}")
                    .on_hover_text("Run this operation later")
                    .clicked()
                {
                    self.editor.move_operation(id, 1);
                }
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
                use crate::objects::ImageMode;
                ui.label("Image");
                ui.label(&image.path);
                ui.weak(format!(
                    "{:.1} x {:.1} mm at {:.0} dpi",
                    image.width_mm(),
                    image.height_mm(),
                    image.dpi
                ));
                ui.separator();
                ui.label("Preparation (used by G-code export)");
                let mut mode = image.settings.mode;
                egui::ComboBox::from_label("Mode")
                    .selected_text(mode.label())
                    .show_ui(ui, |ui| {
                        for candidate in ImageMode::all() {
                            ui.selectable_value(&mut mode, candidate, candidate.label());
                        }
                    });
                image.settings.mode = mode;
                ui.add(egui::Slider::new(&mut image.settings.threshold, 0..=255).text("Threshold"));
                ui.add(
                    egui::Slider::new(&mut image.settings.brightness, -100..=100)
                        .text("Brightness"),
                );
                ui.add(
                    egui::Slider::new(&mut image.settings.contrast, 0.25..=3.0).text("Contrast"),
                );
                ui.add(egui::Slider::new(&mut image.settings.gamma, 0.2..=3.0).text("Gamma"));
                ui.checkbox(&mut image.settings.invert, "Invert");
                ui.add(
                    egui::DragValue::new(&mut image.settings.line_interval_mm)
                        .speed(0.01)
                        .range(0.01..=2.0)
                        .suffix(" mm line interval"),
                );
                if let Err(error) = image.settings.validate() {
                    ui.colored_label(Color32::from_rgb(200, 60, 60), error);
                }
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
                            Tool::Rect
                            | Tool::Ellipse
                            | Tool::Line
                            | Tool::Polygon
                            | Tool::Pencil => {
                                if self.editor.layer_locked(&self.editor.active_layer_id)
                                    || !self.editor.layer_visible(&self.editor.active_layer_id)
                                {
                                    self.status =
                                        "Cannot draw on a hidden or locked layer".to_string();
                                } else if self.editor.tool == Tool::Pencil {
                                    self.drag = Some(DragState::Sketch {
                                        points: vec![point],
                                    });
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
                    Some(DragState::Sketch { points }) => {
                        Some(PreviewShape::Polyline(points.clone()))
                    }
                    _ => None,
                };
                let theme = Theme {
                    dark: self.dark_mode,
                };
                let painter = ui.painter_at(rect);
                let toolpaths = if self.preview_toolpaths {
                    Some(crate::gcode::build_toolpaths(&self.editor.document).0)
                } else {
                    None
                };
                self.refresh_image_previews(&ctx);
                let textures: crate::editor::renderer::ImageTextures = self
                    .image_previews
                    .iter()
                    .map(|(id, preview)| (id.clone(), preview.texture.id()))
                    .collect();
                let playback_overlay = if self.preview_open {
                    self.playback
                        .as_ref()
                        .map(|playback| (playback, self.playback_progress))
                } else {
                    None
                };
                let overlays = crate::editor::renderer::CanvasOverlays {
                    toolpaths: toolpaths.as_deref(),
                    images: Some(&textures),
                    playback: playback_overlay,
                };
                paint_canvas(&painter, rect, &self.editor, theme, preview, &overlays);

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

    fn refresh_image_previews(&mut self, ctx: &egui::Context) {
        use crate::document::Shape;
        use crate::objects::ImageSettings;

        let mut needed: Vec<(String, String, ImageSettings)> = Vec::new();
        for object in &self.editor.document.objects {
            if let Shape::Image(image) = &object.shape {
                needed.push((object.id.clone(), image.path.clone(), image.settings));
            }
        }
        self.image_previews.retain(|id, preview| {
            needed.iter().any(|(needed_id, path, settings)| {
                needed_id == id && preview.path == *path && preview.settings == *settings
            })
        });
        for (id, path, settings) in needed {
            if self.image_previews.contains_key(&id) {
                continue;
            }
            let Ok(bitmap) = crate::raster::process_file(std::path::Path::new(&path), &settings)
            else {
                continue;
            };
            let color = preview_color_image(&bitmap);
            let texture = ctx.load_texture(
                format!("image-preview-{id}"),
                color,
                egui::TextureOptions::NEAREST,
            );
            self.image_previews.insert(
                id,
                ImagePreview {
                    texture,
                    path,
                    settings,
                },
            );
        }
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
            DragState::Sketch { points } => {
                if points.last().is_none_or(|last| last.distance(point) >= 0.5) {
                    points.push(point);
                }
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
                Tool::Polygon => {
                    self.editor.add_polygon(
                        Point2::new(f64::midpoint(start.x, end.x), f64::midpoint(start.y, end.y)),
                        end,
                        self.polygon_sides,
                    );
                }
                _ => {}
            },
            DragState::Sketch { points } => {
                if self.editor.add_polyline(&points, false).is_some() {
                    self.status = "Freehand path created".to_string();
                }
            }
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

    fn advance_playback(&mut self, ctx: &egui::Context) {
        if self.preview_open
            && self.preview_snapshot.as_ref() != Some(&self.editor.document)
            && !ctx.input(|input| input.pointer.any_down())
        {
            self.refresh_preview();
        }
        if !self.playback_playing {
            return;
        }
        let Some(playback) = &self.playback else {
            self.playback_playing = false;
            return;
        };
        let delta = f64::from(ctx.input(|input| input.stable_dt).min(0.1));
        let rate = if playback.duration_seconds > 0.0 {
            playback.total_length / playback.duration_seconds
        } else {
            0.0
        };
        let advance = delta * rate * f64::from(self.playback_speed);
        self.playback_progress = (self.playback_progress + advance).min(playback.total_length);
        if self.playback_progress >= playback.total_length {
            self.playback_playing = false;
        }
        ctx.request_repaint();
    }

    fn open_preview(&mut self) {
        self.preview_open = true;
        self.refresh_preview();
    }

    /// Rebuild the simulated job and the G-code text from the document.
    fn refresh_preview(&mut self) {
        let options = crate::gcode::JobOptions::default();
        match crate::gcode::render_job(&self.editor.document, &options) {
            Ok((text, _report)) => {
                let (toolpaths, _) =
                    crate::gcode::planned_toolpaths(&self.editor.document, &options);
                let playback =
                    crate::gcode::Playback::from_toolpaths(&toolpaths, options.travel_speed_mm_min);
                self.playback_progress = self.playback_progress.min(playback.total_length);
                self.playback = Some(playback);
                self.playback_error = None;
                self.gcode_text = text;
            }
            Err(error) => {
                self.playback = None;
                self.playback_error = Some(error.to_string());
                self.gcode_text.clear();
            }
        }
        self.preview_snapshot = Some(self.editor.document.clone());
    }

    fn preview_window(&mut self, ctx: &egui::Context) {
        let mut open = self.preview_open;
        let (total, duration, position) = match &self.playback {
            Some(playback) => (
                playback.total_length,
                playback.duration_seconds,
                playback.position_at(self.playback_progress),
            ),
            None => (0.0, 0.0, None),
        };
        egui::Window::new("Preview and G-code")
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(520.0)
            .show(ctx, |ui| {
                if let Some(error) = self.playback_error.clone() {
                    ui.colored_label(
                        Color32::from_rgb(190, 60, 50),
                        format!("Cannot build the job: {error}"),
                    );
                    if ui.button("Refresh").clicked() {
                        self.refresh_preview();
                    }
                    return;
                }
                ui.horizontal(|ui| {
                    let play_label = if self.playback_playing {
                        "Pause"
                    } else {
                        "Play"
                    };
                    if ui.button(play_label).clicked() {
                        if !self.playback_playing && self.playback_progress >= total && total > 0.0
                        {
                            self.playback_progress = 0.0;
                        }
                        self.playback_playing = !self.playback_playing;
                    }
                    if ui.button("Stop").clicked() {
                        self.playback_playing = false;
                        self.playback_progress = 0.0;
                    }
                    ui.add(
                        egui::Slider::new(&mut self.playback_progress, 0.0..=total.max(0.001))
                            .show_value(false),
                    );
                    ui.label(format!("{:.1} / {:.1} mm", self.playback_progress, total));
                });
                ui.horizontal(|ui| {
                    ui.label("Speed");
                    ui.add(
                        egui::Slider::new(&mut self.playback_speed, 0.25..=16.0)
                            .logarithmic(true)
                            .suffix("x"),
                    );
                    if ui.button("Refresh").clicked() {
                        self.refresh_preview();
                    }
                });
                if let Some(position) = position {
                    ui.label(format!(
                        "Pass {} - {} - S{:.0} - ({:.2}, {:.2}) mm",
                        position.pass,
                        match position.kind {
                            crate::gcode::MoveKind::Cut => "cut",
                            crate::gcode::MoveKind::Travel => "travel",
                        },
                        position.power,
                        position.point.x,
                        position.point.y
                    ));
                } else {
                    ui.weak("The document has no toolpaths yet.");
                }
                ui.weak(format!("Estimated job time: {duration:.1} s"));
                ui.separator();
                ui.heading("G-code");
                let mut text = self.gcode_text.clone();
                egui::ScrollArea::vertical()
                    .max_height(260.0)
                    .show(ui, |ui| {
                        ui.add(
                            egui::TextEdit::multiline(&mut text)
                                .code_editor()
                                .desired_rows(18)
                                .desired_width(f32::INFINITY),
                        );
                    });
                if ui.button("Copy G-code").clicked() {
                    ui.ctx().copy_text(self.gcode_text.clone());
                }
            });
        self.preview_open = open;
    }

    fn show_report(&mut self, report: FileReport) {
        self.file_report = Some(report);
        self.report_open = true;
    }

    fn report_window(&mut self, ctx: &egui::Context) {
        let Some(report) = self.file_report.clone() else {
            self.report_open = false;
            return;
        };
        let mut open = self.report_open;
        egui::Window::new(format!("{} report", report.action))
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(460.0)
            .show(ctx, |ui| {
                ui.label(&report.path);
                egui::Grid::new("file_report_summary")
                    .num_columns(2)
                    .spacing([12.0, 4.0])
                    .show(ui, |ui| {
                        for (label, value) in &report.summary {
                            ui.label(label);
                            ui.label(value);
                            ui.end_row();
                        }
                    });
                ui.separator();
                if report.is_clean() {
                    ui.colored_label(Color32::from_rgb(40, 140, 60), "Nothing was skipped.");
                } else {
                    ui.heading(format!("Warnings ({})", report.warning_count()));
                    egui::ScrollArea::vertical()
                        .max_height(220.0)
                        .show(ui, |ui| {
                            for warning in &report.warnings {
                                ui.colored_label(Color32::from_rgb(180, 120, 0), warning);
                            }
                        });
                }
                if ui.button("Close").clicked() {
                    self.report_open = false;
                }
            });
        self.report_open = open && self.report_open;
        if !self.report_open {
            self.file_report = None;
        }
    }

    fn apply_boolean(&mut self, op: crate::geometry::BooleanOp) {
        match self.editor.boolean_selection(op) {
            Ok(count) => {
                self.status = if count == 1 {
                    format!("{} applied", op.label())
                } else {
                    format!("{} applied ({count} shapes)", op.label())
                };
            }
            Err(error) => self.status = format!("{} failed: {error}", op.label()),
        }
    }

    fn offset_window(&mut self, ctx: &egui::Context) {
        let mut open = self.offset_open;
        let mut apply = false;
        egui::Window::new("Offset")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label("Distance");
                ui.add(
                    egui::DragValue::new(&mut self.offset_distance)
                        .speed(0.1)
                        .range(-100.0..=100.0)
                        .suffix(" mm"),
                );
                ui.weak("Positive expands the outline; negative contracts it.");
                ui.weak("Shapes that collapse are removed.");
                if ui.button("Apply").clicked() {
                    apply = true;
                }
            });
        if apply {
            match self.editor.offset_selection(self.offset_distance) {
                Ok(count) => {
                    self.status = format!("Offset applied to {count} shape(s)");
                    self.offset_open = false;
                }
                Err(error) => self.status = format!("Offset failed: {error}"),
            }
        } else {
            self.offset_open = open;
        }
    }

    fn grid_array_window(&mut self, ctx: &egui::Context) {
        let mut open = self.grid_array_open;
        egui::Window::new("Grid array")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                let (columns, rows, dx, dy) = &mut self.grid_array;
                ui.horizontal(|ui| {
                    ui.label("Columns");
                    ui.add(egui::DragValue::new(columns).speed(1.0).range(1..=100));
                    ui.label("Rows");
                    ui.add(egui::DragValue::new(rows).speed(1.0).range(1..=100));
                });
                ui.horizontal(|ui| {
                    ui.label("Spacing X");
                    ui.add(
                        egui::DragValue::new(dx)
                            .speed(0.5)
                            .range(-10000.0..=10000.0)
                            .suffix(" mm"),
                    );
                    ui.label("Y");
                    ui.add(
                        egui::DragValue::new(dy)
                            .speed(0.5)
                            .range(-10000.0..=10000.0)
                            .suffix(" mm"),
                    );
                });
                ui.label(format!(
                    "Creates {} copies of the {} selected object(s).",
                    columns.saturating_mul(*rows).saturating_sub(1),
                    self.editor.selection.len()
                ));
                if ui
                    .add_enabled(
                        !self.editor.selection.is_empty(),
                        egui::Button::new("Create array"),
                    )
                    .clicked()
                {
                    let created = self.editor.duplicate_grid(*columns, *rows, *dx, *dy);
                    self.status = format!("Created {created} copies");
                    self.grid_array_open = false;
                }
            });
        self.grid_array_open = open && self.grid_array_open;
    }

    fn preferences_window(&mut self, ctx: &egui::Context) {
        let mut open = self.preferences_open;
        let mut changed = false;
        egui::Window::new("Preferences")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.heading("Drawing");
                changed |= ui
                    .add(
                        egui::DragValue::new(&mut self.preferences.drawing.grid_spacing_mm)
                            .speed(0.5)
                            .range(0.1..=1000.0)
                            .suffix(" mm grid"),
                    )
                    .changed();
                changed |= ui
                    .checkbox(
                        &mut self.preferences.drawing.snap_enabled,
                        "Snap to grid step",
                    )
                    .changed();
                changed |= ui
                    .add(
                        egui::DragValue::new(&mut self.preferences.drawing.snap_step_mm)
                            .speed(0.1)
                            .range(0.1..=100.0)
                            .suffix(" mm snap"),
                    )
                    .changed();
                ui.separator();
                ui.heading("Appearance");
                let mut dark = self.preferences.general.theme == "dark";
                if ui.checkbox(&mut dark, "Dark theme").changed() {
                    self.preferences.general.theme =
                        if dark { "dark" } else { "light" }.to_string();
                    changed = true;
                }
                if changed {
                    self.editor.canvas.grid_spacing_mm = self.preferences.drawing.grid_spacing_mm;
                    self.editor.snap_step_mm = self.preferences.drawing.snap_step_mm;
                    self.editor.snap_enabled = self.preferences.drawing.snap_enabled;
                    self.dark_mode = self.preferences.general.theme == "dark";
                    match self.preferences.save(&self.preferences_path) {
                        Ok(()) => self.status = "Preferences saved".to_string(),
                        Err(error) => self.status = format!("Cannot save preferences: {error}"),
                    }
                }
            });
        self.preferences_open = open;
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
                ui.heading("Image profiles");
                let image_profiles: Vec<_> = self
                    .profiles
                    .images
                    .iter()
                    .filter(|profile| {
                        query.is_empty()
                            || format!("{} {}", profile.name, profile.notes)
                                .to_lowercase()
                                .contains(&query)
                    })
                    .map(|profile| (profile.id.clone(), profile.name.clone(), profile.builtin))
                    .collect();
                for (id, name, builtin) in &image_profiles {
                    ui.horizontal(|ui| {
                        let selected = self.selected_image_profile.as_deref() == Some(id.as_str());
                        if ui.selectable_label(selected, name).clicked() {
                            self.selected_image_profile = Some(id.clone());
                        }
                        if *builtin {
                            ui.weak("built-in");
                        }
                    });
                }
                ui.horizontal(|ui| {
                    if ui.button("+ Image profile").clicked() {
                        let profile = self
                            .profiles
                            .add_image(crate::profiles::ImageProfile::new("New image profile"));
                        self.selected_image_profile = Some(profile.id.clone());
                        changed = true;
                    }
                    if ui
                        .add_enabled(
                            self.selected_image_profile.is_some(),
                            egui::Button::new("Duplicate"),
                        )
                        .clicked()
                    {
                        if let Some(id) = self.selected_image_profile.clone() {
                            if let Some(copy) = self.profiles.duplicate_image(&id) {
                                self.selected_image_profile = Some(copy);
                            }
                        }
                    }
                    let removable = self
                        .selected_image_profile
                        .as_deref()
                        .and_then(|id| self.profiles.image(id))
                        .is_some_and(|profile| !profile.builtin);
                    if ui
                        .add_enabled(removable, egui::Button::new("Delete"))
                        .clicked()
                    {
                        if let Some(id) = self.selected_image_profile.clone() {
                            match self.profiles.remove_image(&id) {
                                Ok(()) => {
                                    self.selected_image_profile = None;
                                    changed = true;
                                }
                                Err(error) => self.profile_message = error,
                            }
                        }
                    }
                });
                if let Some(id) = self.selected_image_profile.clone() {
                    if let Some(profile) = self
                        .profiles
                        .images
                        .iter_mut()
                        .find(|profile| profile.id == id)
                    {
                        let editable = !profile.builtin;
                        ui.add_enabled_ui(editable, |ui| {
                            ui.horizontal(|ui| {
                                ui.label("Name");
                                changed |= ui.text_edit_singleline(&mut profile.name).changed();
                            });
                            let mut mode = profile.settings.mode;
                            egui::ComboBox::from_label("Mode")
                                .selected_text(mode.label())
                                .show_ui(ui, |ui| {
                                    for candidate in crate::objects::ImageMode::all() {
                                        ui.selectable_value(
                                            &mut mode,
                                            candidate,
                                            candidate.label(),
                                        );
                                    }
                                });
                            if mode != profile.settings.mode {
                                profile.settings.mode = mode;
                                changed = true;
                            }
                            changed |= ui
                                .add(
                                    egui::Slider::new(&mut profile.settings.threshold, 0..=255)
                                        .text("Threshold"),
                                )
                                .changed();
                            changed |= ui
                                .add(
                                    egui::Slider::new(
                                        &mut profile.settings.brightness,
                                        -100..=100,
                                    )
                                    .text("Brightness"),
                                )
                                .changed();
                            changed |= ui
                                .add(
                                    egui::Slider::new(&mut profile.settings.contrast, 0.25..=3.0)
                                        .text("Contrast"),
                                )
                                .changed();
                            changed |= ui
                                .add(
                                    egui::Slider::new(&mut profile.settings.gamma, 0.2..=3.0)
                                        .text("Gamma"),
                                )
                                .changed();
                            changed |= ui
                                .checkbox(&mut profile.settings.invert, "Invert")
                                .changed();
                            changed |= ui
                                .add(
                                    egui::DragValue::new(&mut profile.settings.line_interval_mm)
                                        .speed(0.01)
                                        .range(0.01..=2.0)
                                        .suffix(" mm"),
                                )
                                .changed();
                        });
                        if !editable {
                            ui.weak("Built-in profile: duplicate it to edit.");
                        }
                        let profile_clone = profile.clone();
                        if ui
                            .add_enabled(
                                !self.editor.selection.is_empty(),
                                egui::Button::new("Apply to selected images"),
                            )
                            .clicked()
                        {
                            let count = self.editor.apply_image_profile(&profile_clone);
                            self.status =
                                format!("Applied image profile to {count} object(s)");
                        }
                    }
                }
                ui.separator();
                ui.heading("Project profiles");
                let project_profiles: Vec<_> = self
                    .profiles
                    .projects
                    .iter()
                    .map(|profile| (profile.id.clone(), profile.name.clone(), profile.builtin))
                    .collect();
                for (id, name, builtin) in &project_profiles {
                    ui.horizontal(|ui| {
                        let selected =
                            self.selected_project_profile.as_deref() == Some(id.as_str());
                        if ui.selectable_label(selected, name).clicked() {
                            self.selected_project_profile = Some(id.clone());
                        }
                        if *builtin {
                            ui.weak("built-in");
                        }
                    });
                }
                ui.horizontal(|ui| {
                    if ui.button("+ Project profile").clicked() {
                        let profile = self.profiles.add_project(
                            crate::profiles::ProjectProfile::new("New project", 300.0, 200.0),
                        );
                        self.selected_project_profile = Some(profile.id.clone());
                        changed = true;
                    }
                    if ui
                        .add_enabled(
                            self.selected_project_profile.is_some(),
                            egui::Button::new("Duplicate"),
                        )
                        .clicked()
                    {
                        if let Some(id) = self.selected_project_profile.clone() {
                            if let Some(copy) = self.profiles.duplicate_project(&id) {
                                self.selected_project_profile = Some(copy);
                            }
                        }
                    }
                    let removable = self
                        .selected_project_profile
                        .as_deref()
                        .and_then(|id| self.profiles.project(id))
                        .is_some_and(|profile| !profile.builtin);
                    if ui
                        .add_enabled(removable, egui::Button::new("Delete"))
                        .clicked()
                    {
                        if let Some(id) = self.selected_project_profile.clone() {
                            match self.profiles.remove_project(&id) {
                                Ok(()) => {
                                    self.selected_project_profile = None;
                                    changed = true;
                                }
                                Err(error) => self.profile_message = error,
                            }
                        }
                    }
                });
                if let Some(id) = self.selected_project_profile.clone() {
                    if let Some(profile) = self
                        .profiles
                        .projects
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
                                ui.label("Width");
                                changed |= ui
                                    .add(
                                        egui::DragValue::new(&mut profile.width_mm)
                                            .speed(1.0)
                                            .range(1.0..=10000.0)
                                            .suffix(" mm"),
                                    )
                                    .changed();
                                ui.label("Height");
                                changed |= ui
                                    .add(
                                        egui::DragValue::new(&mut profile.height_mm)
                                            .speed(1.0)
                                            .range(1.0..=10000.0)
                                            .suffix(" mm"),
                                    )
                                    .changed();
                            });
                            ui.horizontal(|ui| {
                                ui.label("Grid");
                                changed |= ui
                                    .add(
                                        egui::DragValue::new(&mut profile.grid_spacing_mm)
                                            .speed(0.5)
                                            .range(0.1..=100.0)
                                            .suffix(" mm"),
                                    )
                                    .changed();
                                ui.label("Snap");
                                changed |= ui
                                    .add(
                                        egui::DragValue::new(&mut profile.snap_mm)
                                            .speed(0.1)
                                            .range(0.1..=100.0)
                                            .suffix(" mm"),
                                    )
                                    .changed();
                            });
                        });
                        if !editable {
                            ui.weak("Built-in profile: duplicate it to edit.");
                        }
                        let profile_clone = profile.clone();
                        if ui.button("Apply to project").clicked() {
                            if self.editor.apply_project_profile(&profile_clone) {
                                self.needs_fit = true;
                                self.status =
                                    format!("Applied project profile '{}'", profile_clone.name);
                            } else {
                                self.status = "Project profile is invalid".to_string();
                            }
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
        if self.preferences_open {
            self.preferences_window(ctx);
        }
        if self.grid_array_open {
            self.grid_array_window(ctx);
        }
        if self.offset_open {
            self.offset_window(ctx);
        }
        if self.report_open {
            self.report_window(ctx);
        }
        if self.preview_open {
            self.preview_window(ctx);
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
        self.advance_playback(ctx);
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

/// Downscale a burn bitmap to a preview texture (white paper, black burn).
fn preview_color_image(bitmap: &crate::raster::RasterBitmap) -> egui::ColorImage {
    const MAX_SIDE: usize = 320;
    let step = ((bitmap.width.max(bitmap.height) as f32) / MAX_SIDE as f32)
        .ceil()
        .max(1.0) as usize;
    let width = (bitmap.width / step).max(1);
    let height = (bitmap.height / step).max(1);
    let mut pixels = Vec::with_capacity(width * height * 4);
    for y in 0..height {
        for x in 0..width {
            let burned = bitmap.get(
                (x * step).min(bitmap.width - 1),
                (y * step).min(bitmap.height - 1),
            );
            let value = if burned { 0 } else { 255 };
            pixels.extend_from_slice(&[value, value, value, 255]);
        }
    }
    egui::ColorImage::from_rgba_unmultiplied([width, height], &pixels)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_marks_burn_black() {
        let bitmap = crate::raster::RasterBitmap {
            width: 2,
            height: 1,
            burn: vec![true, false],
        };
        let image = preview_color_image(&bitmap);
        assert_eq!(image.size, [2, 1]);
        assert_eq!(image.pixels[0], egui::Color32::BLACK);
        assert_eq!(image.pixels[1], egui::Color32::WHITE);
    }

    #[test]
    fn preview_downscales_large_bitmaps() {
        let bitmap = crate::raster::RasterBitmap {
            width: 1000,
            height: 500,
            burn: vec![false; 500_000],
        };
        let image = preview_color_image(&bitmap);
        assert!(image.size[0] <= 320);
        assert!(image.size[1] <= 320);
        assert_eq!(image.size[0], 250);
    }

    #[test]
    fn preview_handles_single_pixel() {
        let bitmap = crate::raster::RasterBitmap {
            width: 1,
            height: 1,
            burn: vec![true],
        };
        let image = preview_color_image(&bitmap);
        assert_eq!(image.size, [1, 1]);
        assert_eq!(image.pixels[0], egui::Color32::BLACK);
    }
}
