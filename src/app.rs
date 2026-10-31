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
use crate::ui::icons::{self, Icon};

/// Result produced by a background task.
#[derive(Debug)]
enum TaskResult {
    Import {
        path: PathBuf,
        outcome: std::result::Result<crate::import::format::ImportOutcome, String>,
    },
    Export(std::result::Result<FileReport, String>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PendingAction {
    New,
    Open,
    Quit,
}

/// Entry offered by the canvas context menus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ContextAction {
    Cut,
    Copy,
    Paste,
    Duplicate,
    Delete,
    ToFront,
    ToBack,
    Group,
    Ungroup,
    Lock,
    Unlock,
    EditNodes,
    TextToOutlines,
    AdjustImage,
    ExportSvg,
    ExportDxf,
    ExportGcode,
    SelectAll,
    ZoomIn,
    ZoomOut,
    FitPage,
    ZoomSelection,
    Guides,
    DocumentProperties,
    Preview,
    Preferences,
}

impl ContextAction {
    fn label(self) -> &'static str {
        match self {
            ContextAction::Cut => "Cut",
            ContextAction::Copy => "Copy",
            ContextAction::Paste => "Paste",
            ContextAction::Duplicate => "Duplicate",
            ContextAction::Delete => "Delete",
            ContextAction::ToFront => "Bring to front",
            ContextAction::ToBack => "Send to back",
            ContextAction::Group => "Group",
            ContextAction::Ungroup => "Ungroup",
            ContextAction::Lock => "Lock",
            ContextAction::Unlock => "Unlock",
            ContextAction::EditNodes => "Edit nodes",
            ContextAction::TextToOutlines => "Text to outlines",
            ContextAction::AdjustImage => "Adjust image...",
            ContextAction::ExportSvg => "Export selection as SVG...",
            ContextAction::ExportDxf => "Export selection as DXF...",
            ContextAction::ExportGcode => "Export selection as G-code...",
            ContextAction::SelectAll => "Select all",
            ContextAction::ZoomIn => "Zoom in",
            ContextAction::ZoomOut => "Zoom out",
            ContextAction::FitPage => "Fit page",
            ContextAction::ZoomSelection => "Zoom to selection",
            ContextAction::Guides => "Guides...",
            ContextAction::DocumentProperties => "Document properties...",
            ContextAction::Preview => "Preview and G-code...",
            ContextAction::Preferences => "Preferences...",
        }
    }

    fn icon(self) -> Icon {
        match self {
            ContextAction::Cut => Icon::Cut,
            ContextAction::Copy => Icon::Copy,
            ContextAction::Paste => Icon::Paste,
            ContextAction::Duplicate => Icon::Duplicate,
            ContextAction::Delete => Icon::Delete,
            ContextAction::ToFront => Icon::ToFront,
            ContextAction::ToBack => Icon::ToBack,
            ContextAction::Group => Icon::Group,
            ContextAction::Ungroup => Icon::Ungroup,
            ContextAction::Lock => Icon::Lock,
            ContextAction::Unlock => Icon::Unlock,
            ContextAction::EditNodes => Icon::Nodes,
            ContextAction::TextToOutlines => Icon::TextOutline,
            ContextAction::AdjustImage => Icon::Image,
            ContextAction::ExportSvg | ContextAction::ExportDxf | ContextAction::ExportGcode => {
                Icon::Export
            }
            ContextAction::SelectAll => Icon::SelectAll,
            ContextAction::ZoomIn => Icon::ZoomIn,
            ContextAction::ZoomOut => Icon::ZoomOut,
            ContextAction::FitPage => Icon::ZoomFit,
            ContextAction::ZoomSelection => Icon::ZoomSelection,
            ContextAction::Guides => Icon::Guides,
            ContextAction::DocumentProperties => Icon::Page,
            ContextAction::Preview => Icon::Play,
            ContextAction::Preferences => Icon::Preferences,
        }
    }
}

/// Entries of the context menu over the selected objects.
fn object_context_actions(
    count: usize,
    text_selected: bool,
    image_selected: bool,
) -> Vec<ContextAction> {
    let mut actions = vec![
        ContextAction::Cut,
        ContextAction::Copy,
        ContextAction::Duplicate,
        ContextAction::Delete,
        ContextAction::ToFront,
        ContextAction::ToBack,
    ];
    if count >= 2 {
        actions.push(ContextAction::Group);
    }
    actions.push(ContextAction::Ungroup);
    actions.push(ContextAction::Lock);
    actions.push(ContextAction::Unlock);
    if count == 1 {
        actions.push(ContextAction::EditNodes);
    }
    if text_selected {
        actions.push(ContextAction::TextToOutlines);
    }
    if image_selected {
        actions.push(ContextAction::AdjustImage);
    }
    actions.extend([
        ContextAction::ExportSvg,
        ContextAction::ExportDxf,
        ContextAction::ExportGcode,
    ]);
    actions
}

/// Entries of the context menu over empty work area.
fn canvas_context_actions() -> Vec<ContextAction> {
    let mut actions = vec![ContextAction::Paste, ContextAction::SelectAll];
    actions.extend([
        ContextAction::ZoomIn,
        ContextAction::ZoomOut,
        ContextAction::FitPage,
        ContextAction::ZoomSelection,
        ContextAction::Guides,
        ContextAction::DocumentProperties,
        ContextAction::Preview,
        ContextAction::Preferences,
    ]);
    actions
}

/// Which part of the selection frame is being dragged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HandleKind {
    Move,
    /// Index into the eight frame handles, clockwise from the top-left.
    Scale(usize),
    Rotate,
}

#[derive(Debug, Clone)]
enum DragState {
    Move {
        last: Point2,
        recorded: bool,
    },
    NodeMove {
        last: Point2,
        recorded: bool,
    },
    Handle {
        node: usize,
        outgoing: bool,
        recorded: bool,
    },
    Draw {
        tool: Tool,
        start: Point2,
        end: Point2,
        /// Shift: square/circle for rectangles and ellipses, 45° steps for lines.
        constrain: bool,
    },
    Marquee {
        start: Point2,
        end: Point2,
        additive: bool,
    },
    Sketch {
        points: Vec<Point2>,
    },
    Transform {
        kind: HandleKind,
        last: Point2,
        centre: Point2,
        recorded: bool,
        constrain: bool,
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
    profile_favorites_only: bool,
    profile_category_filter: String,
    profile_sort_by_date: bool,
    profile_tag_buffer: String,
    profile_tag_owner: String,
    selected_image_profile: Option<String>,
    selected_project_profile: Option<String>,
    file_report: Option<FileReport>,
    report_open: bool,
    preview_toolpaths: bool,
    image_previews: std::collections::HashMap<String, ImagePreview>,
    image_original_previews: std::collections::HashMap<String, egui::TextureHandle>,
    image_adjust: Option<String>,
    image_adjust_snapshot: Option<Document>,
    machines: crate::machines::MachineLibrary,
    machines_path: PathBuf,
    machines_open: bool,
    machine_wizard_open: bool,
    machine_editor: crate::machines::MachineProfile,
    machine_edit_id: Option<String>,
    machine_error: Option<String>,
    doc_properties_snapshot: Option<Document>,
    cuts_open: bool,
    cuts_snapshot: Option<Document>,
    help_shortcuts_open: bool,
    multi_props_recorded: bool,
    smart_guides: Vec<(bool, f64)>,
    cuts_drag: Option<usize>,
    cuts_drag_target: Option<usize>,
    bridge: Option<crate::bridge::LiveBridge>,
    live_server: crate::mcp::McpServer,
    preferences: crate::config::preferences::AppPreferences,
    preferences_path: PathBuf,
    preferences_open: bool,
    polygon_sides: usize,
    grid_array_open: bool,
    offset_open: bool,
    offset_distance: f64,
    star_points: usize,
    star_inner: f64,
    repeat_open: bool,
    repeat_options: crate::editor::RepeatOptions,
    clipboard: Vec<crate::document::Object>,
    context_object: Option<String>,
    context_point: Option<Point2>,
    operation_params: Option<crate::operations::OperationParams>,
    link_proportions: bool,
    skew_open: bool,
    skew_x: f64,
    skew_y: f64,
    simplify_open: bool,
    simplify_tolerance: f64,
    nest_open: bool,
    nest_spacing: f64,
    nest_margin: f64,
    nest_rotation: bool,
    nest_proposal: Option<crate::editor::NestProposal>,
    nest_message: String,
    grid_array: crate::editor::GridOptions,
    search_query: String,
    preview_open: bool,
    playback: Option<crate::gcode::Playback>,
    playback_progress: f64,
    playback_playing: bool,
    playback_speed: f32,
    playback_error: Option<String>,
    gcode_text: String,
    preview_snapshot: Option<Document>,
    preview_line_numbers: bool,
    preview_order: bool,
    show_layers_panel: bool,
    show_properties_panel: bool,
    doc_properties_open: bool,
    operations_compare_open: bool,
    profile_pending: Option<(String, crate::profiles::OperationProfile)>,
    relink_target: Option<String>,
    pending_file: Option<(FileMode, PathBuf)>,
    recovery_path: PathBuf,
    last_autosave: std::time::Instant,
    recovery_prompt_open: bool,
    recovery_timestamp: String,
    task: Option<crate::task::BackgroundTask<TaskResult>>,
    guides_open: bool,
    guide_position: f64,
    exact_size: bool,
    exact_width: f64,
    exact_height: f64,
    profile_confirm_open: bool,
    zoom_to_selection_request: bool,
    export_subset: bool,
    recover_next: bool,
    pen_nodes: Vec<crate::geometry::path::Node>,
    pen_drag_start: Option<Point2>,
}

/// Build one export and write it, reporting what it contains.
///
/// Runs on a worker thread; `cancel` is checked before writing so a cancelled
/// export never creates or replaces a file.
fn run_export(
    mode: FileMode,
    path: &Path,
    document: &crate::document::Document,
    cancel: &std::sync::atomic::AtomicBool,
) -> std::result::Result<FileReport, String> {
    let cancelled = || {
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            Err("Cancelled".to_string())
        } else {
            Ok(())
        }
    };
    match mode {
        FileMode::ExportPng => {
            cancelled()?;
            let report =
                crate::export::export_png(document, path, &crate::export::PngOptions::default())
                    .map_err(|error| error.to_string())?;
            Ok(FileReport::new("Export PNG", path.display().to_string())
                .with_summary("Width", format!("{} px", report.width_px))
                .with_summary("Height", format!("{} px", report.height_px))
                .with_summary("Resolution", "96 dpi")
                .with_warnings(report.warnings))
        }
        FileMode::ExportDxf => {
            let dxf = crate::export::export_dxf(document).map_err(|error| error.to_string())?;
            cancelled()?;
            std::fs::write(path, dxf).map_err(|error| error.to_string())?;
            Ok(FileReport::new("Export DXF", path.display().to_string())
                .with_summary("Objects", document.objects.len().to_string())
                .with_summary("Format", "LWPOLYLINE subset"))
        }
        FileMode::ExportGcode => {
            cancelled()?;
            let cancelled_flag = || cancel.load(std::sync::atomic::Ordering::Relaxed);
            let report = crate::gcode::export_job_cancellable(
                document,
                path,
                &crate::gcode::JobOptions::default(),
                &cancelled_flag,
            )
            .map_err(|error| error.to_string())?;
            Ok(FileReport::new("Export G-code", path.display().to_string())
                .with_summary("Paths", report.paths.to_string())
                .with_summary("Lines", report.lines.to_string())
                .with_summary("Bytes", report.bytes.to_string())
                .with_summary("Estimated", format!("{:.1} s", report.seconds))
                .with_warnings(report.warnings))
        }
        FileMode::ExportSvg => {
            let svg = crate::export::export_svg(document).map_err(|error| error.to_string())?;
            cancelled()?;
            std::fs::write(path, svg).map_err(|error| error.to_string())?;
            Ok(FileReport::new("Export SVG", path.display().to_string())
                .with_summary("Objects", document.objects.len().to_string()))
        }
        _ => Err("unsupported export mode".to_string()),
    }
}

/// Short description shown when hovering a toolbar tool.
fn tool_help(tool: Tool) -> &'static str {
    match tool {
        Tool::Select => "Select and move objects (V)",
        Tool::Line => "Draw a line; Shift snaps to 45° (L)",
        Tool::Rect => "Draw a rectangle; Shift for a square (R)",
        Tool::Ellipse => "Draw an ellipse; Shift for a circle (E)",
        Tool::Polygon => "Draw a regular polygon (P)",
        Tool::Star => "Draw a star with adjustable tips (S)",
        Tool::Text => "Insert text (T)",
        Tool::Measure => "Measure a distance and angle",
        Tool::Bezier => "Draw a Bézier path node by node",
        Tool::Pencil => "Draw a freehand path (B)",
        Tool::Pan => "Pan the canvas",
    }
}

/// Icon shown by each tool in the toolbar and Draw menu.
fn alignment_snap(
    document: &Document,
    selection: &[String],
    bounds: Option<(Point2, Point2)>,
    zoom: f64,
    dx: f64,
    dy: f64,
) -> (f64, f64, Vec<(bool, f64)>) {
    let Some((min, max)) = bounds else {
        return (dx, dy, Vec::new());
    };
    let tolerance = 6.0 / zoom.max(0.01);
    let selected: std::collections::HashSet<&str> = selection.iter().map(String::as_str).collect();
    let mut verticals = Vec::new();
    let mut horizontals = Vec::new();
    for object in &document.objects {
        if selected.contains(object.id.as_str()) || !object.visible || object.locked {
            continue;
        }
        let Some((other_min, other_max)) = object.bounds() else {
            continue;
        };
        verticals.extend([
            other_min.x,
            f64::midpoint(other_min.x, other_max.x),
            other_max.x,
        ]);
        horizontals.extend([
            other_min.y,
            f64::midpoint(other_min.y, other_max.y),
            other_max.y,
        ]);
    }
    let current_x = [min.x + dx, f64::midpoint(min.x, max.x) + dx, max.x + dx];
    let current_y = [min.y + dy, f64::midpoint(min.y, max.y) + dy, max.y + dy];
    let mut guides = Vec::new();
    let snap = |lines: [f64; 3], candidates: &[f64], guides: &mut Vec<f64>| -> f64 {
        let mut best: Option<(f64, f64)> = None;
        for line in lines {
            for candidate in candidates {
                let difference = candidate - line;
                if difference.abs() <= tolerance {
                    if best.is_none_or(|(best, _)| difference.abs() < best.abs()) {
                        best = Some((difference, *candidate));
                    }
                }
            }
        }
        match best {
            Some((difference, coordinate)) => {
                guides.push(coordinate);
                difference
            }
            None => 0.0,
        }
    };
    let adjust_x = snap(current_x, &verticals, &mut guides);
    let adjust_y = snap(current_y, &horizontals, &mut guides);
    guides.sort_by(|left, right| left.total_cmp(right));
    guides.dedup_by(|left, right| (*left - *right).abs() < 1e-9);
    let mut result = Vec::new();
    for guide in guides {
        if verticals.iter().any(|value| (value - guide).abs() < 1e-9) {
            result.push((true, guide));
        }
        if horizontals.iter().any(|value| (value - guide).abs() < 1e-9) {
            result.push((false, guide));
        }
    }
    (dx + adjust_x, dy + adjust_y, result)
}

fn boolean_icon(op: crate::geometry::BooleanOp) -> Icon {
    match op {
        crate::geometry::BooleanOp::Union => Icon::BooleanUnion,
        crate::geometry::BooleanOp::Difference => Icon::BooleanDifference,
        crate::geometry::BooleanOp::Intersection => Icon::BooleanIntersection,
        crate::geometry::BooleanOp::Xor => Icon::BooleanExclusion,
    }
}

fn tool_icon(tool: Tool) -> Icon {
    match tool {
        Tool::Select => Icon::Select,
        Tool::Line => Icon::Line,
        Tool::Rect => Icon::Rectangle,
        Tool::Ellipse => Icon::Ellipse,
        Tool::Polygon => Icon::Polygon,
        Tool::Star => Icon::Star,
        Tool::Text => Icon::Text,
        Tool::Measure => Icon::Measure,
        Tool::Bezier => Icon::Bezier,
        Tool::Pencil => Icon::Freehand,
        Tool::Pan => Icon::Pan,
    }
}

/// Scale around a point, for direct property edits.
fn egui_transform_scale(
    factor_x: f64,
    factor_y: f64,
    center: Point2,
) -> crate::geometry::Transform2 {
    crate::geometry::Transform2::scale_around(factor_x, factor_y, center.x, center.y)
}

/// One row in a profile list.
struct ProfileRow {
    id: String,
    name: String,
    builtin: bool,
    favorite: bool,
    category: String,
    modified_at: String,
    tags: Vec<String>,
}

/// Search, filter and sort settings shared by the four profile lists.
#[derive(Clone)]
struct ProfileFilter {
    query: String,
    favorites_only: bool,
    category: String,
    sort_by_date: bool,
}

fn profile_rows<T>(
    profiles: &[T],
    parts: impl Fn(&T) -> (&str, &str, bool, &crate::profiles::ProfileMeta),
    extra: impl Fn(&T) -> String,
    filter: &ProfileFilter,
) -> Vec<ProfileRow> {
    let mut rows: Vec<ProfileRow> = profiles
        .iter()
        .filter_map(|profile| {
            let (id, name, builtin, meta) = parts(profile);
            let row = ProfileRow {
                id: id.to_string(),
                name: name.to_string(),
                builtin,
                favorite: meta.favorite,
                category: meta.category.clone(),
                modified_at: meta.modified_at.clone(),
                tags: meta.tags.clone(),
            };
            let text = format!("{} {}", row.name, extra(profile)).to_lowercase();
            let query_matches = filter.query.is_empty()
                || text.contains(&filter.query)
                || row.category.to_lowercase().contains(&filter.query)
                || row
                    .tags
                    .iter()
                    .any(|tag| tag.to_lowercase().contains(&filter.query));
            (query_matches
                && (!filter.favorites_only || row.favorite)
                && (filter.category.is_empty() || row.category == filter.category))
                .then_some(row)
        })
        .collect();
    if filter.sort_by_date {
        rows.sort_by(|left, right| right.modified_at.cmp(&left.modified_at));
    } else {
        rows.sort_by_key(|row| row.name.to_lowercase());
    }
    rows
}

/// Edit the category, tags and favourite mark of a profile.
///
/// `editable` is false for built-in profiles, whose other fields are read-only;
/// marking a built-in as a favourite is still allowed.
fn profile_meta_editor(
    ui: &mut egui::Ui,
    id: &str,
    meta: &mut crate::profiles::ProfileMeta,
    editable: bool,
    tag_buffer: &mut String,
    tag_owner: &mut String,
) -> bool {
    if tag_owner != id {
        *tag_owner = id.to_string();
        *tag_buffer = meta.tag_text();
    }
    let mut changed = ui.checkbox(&mut meta.favorite, "Favourite").changed();
    ui.add_enabled_ui(editable, |ui| {
        ui.horizontal(|ui| {
            ui.label("Category");
            changed |= ui.text_edit_singleline(&mut meta.category).changed();
            egui::ComboBox::from_id_salt(format!("category_suggest_{id}"))
                .selected_text("Suggest")
                .width(90.0)
                .show_ui(ui, |ui| {
                    for category in crate::profiles::DEFAULT_CATEGORIES {
                        if ui
                            .selectable_label(meta.category == category, category)
                            .clicked()
                        {
                            meta.category = category.to_string();
                            changed = true;
                        }
                    }
                });
        });
        ui.horizontal(|ui| {
            ui.label("Tags");
            if ui.text_edit_singleline(tag_buffer).changed() {
                meta.set_tag_text(tag_buffer);
                changed = true;
            }
            ui.weak("comma separated");
        });
    });
    let created = if meta.created_at.is_empty() {
        "unknown".to_string()
    } else {
        meta.created_at.clone()
    };
    let modified = if meta.modified_at.is_empty() {
        "unknown".to_string()
    } else {
        meta.modified_at.clone()
    };
    ui.weak(format!("Created {created} - modified {modified}"));
    if changed && editable {
        meta.touch();
    }
    changed
}

struct ImagePreview {
    texture: egui::TextureHandle,
    path: String,
    settings: crate::objects::ImageSettings,
    crop: Option<crate::objects::ImageCrop>,
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
        editor.snap_nodes = preferences.drawing.snap_nodes;
        editor.snap_endpoints = preferences.drawing.snap_endpoints;
        editor.snap_centers = preferences.drawing.snap_centers;
        editor.snap_midpoints = preferences.drawing.snap_midpoints;
        editor.snap_intersections = preferences.drawing.snap_intersections;
        editor.snap_limits = preferences.drawing.snap_limits;
        editor.show_rulers = preferences.drawing.show_rulers;
        editor.show_selection = preferences.drawing.show_selection;
        editor
            .selection_color
            .clone_from(&preferences.drawing.selection_color);
        editor.handle_style =
            crate::editor::state::HandleStyle::from_label(&preferences.drawing.handle_style);
        editor.zoom_step = preferences.drawing.zoom_step;
        editor.select_whole_group = preferences.drawing.select_whole_group;
        editor.scroll_zoom_step = preferences.drawing.scroll_zoom_step;
        editor.guides_visible = preferences.drawing.guides_visible;
        editor.snap_guides = preferences.drawing.snap_guides;
        let dark_mode = preferences.general.theme == "dark";
        let machines_path = crate::machines::default_path();
        let machines =
            crate::machines::MachineLibrary::load_or_default(&machines_path).unwrap_or_default();
        let machine_wizard_open = !machines.first_run_done;
        let recovery_path = crate::config::paths::recovery_path();
        let recovery_timestamp = std::fs::metadata(&recovery_path)
            .and_then(|metadata| metadata.modified())
            .ok()
            .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
            .map_or_else(String::new, |duration| {
                crate::profiles::model::format_utc(duration.as_secs())
            });
        let recovery_prompt_open = recovery_path.is_file();
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
            profile_favorites_only: false,
            profile_category_filter: String::new(),
            profile_sort_by_date: false,
            profile_tag_buffer: String::new(),
            profile_tag_owner: String::new(),
            profile_io_path: crate::profiles::default_path()
                .to_string_lossy()
                .to_string(),
            selected_image_profile: None,
            selected_project_profile: None,
            file_report: None,
            report_open: false,
            preview_toolpaths: false,
            image_previews: std::collections::HashMap::new(),
            image_original_previews: std::collections::HashMap::new(),
            image_adjust: None,
            image_adjust_snapshot: None,
            machines,
            machines_path,
            machines_open: false,
            machine_wizard_open,
            machine_editor: crate::machines::MachineProfile::default(),
            machine_edit_id: None,
            machine_error: None,
            doc_properties_snapshot: None,
            cuts_open: false,
            cuts_snapshot: None,
            help_shortcuts_open: false,
            multi_props_recorded: false,
            smart_guides: Vec::new(),
            cuts_drag: None,
            cuts_drag_target: None,
            bridge: crate::bridge::LiveBridge::start(crate::bridge::configured_port()),
            live_server: crate::mcp::McpServer::new(),
            preferences: preferences.clone(),
            preferences_path,
            preferences_open: false,
            polygon_sides: 6,
            grid_array_open: false,
            offset_open: false,
            offset_distance: 1.0,
            star_points: 5,
            star_inner: 0.5,
            repeat_open: false,
            repeat_options: crate::editor::RepeatOptions::default(),
            clipboard: Vec::new(),
            context_object: None,
            context_point: None,
            operation_params: None,
            link_proportions: false,
            skew_open: false,
            skew_x: 0.0,
            skew_y: 0.0,
            simplify_open: false,
            simplify_tolerance: 0.2,
            nest_open: false,
            nest_spacing: 1.0,
            nest_margin: 0.0,
            nest_rotation: true,
            nest_proposal: None,
            nest_message: String::new(),
            grid_array: crate::editor::GridOptions {
                columns: 3,
                rows: 2,
                dx: 20.0,
                dy: 20.0,
                ..crate::editor::GridOptions::default()
            },
            search_query: String::new(),
            preview_open: false,
            playback: None,
            playback_progress: 0.0,
            playback_playing: false,
            playback_speed: 1.0,
            playback_error: None,
            gcode_text: String::new(),
            preview_snapshot: None,
            preview_line_numbers: false,
            preview_order: false,
            show_layers_panel: true,
            show_properties_panel: true,
            doc_properties_open: false,
            operations_compare_open: false,
            profile_pending: None,
            relink_target: None,
            pending_file: None,
            recovery_path,
            last_autosave: std::time::Instant::now(),
            recovery_prompt_open,
            recovery_timestamp,
            task: None,
            guides_open: false,
            guide_position: 50.0,
            exact_size: false,
            exact_width: 20.0,
            exact_height: 10.0,
            profile_confirm_open: false,
            zoom_to_selection_request: false,
            export_subset: false,
            recover_next: false,
            pen_nodes: Vec::new(),
            pen_drag_start: None,
        };
        if let Some(path) = file {
            app.open_path(&path);
        }
        if let Some(bridge) = &app.bridge {
            app.status = format!(
                "Ready - live bridge on 127.0.0.1:{} (BeamWright mcp --live)",
                bridge.port()
            );
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
        let mut document = Document::default();
        if let Some(unit) = Unit::from_label(self.preferences.general.default_unit.trim()) {
            document.unit = unit;
        }
        self.editor.set_document(document.clone());
        self.saved_snapshot = Some(document);
        self.current_path = None;
        self.needs_fit = true;
        self.drag = None;
        let _ = std::fs::remove_file(&self.recovery_path);
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
            FileMode::RelinkImage => "image.png".to_string(),
            FileMode::ImportProfiles | FileMode::ExportProfiles => "profiles.json".to_string(),
            FileMode::ImportMachines | FileMode::ExportMachines => "machines.json".to_string(),
            FileMode::OpenProject | FileMode::SaveProject => format!("{stem}.bwp"),
        };
        let default_directory = self.preferences.general.default_directory.trim();
        let start = self.current_path.clone().or_else(|| {
            if default_directory.is_empty() {
                None
            } else {
                Some(std::path::PathBuf::from(default_directory))
            }
        });
        self.dialog = Some(FileDialog::new(mode, start.as_deref(), &suggested));
    }

    fn handle_file(&mut self, mode: FileMode, path: &Path) {
        if self.recover_next && matches!(mode, FileMode::OpenProject) {
            self.recover_next = false;
            self.recover_project(path);
            return;
        }
        let selection_only = self.export_subset && !self.editor.selection.is_empty();
        self.export_subset = false;
        let document = if selection_only {
            self.editor.document.subset(self.editor.selection.ids())
        } else {
            self.editor.document.clone()
        };
        match mode {
            FileMode::OpenProject | FileMode::ImportFile => self.start_import(path),
            FileMode::ImportProfiles => {
                match crate::profiles::library::import_into(&mut self.profiles, path) {
                    Ok(count) => {
                        match crate::profiles::library::save(&self.profiles, &self.profiles_path) {
                            Ok(()) => {
                                self.status = format!("Imported {count} profile(s)");
                            }
                            Err(error) => {
                                self.status = format!("Cannot save the profile library: {error}");
                            }
                        }
                    }
                    Err(error) => self.status = format!("Profile import failed: {error}"),
                }
            }
            FileMode::ExportProfiles => {
                match crate::profiles::library::export(&self.profiles, path) {
                    Ok(()) => self.status = format!("Exported profiles to {}", path.display()),
                    Err(error) => self.status = format!("Profile export failed: {error}"),
                }
            }
            FileMode::ImportMachines => match self.machines.import_into(path) {
                Ok(count) => match self.machines.save(&self.machines_path) {
                    Ok(()) => {
                        self.status = format!("Imported {count} machine profile(s)");
                    }
                    Err(error) => {
                        self.status = format!("Cannot save the machine library: {error}");
                    }
                },
                Err(error) => self.status = format!("Machine import failed: {error}"),
            },
            FileMode::ExportMachines => match self.machines.export_to(path) {
                Ok(()) => {
                    self.status = format!("Exported machine profiles to {}", path.display());
                }
                Err(error) => self.status = format!("Machine export failed: {error}"),
            },
            FileMode::SaveProject => self.save_to(path),
            FileMode::ExportPng
            | FileMode::ExportDxf
            | FileMode::ExportGcode
            | FileMode::ExportSvg => self.start_export(mode, path, document),
            FileMode::RelinkImage => self.relink_image(path),
        }
    }

    fn relink_image(&mut self, path: &Path) {
        let Some(object_id) = self.relink_target.take() else {
            self.status = "No image selected for relinking".to_string();
            return;
        };
        match crate::raster::image_dimensions(path) {
            Ok((width, height)) => {
                match self.editor.relink_image(&object_id, path, width, height) {
                    Ok(()) => {
                        self.status = format!("Relinked image to {}", path.display());
                    }
                    Err(error) => self.status = format!("Relink failed: {error}"),
                }
            }
            Err(error) => self.status = format!("Relink failed: {error}"),
        }
    }

    fn collect_images(&mut self) {
        let Some(project) = self.current_path.clone() else {
            self.status = "Save the project before collecting its images".to_string();
            return;
        };
        let stem = project
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("project");
        let directory = project
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(format!("{stem}_assets"));
        let has_images = self
            .editor
            .document
            .objects
            .iter()
            .any(|object| matches!(object.shape, crate::document::Shape::Image(_)));
        if !has_images {
            self.status = "The project has no images".to_string();
            return;
        }
        self.editor.record_action("Collect images");
        match self.editor.document.collect_images(&directory) {
            Ok(report) => {
                let mut file_report =
                    FileReport::new("Collect images", directory.display().to_string())
                        .with_summary("Copied", report.collected.len().to_string())
                        .with_warnings(report.missing.clone());
                if report.missing.is_empty() {
                    file_report.summary.push((
                        "Next".to_string(),
                        "save the project to keep the new paths".to_string(),
                    ));
                }
                self.status = file_report.headline();
                self.show_report(file_report);
            }
            Err(error) => {
                self.status = format!("Collect failed: {error}");
            }
        }
    }

    fn recover_project(&mut self, path: &Path) {
        let backup = crate::document::serialization::backup_path(path);
        match crate::document::serialization::load_from_file(&backup) {
            Ok(document) => {
                self.editor.set_document(document);
                self.current_path = Some(path.to_path_buf());
                self.saved_snapshot = None;
                self.needs_fit = true;
                let report = FileReport::new("Recover project", path.display().to_string())
                    .with_summary("Backup", backup.display().to_string())
                    .with_summary("Objects", self.editor.document.objects.len().to_string())
                    .with_warnings(vec![
                        "Recovered the previous version; the current project file was not read."
                            .to_string(),
                    ]);
                self.status = format!("Recovered {}", backup.display());
                self.show_report(report);
            }
            Err(error) => {
                self.status = format!("Recovery failed: {error}");
            }
        }
    }

    fn open_document_properties(&mut self) {
        self.doc_properties_snapshot = Some(self.editor.document.clone());
        self.doc_properties_open = true;
    }

    fn document_properties_window(&mut self, ctx: &egui::Context) {
        use crate::document::Unit;

        let mut open = self.doc_properties_open;
        let mut changed = false;
        egui::Window::new("Document properties")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                egui::Grid::new("document_properties_window")
                    .num_columns(2)
                    .spacing([12.0, 4.0])
                    .show(ui, |ui| {
                        ui.label("Name");
                        changed |= ui
                            .text_edit_singleline(&mut self.editor.document.name)
                            .changed();
                        ui.end_row();
                        ui.label("Width");
                        changed |= ui
                            .add(
                                egui::DragValue::new(&mut self.editor.document.width_mm)
                                    .speed(1.0)
                                    .range(10.0..=100_000.0)
                                    .suffix(" mm"),
                            )
                            .changed();
                        ui.end_row();
                        ui.label("Height");
                        changed |= ui
                            .add(
                                egui::DragValue::new(&mut self.editor.document.height_mm)
                                    .speed(1.0)
                                    .range(10.0..=100_000.0)
                                    .suffix(" mm"),
                            )
                            .changed();
                        ui.end_row();
                        ui.label("Unit");
                        let mut unit = self.editor.document.unit;
                        egui::ComboBox::from_id_salt("document_unit")
                            .selected_text(unit.label())
                            .show_ui(ui, |ui| {
                                for candidate in [
                                    Unit::Millimeters,
                                    Unit::Centimeters,
                                    Unit::Inches,
                                    Unit::Pixels,
                                ] {
                                    if ui
                                        .selectable_value(&mut unit, candidate, candidate.label())
                                        .changed()
                                    {
                                        changed = true;
                                    }
                                }
                            });
                        self.editor.document.unit = unit;
                        ui.end_row();
                        ui.label("Origin");
                        ui.label(format!("{:?}", self.editor.document.origin));
                        ui.end_row();
                        ui.label("Layers");
                        ui.label(self.editor.document.layers.len().to_string());
                        ui.end_row();
                        ui.label("Objects");
                        ui.label(self.editor.document.objects.len().to_string());
                        ui.end_row();
                        ui.label("Operations");
                        ui.label(self.editor.document.operations.len().to_string());
                        ui.end_row();
                        ui.label("Format version");
                        ui.label(self.editor.document.format_version.to_string());
                        ui.end_row();
                        ui.label("File");
                        ui.label(self.current_path.as_deref().map_or_else(
                            || "not saved yet".to_string(),
                            |path| path.display().to_string(),
                        ));
                        ui.end_row();
                    });
            });
        if changed {
            self.needs_fit = true;
        }
        if !open || !self.doc_properties_open {
            self.doc_properties_open = open;
        }
        if !open {
            if let Some(snapshot) = self.doc_properties_snapshot.take() {
                if snapshot != self.editor.document {
                    self.editor.history.record(&snapshot, "Document properties");
                }
            }
            self.doc_properties_open = false;
        }
    }

    fn operations_compare_window(&mut self, ctx: &egui::Context) {
        let mut open = self.operations_compare_open;
        egui::Window::new("Compare operations")
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(520.0)
            .show(ctx, |ui| {
                if self.editor.document.operations.is_empty() {
                    ui.weak("No operations yet.");
                    return;
                }
                egui::Grid::new("operations_compare")
                    .num_columns(7)
                    .striped(true)
                    .spacing([10.0, 4.0])
                    .show(ui, |ui| {
                        for heading in [
                            "Name", "Type", "Speed", "Power", "Passes", "Interval", "Enabled",
                        ] {
                            ui.strong(heading);
                        }
                        ui.end_row();
                        for operation in &self.editor.document.operations {
                            ui.label(&operation.name);
                            ui.label(operation.op_type.label());
                            ui.label(format!("{:.0} mm/min", operation.params.speed));
                            ui.label(format!("{:.0} %", operation.params.power));
                            ui.label(operation.params.passes.to_string());
                            ui.label(format!("{:.2} mm", operation.params.line_interval_mm));
                            ui.label(if operation.enabled { "yes" } else { "no" });
                            ui.end_row();
                        }
                    });
            });
        self.operations_compare_open = open;
    }

    fn guides_window(&mut self, ctx: &egui::Context) {
        let mut open = self.guides_open;
        egui::Window::new("Guides")
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(360.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Position");
                    ui.add(
                        egui::DragValue::new(&mut self.guide_position)
                            .speed(0.5)
                            .suffix(" mm"),
                    );
                    if ui.button("Add horizontal").clicked() {
                        self.editor.add_guide(true, self.guide_position);
                    }
                    if ui.button("Add vertical").clicked() {
                        self.editor.add_guide(false, self.guide_position);
                    }
                });
                ui.separator();
                if self.editor.document.guides.is_empty() {
                    ui.weak("No guides yet.");
                }
                let mut remove: Option<String> = None;
                for guide in &self.editor.document.guides {
                    ui.horizontal(|ui| {
                        ui.label(format!(
                            "{} at {:.2} mm",
                            if guide.horizontal {
                                "Horizontal"
                            } else {
                                "Vertical"
                            },
                            guide.position
                        ));
                        if ui.small_button("Remove").clicked() {
                            remove = Some(guide.id.clone());
                        }
                    });
                }
                if let Some(id) = remove {
                    self.editor.remove_guide(&id);
                }
            });
        self.guides_open = open;
    }

    fn profile_confirm_window(&mut self, ctx: &egui::Context) {
        let Some((operation_id, profile)) = self.profile_pending.clone() else {
            self.profile_confirm_open = false;
            return;
        };
        let mut open = self.profile_confirm_open;
        let mut apply = false;
        egui::Window::new(format!("Apply '{}'", profile.name))
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(420.0)
            .show(ctx, |ui| {
                let current = self
                    .editor
                    .document
                    .operations
                    .iter()
                    .find(|operation| operation.id == operation_id)
                    .map(|operation| operation.params.clone());
                let Some(current) = current else {
                    ui.weak("The operation no longer exists.");
                    return;
                };
                let changes = current.changes_to(&profile.params);
                ui.label(format!(
                    "{} parameter(s) will change in this operation:",
                    changes.len()
                ));
                egui::Grid::new("profile_changes")
                    .num_columns(3)
                    .striped(true)
                    .spacing([12.0, 4.0])
                    .show(ui, |ui| {
                        ui.strong("Parameter");
                        ui.strong("Current");
                        ui.strong("Profile");
                        ui.end_row();
                        for change in &changes {
                            ui.label(change.label);
                            ui.label(&change.from);
                            ui.label(&change.to);
                            ui.end_row();
                        }
                    });
                ui.separator();
                if ui.button("Apply").clicked() {
                    apply = true;
                }
                if ui.button("Cancel").clicked() {
                    self.profile_confirm_open = false;
                }
            });
        if apply {
            match self
                .editor
                .apply_profile_to_operation(&operation_id, &profile)
            {
                Ok(0) => {
                    self.status = "The operation already has these values".to_string();
                }
                Ok(count) => {
                    self.status = format!("Applied '{}' ({count} parameter(s))", profile.name);
                }
                Err(error) => self.status = format!("Cannot apply profile: {error}"),
            }
            self.profile_confirm_open = false;
            self.profile_pending = None;
        } else {
            self.profile_confirm_open = open;
            if !open {
                self.profile_pending = None;
            }
        }
    }

    fn open_path(&mut self, path: &Path) {
        match crate::import::format::import_with_report_limited(
            path,
            self.preferences.drawing.max_image_pixels,
        ) {
            Ok(outcome) => self.apply_import(path, outcome),
            Err(error) => self.status = format!("Open failed: {error}"),
        }
    }

    fn start_import(&mut self, path: &Path) {
        if self.task.is_some() {
            self.status = "Another import is still running".to_string();
            return;
        }
        let path = path.to_path_buf();
        let limit = self.preferences.drawing.max_image_pixels;
        let label = format!("Importing {}", path.display());
        let worker_path = path.clone();
        self.task = Some(crate::task::BackgroundTask::spawn(label, move |cancel| {
            let cancelled = || cancel.load(std::sync::atomic::Ordering::Relaxed);
            let outcome = crate::import::format::import_with_report_cancellable(
                &worker_path,
                limit,
                &cancelled,
            )
            .map_err(|error| error.to_string());
            if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                return None;
            }
            Some(TaskResult::Import {
                path: worker_path,
                outcome,
            })
        }));
        self.status = "Importing...".to_string();
    }

    fn start_export(&mut self, mode: FileMode, path: &Path, document: crate::document::Document) {
        if self.task.is_some() {
            self.status = "Another operation is still running".to_string();
            return;
        }
        let path = path.to_path_buf();
        let label = format!("Exporting {}", path.display());
        let worker_path = path.clone();
        self.task = Some(crate::task::BackgroundTask::spawn(label, move |cancel| {
            if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                return None;
            }
            Some(TaskResult::Export(run_export(
                mode,
                &worker_path,
                &document,
                cancel,
            )))
        }));
        self.status = "Exporting...".to_string();
    }

    fn apply_import(&mut self, path: &Path, outcome: crate::import::format::ImportOutcome) {
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
        let _ = std::fs::remove_file(&self.recovery_path);
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
        match self
            .editor
            .save_with_backup(path, self.preferences.general.backup_enabled)
        {
            Ok(()) => {
                self.current_path = Some(path.to_path_buf());
                self.saved_snapshot = Some(self.editor.document.clone());
                self.remember_recent(path);
                let _ = std::fs::remove_file(&self.recovery_path);
                self.last_autosave = std::time::Instant::now();
                self.status = format!("Saved {}", path.display());
            }
            Err(error) => self.status = format!("Save failed: {error}"),
        }
    }

    fn undo(&mut self) {
        self.status = match self.editor.undo() {
            Some(action) => format!("Undo {action}"),
            None => "Nothing to undo".to_string(),
        };
    }

    fn redo(&mut self) {
        self.status = match self.editor.redo() {
            Some(action) => format!("Redo {action}"),
            None => "Nothing to redo".to_string(),
        };
    }

    fn undo_label(&self) -> String {
        self.editor
            .history
            .undo_label()
            .map_or_else(|| "Undo".to_string(), |action| format!("Undo {action}"))
    }

    fn redo_label(&self) -> String {
        self.editor
            .history
            .redo_label()
            .map_or_else(|| "Redo".to_string(), |action| format!("Redo {action}"))
    }

    fn shortcuts(&mut self, ui: &Ui) {
        let ctx = ui.ctx();
        if ctx.text_edit_focused()
            || self.dialog.is_some()
            || self.confirm.is_some()
            || self.preferences_open
            || self.machine_wizard_open
            || self.machines_open
            || self.cuts_open
            || self.image_adjust.is_some()
            || self.help_shortcuts_open
        {
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
            tool_select,
            tool_line,
            tool_rect,
            tool_ellipse,
            tool_star,
            tool_text,
            preview,
            toggle_layers,
            toggle_properties,
            enter,
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
                input.consume_key(Modifiers::NONE, Key::V),
                input.consume_key(Modifiers::NONE, Key::L),
                input.consume_key(Modifiers::NONE, Key::R),
                input.consume_key(Modifiers::NONE, Key::E),
                input.consume_key(Modifiers::NONE, Key::S),
                input.consume_key(Modifiers::NONE, Key::T),
                input.consume_key(Modifiers::CTRL, Key::P),
                input.consume_key(Modifiers::NONE, Key::F9),
                input.consume_key(Modifiers::NONE, Key::F10),
                input.consume_key(Modifiers::NONE, Key::Enter),
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
        let (cut, copy, paste) = ctx.input_mut(|input| {
            (
                input.consume_key(Modifiers::CTRL, Key::X),
                input.consume_key(Modifiers::CTRL, Key::C),
                input.consume_key(Modifiers::CTRL, Key::V),
            )
        });
        if cut {
            self.cut();
        }
        if copy {
            self.copy();
        }
        if paste {
            self.paste();
        }
        if duplicate {
            self.editor.duplicate_selection();
        }
        if delete {
            if self.editor.is_editing_nodes() && !self.editor.node_selection.is_empty() {
                let removed = self.editor.delete_selected_nodes();
                self.status = format!("Deleted {removed} node(s)");
            } else {
                self.editor.delete_selection();
            }
        }
        if group && self.editor.group_selection() {
            self.status = "Objects grouped".to_string();
        }
        if ungroup && self.editor.ungroup_selection() {
            self.status = "Objects ungrouped".to_string();
        }
        if escape {
            self.drag = None;
            if self.editor.tool == Tool::Bezier {
                self.pen_nodes.clear();
                self.pen_drag_start = None;
            }
            if self.editor.is_editing_nodes() {
                self.editor.end_node_edit();
            }
            self.editor.selection.clear();
        }
        if tool_select {
            self.editor.tool = Tool::Select;
        }
        if tool_line {
            self.editor.tool = Tool::Line;
        }
        if tool_rect {
            self.editor.tool = Tool::Rect;
        }
        if tool_ellipse {
            self.editor.tool = Tool::Ellipse;
        }
        if tool_star {
            self.editor.tool = Tool::Star;
        }
        if tool_text {
            self.editor.tool = Tool::Text;
        }
        if preview {
            self.open_preview();
        }
        if toggle_layers {
            self.show_layers_panel = !self.show_layers_panel;
        }
        if toggle_properties {
            self.show_properties_panel = !self.show_properties_panel;
        }
        if enter && self.editor.tool == Tool::Bezier {
            self.finish_pen(false);
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
            self.editor.zoom_in();
        }
        if zoom_out {
            self.editor.zoom_out();
        }
        let step = if ctx.input(|input| input.modifiers.shift) {
            0.1
        } else {
            1.0
        };
        let editing_nodes =
            self.editor.is_editing_nodes() && !self.editor.node_selection.is_empty();
        if left {
            if editing_nodes {
                self.editor.nudge_selected_nodes(-step, 0.0);
            } else {
                self.editor.nudge_selection(-step, 0.0);
            }
        }
        if right {
            if editing_nodes {
                self.editor.nudge_selected_nodes(step, 0.0);
            } else {
                self.editor.nudge_selection(step, 0.0);
            }
        }
        if up {
            if editing_nodes {
                self.editor.nudge_selected_nodes(0.0, step);
            } else {
                self.editor.nudge_selection(0.0, step);
            }
        }
        if down {
            if editing_nodes {
                self.editor.nudge_selected_nodes(0.0, -step);
            } else {
                self.editor.nudge_selection(0.0, -step);
            }
        }
    }

    fn menu_bar(&mut self, ui: &mut Ui) {
        egui::Panel::top("menu_bar").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.menu_button(icons::menu_label(ui, Icon::File, "File"), |ui| {
                    if icons::button(ui, Icon::New, "New").clicked() {
                        self.request(PendingAction::New);
                        ui.close();
                    }
                    if icons::button(ui, Icon::Open, "Open...").clicked() {
                        self.request(PendingAction::Open);
                        ui.close();
                    }
                    let recent = self.preferences.general.recent_projects.clone();
                    ui.add_enabled_ui(!recent.is_empty(), |ui| {
                        ui.menu_button(icons::menu_label(ui, Icon::Recover, "Recent projects"), |ui| {
                            for entry in &recent {
                                let path = std::path::PathBuf::from(entry);
                                let label = path
                                    .file_name()
                                    .and_then(|name| name.to_str())
                                    .unwrap_or(entry);
                                if ui.button(label).on_hover_text(entry).clicked() {
                                    self.start_import(&path);
                                    ui.close();
                                }
                            }
                        });
                    });
                    if icons::button(ui, Icon::Save, "Save").clicked() {
                        self.save();
                        ui.close();
                    }
                    if icons::button(ui, Icon::Save, "Save as...").clicked() {
                        self.open_dialog(FileMode::SaveProject);
                        ui.close();
                    }
                    ui.separator();
                    if icons::button(ui, Icon::Import, "Import SVG or DXF...").clicked() {
                        self.open_dialog(FileMode::ImportFile);
                        ui.close();
                    }
                    if icons::button(ui, Icon::Export, "Export SVG...").clicked() {
                        self.open_dialog(FileMode::ExportSvg);
                        ui.close();
                    }
                    if icons::button(ui, Icon::Export, "Export DXF...").clicked() {
                        self.open_dialog(FileMode::ExportDxf);
                        ui.close();
                    }
                    if icons::button(ui, Icon::Export, "Export PNG...").clicked() {
                        self.open_dialog(FileMode::ExportPng);
                        ui.close();
                    }
                    if icons::button(ui, Icon::Export, "Export G-code...").clicked() {
                        self.open_dialog(FileMode::ExportGcode);
                        ui.close();
                    }
                    ui.menu_button(icons::menu_label(ui, Icon::Export, "Export selection"), |ui| {
                        let enabled = !self.editor.selection.is_empty();
                        if ui
                            .add_enabled(enabled, icons::widget(ui, Icon::Export, "As SVG..."))
                            .clicked()
                        {
                            self.export_subset = true;
                            self.open_dialog(FileMode::ExportSvg);
                            ui.close();
                        }
                        if ui
                            .add_enabled(enabled, icons::widget(ui, Icon::Export, "As DXF..."))
                            .clicked()
                        {
                            self.export_subset = true;
                            self.open_dialog(FileMode::ExportDxf);
                            ui.close();
                        }
                        if ui
                            .add_enabled(enabled, icons::widget(ui, Icon::Export, "As G-code..."))
                            .clicked()
                        {
                            self.export_subset = true;
                            self.open_dialog(FileMode::ExportGcode);
                            ui.close();
                        }
                    });
                    ui.separator();
                    if icons::button(ui, Icon::Page, "Document properties...").clicked() {
                        self.open_document_properties();
                        ui.close();
                    }
                    if ui
                        .add_enabled(
                            self.current_path.is_some()
                                && self.editor.document.objects.iter().any(|object| {
                                    matches!(object.shape, crate::document::Shape::Image(_))
                                }),
                            icons::widget(ui, Icon::Image, "Collect images..."),
                        )
                        .clicked()
                    {
                        self.collect_images();
                        ui.close();
                    }
                    if icons::button(ui, Icon::Recover, "Recover project...").clicked() {
                        self.recover_next = true;
                        self.open_dialog(FileMode::OpenProject);
                        ui.close();
                    }
                    ui.separator();
                    if icons::button(ui, Icon::Gear, "Machine profiles...").clicked() {
                        self.machines_open = true;
                        ui.close();
                    }
                    if icons::button(ui, Icon::Exit, "Exit").clicked() {
                        self.request(PendingAction::Quit);
                        ui.close();
                    }
                });
                ui.menu_button(icons::menu_label(ui, Icon::Edit, "Edit"), |ui| {
                    if ui
                        .add_enabled(
                            self.editor.history.can_undo(),
                            icons::widget(ui, Icon::Undo, self.undo_label()),
                        )
                        .clicked()
                    {
                        self.undo();
                        ui.close();
                    }
                    if ui
                        .add_enabled(
                            self.editor.history.can_redo(),
                            icons::widget(ui, Icon::Redo, self.redo_label()),
                        )
                        .clicked()
                    {
                        self.redo();
                        ui.close();
                    }
                    ui.separator();
                    if icons::button(ui, Icon::SelectAll, "Select all").clicked() {
                        self.editor.select_all();
                        ui.close();
                    }
                    if ui
                        .add_enabled(
                            !self.editor.selection.is_empty(),
                            icons::widget(ui, Icon::Cut, "Cut"),
                        )
                        .clicked()
                    {
                        self.cut();
                        ui.close();
                    }
                    if ui
                        .add_enabled(
                            !self.editor.selection.is_empty(),
                            icons::widget(ui, Icon::Copy, "Copy"),
                        )
                        .clicked()
                    {
                        self.copy();
                        ui.close();
                    }
                    if ui
                        .add_enabled(!self.clipboard.is_empty(), icons::widget(ui, Icon::Paste, "Paste"))
                        .clicked()
                    {
                        self.paste();
                        ui.close();
                    }
                    if ui
                        .add_enabled(
                            !self.editor.selection.is_empty(),
                            icons::widget(ui, Icon::Duplicate, "Duplicate"),
                        )
                        .clicked()
                    {
                        self.editor.duplicate_selection();
                        ui.close();
                    }
                    if ui
                        .add_enabled(
                            !self.editor.selection.is_empty(),
                            icons::widget(ui, Icon::Delete, "Delete"),
                        )
                        .clicked()
                    {
                        self.editor.delete_selection();
                        ui.close();
                    }
                    if icons::button(ui, Icon::Invert, "Invert selection").clicked() {
                        let count = self.editor.invert_selection();
                        self.status = format!("Selected {count} object(s)");
                        ui.close();
                    }
                    ui.separator();
                    if ui
                        .add_enabled(
                            !self.editor.selection.is_empty(),
                            icons::widget(ui, Icon::Lock, "Lock"),
                        )
                        .clicked()
                    {
                        let count = self.editor.lock_selection();
                        self.status = format!("Locked {count} object(s)");
                        ui.close();
                    }
                    if ui
                        .add_enabled(
                            !self.editor.selection.is_empty(),
                            icons::widget(ui, Icon::Unlock, "Unlock"),
                        )
                        .clicked()
                    {
                        let count = self.editor.unlock_selection();
                        self.status = format!("Unlocked {count} object(s)");
                        ui.close();
                    }
                    ui.separator();
                    if icons::button(ui, Icon::Preferences, "Preferences...").clicked() {
                        self.preferences_open = true;
                        ui.close();
                    }
                    ui.separator();
                    if ui
                        .add_enabled(self.editor.selection.len() >= 2, icons::widget(ui, Icon::Group, "Group"))
                        .clicked()
                    {
                        self.editor.group_selection();
                        ui.close();
                    }
                    if ui
                        .add_enabled(
                            !self.editor.selection.is_empty(),
                            icons::widget(ui, Icon::Ungroup, "Ungroup"),
                        )
                        .clicked()
                    {
                        self.editor.ungroup_selection();
                        ui.close();
                    }
                });
                ui.menu_button(icons::menu_label(ui, Icon::Draw, "Draw"), |ui| {
                    for (tool, icon, label) in [
                        (Tool::Select, Icon::Select, "Selection"),
                        (Tool::Line, Icon::Line, "Line"),
                        (Tool::Rect, Icon::Rectangle, "Rectangle"),
                        (Tool::Ellipse, Icon::Ellipse, "Ellipse"),
                        (Tool::Polygon, Icon::Polygon, "Polygon"),
                        (Tool::Star, Icon::Star, "Star"),
                        (Tool::Text, Icon::Text, "Text"),
                        (Tool::Measure, Icon::Measure, "Measure"),
                        (Tool::Bezier, Icon::Bezier, "Bézier"),
                        (Tool::Pencil, Icon::Freehand, "Freehand"),
                    ] {
                        if icons::selectable(ui, self.editor.tool == tool, icon, label).clicked() {
                            self.editor.tool = tool;
                            ui.close();
                        }
                    }
                    ui.separator();
                    if icons::button(ui, Icon::Image, "Insert image...").clicked() {
                        self.open_dialog(FileMode::ImportFile);
                        ui.close();
                    }
                    if ui
                        .add_enabled(
                            self.editor.selection.len() == 1,
                            icons::widget(ui, Icon::Nodes, "Edit nodes"),
                        )
                        .clicked()
                    {
                        if let Some(id) = self.editor.selection.ids().first().cloned() {
                            match self.editor.begin_node_edit(&id) {
                                Ok(()) => self.status = "Editing nodes".to_string(),
                                Err(error) => {
                                    self.status = format!("Cannot edit nodes: {error}");
                                }
                            }
                        }
                        ui.close();
                    }
                    if ui
                        .add_enabled(
                            self.editor.selection.len() >= 2,
                            icons::widget(ui, Icon::Trim, "Trim segments"),
                        )
                        .clicked()
                    {
                        self.apply_trim();
                        ui.close();
                    }
                    if ui
                        .add_enabled(
                            self.editor.selection.len() >= 2,
                            icons::widget(ui, Icon::Join, "Join paths"),
                        )
                        .clicked()
                    {
                        self.apply_join_paths();
                        ui.close();
                    }
                });
                ui.menu_button(icons::menu_label(ui, Icon::Operations, "Operations"), |ui| {
                    if icons::button(ui, Icon::Layers, "Cuts / Layers...").clicked() {
                        self.open_cuts();
                        ui.close();
                    }
                    if icons::button(ui, Icon::Plus, "New operation").clicked() {
                        let id = self.editor.add_operation(
                            format!(
                                "Operation {}",
                                self.editor.document.operations.len() + 1
                            ),
                            crate::operations::OperationType::OutlineCutting,
                        );
                        self.active_operation = Some(id);
                        ui.close();
                    }
                    if icons::button(ui, Icon::Compare, "Compare operations...").clicked() {
                        self.operations_compare_open = true;
                        ui.close();
                    }
                    if icons::button(ui, Icon::Play, "Preview and G-code...").clicked() {
                        self.open_preview();
                        ui.close();
                    }
                    if icons::button(ui, Icon::Clock, "Estimate duration").clicked() {
                        match crate::gcode::render_job(
                            &self.editor.document,
                            &crate::gcode::JobOptions::default(),
                        ) {
                            Ok((_, report)) => {
                                self.status = format!(
                                    "Paths: {} - {:.1} s estimated",
                                    report.paths, report.seconds
                                );
                            }
                            Err(error) => self.status = format!("Estimate failed: {error}"),
                        }
                        ui.close();
                    }
                    if ui
                        .add_enabled(
                            !self.editor.selection.is_empty(),
                            icons::widget(ui, Icon::Check, "Apply profile"),
                        )
                        .clicked()
                    {
                        self.profiles_open = true;
                        ui.close();
                    }
                    if ui
                        .add_enabled(
                            self.active_operation.is_some(),
                            icons::widget(ui, Icon::Duplicate, "Duplicate operation"),
                        )
                        .clicked()
                    {
                        if let Some(operation) = self.active_operation.clone() {
                            if let Some(copy) = self.editor.duplicate_operation(&operation) {
                                self.active_operation = Some(copy);
                                self.status = "Operation duplicated".to_string();
                            }
                        }
                        ui.close();
                    }
                    if ui
                        .add_enabled(
                            self.active_operation.is_some(),
                            icons::widget(ui, Icon::Layers, "Assign operation to layer"),
                        )
                        .clicked()
                    {
                        if let Some(operation) = self.active_operation.clone() {
                            let layer = self.editor.active_layer_id.clone();
                            let changed = self
                                .editor
                                .assign_operation_to_layer(&layer, Some(&operation));
                            self.status =
                                format!("Assigned operation to {changed} layer object(s)");
                        }
                        ui.close();
                    }
                });
                ui.menu_button(icons::menu_label(ui, Icon::Materials, "Materials and profiles"), |ui| {
                    if icons::button(ui, Icon::Book, "Open library...").clicked() {
                        self.profiles_open = true;
                        ui.close();
                    }
                    if icons::button(ui, Icon::Import, "Import profiles...").clicked() {
                        self.open_dialog(FileMode::ImportProfiles);
                        ui.close();
                    }
                    if icons::button(ui, Icon::Export, "Export profiles...").clicked() {
                        self.open_dialog(FileMode::ExportProfiles);
                        ui.close();
                    }
                });
                ui.menu_button(icons::menu_label(ui, Icon::Arrange, "Arrange"), |ui| {
                    let enabled = !self.editor.selection.is_empty();
                    if ui
                        .add_enabled(enabled, icons::widget(ui, Icon::ToFront, "Bring to front"))
                        .clicked()
                    {
                        let count = self.editor.bring_selection_to_front();
                        self.status = format!("Brought {count} object(s) to the front");
                        ui.close();
                    }
                    if ui
                        .add_enabled(enabled, icons::widget(ui, Icon::ToBack, "Send to back"))
                        .clicked()
                    {
                        let count = self.editor.send_selection_to_back();
                        self.status = format!("Sent {count} object(s) to the back");
                        ui.close();
                    }
                    if ui
                        .add_enabled(enabled, icons::widget(ui, Icon::Center, "Centre on page"))
                        .clicked()
                    {
                        self.editor.center_selection_on_page();
                        ui.close();
                    }
                    if ui
                        .add_enabled(
                            self.editor.selection.len() >= 2,
                            icons::widget(ui, Icon::DistributeH, "Distribute on page horizontally"),
                        )
                        .clicked()
                    {
                        self.editor.distribute_selection_on_page(true);
                        ui.close();
                    }
                    if ui
                        .add_enabled(
                            self.editor.selection.len() >= 2,
                            icons::widget(ui, Icon::DistributeV, "Distribute on page vertically"),
                        )
                        .clicked()
                    {
                        self.editor.distribute_selection_on_page(false);
                        ui.close();
                    }
                    ui.separator();
                    if ui
                        .add_enabled(enabled, icons::widget(ui, Icon::RotateLeft, "Rotate 90° left"))
                        .clicked()
                    {
                        self.editor.rotate_selection(std::f64::consts::FRAC_PI_2);
                        ui.close();
                    }
                    if ui
                        .add_enabled(enabled, icons::widget(ui, Icon::RotateRight, "Rotate 90° right"))
                        .clicked()
                    {
                        self.editor.rotate_selection(-std::f64::consts::FRAC_PI_2);
                        ui.close();
                    }
                    ui.separator();
                    if ui
                        .add_enabled(enabled, icons::widget(ui, Icon::FlipH, "Flip horizontal"))
                        .clicked()
                    {
                        self.editor.flip_selection(true);
                        ui.close();
                    }
                    if ui
                        .add_enabled(enabled, icons::widget(ui, Icon::FlipV, "Flip vertical"))
                        .clicked()
                    {
                        self.editor.flip_selection(false);
                        ui.close();
                    }
                    ui.separator();
                    if ui
                        .add_enabled(enabled, icons::widget(ui, Icon::ScaleUp, "Scale up 10 %"))
                        .clicked()
                    {
                        self.editor.scale_selection(1.1, 1.1);
                        ui.close();
                    }
                    if ui
                        .add_enabled(enabled, icons::widget(ui, Icon::ScaleDown, "Scale down 10 %"))
                        .clicked()
                    {
                        self.editor.scale_selection(1.0 / 1.1, 1.0 / 1.1);
                        ui.close();
                    }
                    ui.separator();
                    if ui
                        .add_enabled(
                            self.editor.selection.len() == 1,
                            icons::widget(ui, Icon::TextOutline, "Text to outlines"),
                        )
                        .clicked()
                    {
                        self.apply_text_outlines();
                        ui.close();
                    }
                    if ui
                        .add_enabled(
                            self.editor.selection.len() == 2,
                            icons::widget(ui, Icon::TextOnPath, "Fit text to path"),
                        )
                        .clicked()
                    {
                        self.apply_text_on_path();
                        ui.close();
                    }
                    ui.menu_button(icons::menu_label(ui, Icon::TransformCenter, "Transform around"), |ui| {
                        use crate::editor::state::TransformCenter;
                        for (mode, label) in [
                            (TransformCenter::Selection, "Selection centre"),
                            (TransformCenter::Page, "Page centre"),
                            (TransformCenter::Object, "Each object's centre"),
                        ] {
                            icons::selectable_value(
                                ui,
                                &mut self.editor.transform_center,
                                mode,
                                Icon::TransformCenter,
                                label,
                            );
                        }
                    });
                    if ui
                        .add_enabled(enabled, icons::widget(ui, Icon::Recover, "Reset transforms"))
                        .clicked()
                    {
                        let count = self.editor.reset_transforms();
                        self.status = format!("Reset {count} object(s)");
                        ui.close();
                    }
                    if ui
                        .add_enabled(enabled, icons::widget(ui, Icon::Skew, "Skew..."))
                        .clicked()
                    {
                        self.skew_open = true;
                        ui.close();
                    }
                    ui.separator();
                    if ui
                        .add_enabled(enabled, icons::widget(ui, Icon::GridArray, "Grid array..."))
                        .clicked()
                    {
                        self.grid_array_open = true;
                        ui.close();
                    }
                    ui.separator();
                    if ui
                        .add_enabled(
                            self.editor.selection.len() >= 2,
                            icons::widget(ui, Icon::Join, "Join paths"),
                        )
                        .clicked()
                    {
                        self.apply_join_paths();
                        ui.close();
                    }
                    if ui
                        .add_enabled(enabled, icons::widget(ui, Icon::ClosePath, "Close paths"))
                        .clicked()
                    {
                        self.apply_close_paths();
                        ui.close();
                    }
                    ui.separator();
                    if self.editor.is_editing_nodes() {
                        if icons::button(ui, Icon::Nodes, "Exit node editing").clicked() {
                            self.editor.end_node_edit();
                            ui.close();
                        }
                        if icons::button(ui, Icon::Simplify, "Simplify path...").clicked() {
                            self.simplify_open = true;
                            ui.close();
                        }
                    } else if ui
                        .add_enabled(
                            self.editor.selection.len() == 1,
                            icons::widget(ui, Icon::Nodes, "Edit nodes"),
                        )
                        .clicked()
                    {
                        if let Some(id) = self.editor.selection.ids().first().cloned() {
                            match self.editor.begin_node_edit(&id) {
                                Ok(()) => {
                                    self.status = "Editing nodes: drag to move, double-click a segment to insert"
                                        .to_string();
                                }
                                Err(error) => {
                                    self.status = format!("Cannot edit nodes: {error}");
                                }
                            }
                        }
                        ui.close();
                    }
                    if ui
                        .add_enabled(
                            !self.editor.selection.is_empty(),
                            icons::widget(ui, Icon::Repeat, "Repeat..."),
                        )
                        .clicked()
                    {
                        self.repeat_open = true;
                        ui.close();
                    }
                    ui.separator();
                    if ui
                        .add_enabled(
                            !self.editor.selection.is_empty(),
                            icons::widget(ui, Icon::Nest, "Nest selection..."),
                        )
                        .clicked()
                    {
                        self.open_nest();
                        ui.close();
                    }
                    ui.separator();
                    if icons::button(ui, Icon::FindDuplicates, "Find duplicates...").clicked() {
                        self.report_duplicates();
                        ui.close();
                    }
                    if icons::button(ui, Icon::Intersections, "Find intersections...").clicked() {
                        self.report_intersections();
                        ui.close();
                    }
                    ui.separator();
                    let can_combine = self.editor.selection.len() == 2;
                    ui.menu_button(icons::menu_label(ui, Icon::Boolean, "Boolean"), |ui| {
                        use crate::geometry::BooleanOp;
                        for op in BooleanOp::all() {
                            if ui
                                .add_enabled(can_combine, icons::widget(ui, boolean_icon(op), op.label()))
                                .clicked()
                            {
                                self.apply_boolean(op);
                                ui.close();
                            }
                        }
                    });
                    if ui
                        .add_enabled(enabled, icons::widget(ui, Icon::OutlineStroke, "Outline stroke"))
                        .clicked()
                    {
                        self.apply_outline_stroke();
                        ui.close();
                    }
                    if ui
                        .add_enabled(
                            self.editor.selection.len() >= 2,
                            icons::widget(ui, Icon::Trim, "Trim segments"),
                        )
                        .clicked()
                    {
                        self.apply_trim();
                        ui.close();
                    }
                    if ui
                        .add_enabled(
                            self.editor.selection.len() >= 2,
                            icons::widget(ui, Icon::Trim, "Delete overlapping segments"),
                        )
                        .clicked()
                    {
                        self.apply_delete_overlaps();
                        ui.close();
                    }
                    if ui
                        .add_enabled(enabled, icons::widget(ui, Icon::Offset, "Offset..."))
                        .clicked()
                    {
                        self.offset_open = true;
                        ui.close();
                    }
                });
                ui.menu_button(icons::menu_label(ui, Icon::Align, "Align"), |ui| {
                    use crate::editor::alignment::AlignMode;
                    let enabled = !self.editor.selection.is_empty();
                    for (mode, icon, label) in [
                        (AlignMode::Left, Icon::AlignLeft, "Align left"),
                        (AlignMode::CenterH, Icon::AlignCenterH, "Align centre horizontally"),
                        (AlignMode::Right, Icon::AlignRight, "Align right"),
                        (AlignMode::Top, Icon::AlignTop, "Align top"),
                        (AlignMode::CenterV, Icon::AlignCenterV, "Align centre vertically"),
                        (AlignMode::Bottom, Icon::AlignBottom, "Align bottom"),
                    ] {
                        if ui
                            .add_enabled(enabled, icons::widget(ui, icon, label))
                            .clicked()
                        {
                            self.editor.align_selection(mode);
                            ui.close();
                        }
                    }
                    ui.separator();
                    if ui
                        .add_enabled(
                            self.editor.selection.len() >= 3,
                            icons::widget(ui, Icon::DistributeH, "Distribute horizontally"),
                        )
                        .clicked()
                    {
                        self.editor.distribute_selection(true);
                        ui.close();
                    }
                    if ui
                        .add_enabled(
                            self.editor.selection.len() >= 3,
                            icons::widget(ui, Icon::DistributeV, "Distribute vertically"),
                        )
                        .clicked()
                    {
                        self.editor.distribute_selection(false);
                        ui.close();
                    }
                });
                ui.menu_button(icons::menu_label(ui, Icon::Book, "Profiles"), |ui| {
                    if icons::button(ui, Icon::Materials, "Materials and operations...").clicked() {
                        self.profiles_open = true;
                        ui.close();
                    }
                });
                ui.menu_button(icons::menu_label(ui, Icon::View, "View"), |ui| {
                    if icons::button(ui, Icon::ZoomIn, "Zoom in").clicked() {
                        self.editor.zoom_in();
                        ui.close();
                    }
                    if icons::button(ui, Icon::ZoomOut, "Zoom out").clicked() {
                        self.editor.zoom_out();
                        ui.close();
                    }
                    if icons::button(ui, Icon::ZoomFit, "Fit page").clicked() {
                        self.needs_fit = true;
                        ui.close();
                    }
                    if ui
                        .add_enabled(
                            !self.editor.selection.is_empty(),
                            icons::widget(ui, Icon::ZoomSelection, "Zoom to selection"),
                        )
                        .clicked()
                    {
                        self.zoom_to_selection_request = true;
                        ui.close();
                    }
                    ui.separator();
                    icons::checkbox(ui, &mut self.editor.canvas.grid_visible, Icon::Grid, "Show grid");
                    if icons::checkbox(
                        ui,
                        &mut self.editor.show_selection,
                        Icon::Eye,
                        "Show selection",
                    )
                    .changed()
                    {
                        self.preferences.drawing.show_selection = self.editor.show_selection;
                    }
                    icons::checkbox(ui, &mut self.editor.show_rulers, Icon::Rulers, "Show rulers");
                    icons::checkbox(ui, &mut self.editor.guides_visible, Icon::Guides, "Show guides");
                    icons::checkbox(ui, &mut self.editor.snap_guides, Icon::Snap, "Snap to guides");
                    if icons::button(ui, Icon::Guides, "Guides...").clicked() {
                        self.guides_open = true;
                        ui.close();
                    }
                    ui.menu_button(icons::menu_label(ui, Icon::Snap, "Snapping"), |ui| {
                        icons::checkbox(ui, &mut self.editor.snap_enabled, Icon::Grid, "Grid");
                        icons::checkbox(ui, &mut self.editor.snap_nodes, Icon::Nodes, "Nodes");
                        icons::checkbox(ui, &mut self.editor.snap_endpoints, Icon::Nodes, "Endpoints");
                        icons::checkbox(ui, &mut self.editor.snap_centers, Icon::SnapCenter, "Centres");
                        icons::checkbox(ui, &mut self.editor.snap_midpoints, Icon::SnapMidpoint, "Midpoints");
                        icons::checkbox(ui, &mut self.editor.snap_intersections, Icon::Intersections, "Intersections");
                        icons::checkbox(ui, &mut self.editor.snap_limits, Icon::SnapLimits, "Object limits");
                        ui.add(
                            egui::DragValue::new(&mut self.editor.snap_step_mm)
                                .speed(0.1)
                                .range(0.1..=100.0)
                                .suffix(" mm grid"),
                        );
                    });
                    icons::checkbox(ui, &mut self.preview_toolpaths, Icon::Toolpaths, "Show toolpaths");
                    icons::checkbox(ui, &mut self.show_layers_panel, Icon::Panels, "Layers panel (F9)");
                    icons::checkbox(ui, &mut self.show_properties_panel, Icon::Panels, "Properties panel (F10)");
                    if icons::button(ui, Icon::Compare, "Compare operations...").clicked() {
                        self.operations_compare_open = true;
                        ui.close();
                    }
                    if icons::button(ui, Icon::Play, "Preview and G-code...").clicked() {
                        self.open_preview();
                        ui.close();
                    }
                    icons::checkbox(ui, &mut self.dark_mode, Icon::DarkMode, "Dark mode");
                    ui.separator();
                    if icons::button(ui, Icon::Preferences, "Preferences...").clicked() {
                        self.preferences_open = true;
                        ui.close();
                    }
                });
                ui.menu_button(icons::menu_label(ui, Icon::Help, "Help"), |ui| {
                    if icons::button(ui, Icon::Text, "Keyboard shortcuts").clicked() {
                        self.help_shortcuts_open = true;
                        ui.close();
                    }
                    if icons::button(ui, Icon::Info, "About BeamWright").clicked() {
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

    fn tool_strip(&mut self, ui: &mut Ui) {
        egui::Panel::left("tool_strip")
            .resizable(false)
            .exact_size(56.0)
            .show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.vertical_centered(|ui| {
                        for tool in [
                            Tool::Select,
                            Tool::Pencil,
                            Tool::Rect,
                            Tool::Ellipse,
                            Tool::Polygon,
                            Tool::Star,
                            Tool::Line,
                            Tool::Bezier,
                            Tool::Text,
                            Tool::Measure,
                            Tool::Pan,
                        ] {
                            if icons::icon_button(
                                ui,
                                self.editor.tool == tool,
                                tool_icon(tool),
                                26.0,
                                tool.label(),
                            )
                            .on_hover_text(tool_help(tool))
                            .clicked()
                            {
                                self.editor.tool = tool;
                            }
                        }
                        ui.separator();
                        let has_selection = !self.editor.selection.is_empty();
                        if icons::icon_button_enabled(
                            ui,
                            has_selection,
                            Icon::Duplicate,
                            22.0,
                            "Duplicate",
                        )
                        .on_hover_text("Duplicate the selection")
                        .clicked()
                        {
                            self.editor.duplicate_selection();
                        }
                        if icons::icon_button_enabled(
                            ui,
                            has_selection,
                            Icon::Delete,
                            22.0,
                            "Delete",
                        )
                        .on_hover_text("Delete the selection")
                        .clicked()
                        {
                            self.editor.delete_selection();
                        }
                        ui.separator();
                        let two_selected = self.editor.selection.len() == 2;
                        for op in crate::geometry::BooleanOp::all() {
                            if icons::icon_button_enabled(
                                ui,
                                two_selected,
                                boolean_icon(op),
                                22.0,
                                op.label(),
                            )
                            .on_hover_text(format!("{} the two selected objects", op.label()))
                            .clicked()
                            {
                                self.apply_boolean(op);
                            }
                        }
                        ui.separator();
                        if icons::icon_button_enabled(
                            ui,
                            has_selection,
                            Icon::Nodes,
                            22.0,
                            "Edit nodes",
                        )
                        .on_hover_text("Edit the nodes of one path")
                        .clicked()
                        {
                            if let Some(id) = self.editor.selection.ids().first().cloned() {
                                match self.editor.begin_node_edit(&id) {
                                    Ok(()) => {
                                        self.status =
                                            "Editing nodes: drag to move, double-click a segment to insert"
                                                .to_string();
                                    }
                                    Err(error) => {
                                        self.status = format!("Cannot edit nodes: {error}");
                                    }
                                }
                            }
                        }
                        if icons::icon_button_enabled(
                            ui,
                            has_selection,
                            Icon::GridArray,
                            22.0,
                            "Grid array",
                        )
                        .on_hover_text("Copy the selection in a grid")
                        .clicked()
                        {
                            self.grid_array_open = true;
                        }
                        if icons::icon_button_enabled(
                            ui,
                            has_selection,
                            Icon::Repeat,
                            22.0,
                            "Circular array",
                        )
                        .on_hover_text("Copy the selection around a circle")
                        .clicked()
                        {
                            self.repeat_open = true;
                        }
                        if icons::icon_button_enabled(
                            ui,
                            has_selection,
                            Icon::Offset,
                            22.0,
                            "Offset",
                        )
                        .on_hover_text("Offset the selection by the radius")
                        .clicked()
                        {
                            self.offset_open = true;
                        }
                        if icons::icon_button_enabled(
                            ui,
                            has_selection,
                            Icon::OutlineStroke,
                            22.0,
                            "Outline stroke",
                        )
                        .on_hover_text("Convert the stroke into an outline")
                        .clicked()
                        {
                            self.apply_outline_stroke();
                        }
                        ui.separator();
                        ui.label("Radius");
                        ui.add(
                            egui::DragValue::new(&mut self.offset_distance)
                                .speed(0.1)
                                .range(0.1..=50.0)
                                .suffix(" mm"),
                        );
                    });
                });
            });
    }

    fn toolbar(&mut self, ui: &mut Ui) {
        egui::Panel::top("toolbar").show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add_enabled(
                        self.editor.history.can_undo(),
                        icons::widget_at(ui, Icon::Undo, self.undo_label(), 20.0),
                    )
                    .clicked()
                {
                    self.undo();
                }
                if ui
                    .add_enabled(
                        self.editor.history.can_redo(),
                        icons::widget_at(ui, Icon::Redo, self.redo_label(), 20.0),
                    )
                    .clicked()
                {
                    self.redo();
                }

                if self.editor.tool == Tool::Polygon {
                    ui.add(
                        egui::DragValue::new(&mut self.polygon_sides)
                            .speed(1.0)
                            .range(3..=64),
                    );
                    ui.label("sides");
                }
                if matches!(self.editor.tool, Tool::Rect | Tool::Ellipse) {
                    icons::checkbox_at(ui, &mut self.exact_size, Icon::Measure, "Exact size", 18.0);
                    if self.exact_size {
                        ui.add(
                            egui::DragValue::new(&mut self.exact_width)
                                .speed(0.5)
                                .range(0.01..=10000.0)
                                .suffix(" mm W"),
                        );
                        ui.add(
                            egui::DragValue::new(&mut self.exact_height)
                                .speed(0.5)
                                .range(0.01..=10000.0)
                                .suffix(" mm H"),
                        );
                    }
                }
                if matches!(self.editor.tool, Tool::Rect | Tool::Ellipse | Tool::Line) {
                    ui.weak("Shift: square/circle or 45° steps");
                }
                if self.editor.tool == Tool::Star {
                    ui.add(
                        egui::DragValue::new(&mut self.star_points)
                            .speed(1.0)
                            .range(3..=64),
                    );
                    ui.label("tips");
                    ui.add(
                        egui::Slider::new(&mut self.star_inner, 0.05..=1.0)
                            .text("inner")
                            .fixed_decimals(2),
                    );
                }
                ui.separator();
                if ui.button("-").clicked() {
                    self.editor.zoom_out();
                }
                ui.label(format!("{:.0}%", self.editor.canvas.zoom * 100.0));
                if ui.button("+").clicked() {
                    self.editor.zoom_in();
                }
                if ui.button("Fit").clicked() {
                    self.needs_fit = true;
                }
                ui.separator();
                ui.checkbox(&mut self.editor.canvas.grid_visible, "Grid");
                ui.checkbox(&mut self.editor.snap_enabled, "Snap");
                ui.checkbox(&mut self.preview_toolpaths, "Toolpaths");
                ui.separator();
                if icons::button(ui, Icon::Open, "Open").clicked() {
                    self.request(PendingAction::Open);
                }
                if icons::button(ui, Icon::Save, "Save").clicked() {
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
                let mut hits = crate::editor::search::search_document(
                    &self.editor.document,
                    &self.search_query,
                );
                hits.extend(crate::editor::search::search_profiles(
                    &self.profiles,
                    &self.search_query,
                ));
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
                                        crate::editor::search::SearchKind::Profile => {
                                            self.select_profile(&hit.id);
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
                ui.horizontal(|ui| {
                    let layer = self.editor.active_layer_id.clone();
                    if ui.button("Duplicate layer").clicked() {
                        if let Some(copy) = self.editor.duplicate_layer(&layer) {
                            self.editor.active_layer_id = copy;
                        }
                    }
                    if ui.button("Select layer objects").clicked() {
                        let count = self.editor.select_layer_objects(&layer);
                        self.status = format!("Selected {count} layer object(s)");
                    }
                    if ui.button("Group layer").clicked() {
                        let count = self.editor.group_layer_objects(&layer);
                        self.status = format!("Grouped {count} layer object(s)");
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
                        if icons::icon_button(ui, is_visible, Icon::Eye, 18.0, "Layer visible")
                            .clicked()
                        {
                            is_visible = !is_visible;
                            if let Some(layer) = self.editor.document.layer_mut(id) {
                                layer.visible = is_visible;
                            }
                        }
                        let mut is_locked = *locked;
                        if icons::icon_button(
                            ui,
                            is_locked,
                            if is_locked { Icon::Lock } else { Icon::Unlock },
                            18.0,
                            "Layer locked",
                        )
                        .clicked()
                        {
                            is_locked = !is_locked;
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
                    self.editor.history.record(&before, "Edit layer");
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
                if ui
                    .small_button("Duplicate")
                    .on_hover_text("Duplicate this operation")
                    .clicked()
                {
                    if let Some(copy) = self.editor.duplicate_operation(id) {
                        self.active_operation = Some(copy);
                    }
                }
            });
        }
        let mut copy_parameters = false;
        let mut paste_parameters = false;
        let mut save_as_profile = false;
        let mut dependency_choice: Option<Option<String>> = None;
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
                    ui.label("Runs after");
                    let selected = operation.depends_on.first().cloned();
                    let selected_text = selected
                        .as_ref()
                        .and_then(|id| {
                            operations
                                .iter()
                                .find(|(operation_id, ..)| operation_id == id)
                                .map(|(_, name, ..)| name.clone())
                        })
                        .unwrap_or_else(|| "None".to_string());
                    egui::ComboBox::from_id_salt("operation_dependency")
                        .selected_text(selected_text)
                        .show_ui(ui, |ui| {
                            if ui.selectable_label(selected.is_none(), "None").clicked() {
                                dependency_choice = Some(None);
                            }
                            for (id, name, ..) in &operations {
                                if *id == active {
                                    continue;
                                }
                                if ui
                                    .selectable_label(
                                        selected.as_deref() == Some(id.as_str()),
                                        name,
                                    )
                                    .clicked()
                                {
                                    dependency_choice = Some(Some(id.clone()));
                                }
                            }
                        });
                });
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
                    ui.horizontal(|ui| {
                        ui.label("Fill angle");
                        ui.add(
                            egui::DragValue::new(&mut operation.params.fill_angle_degrees)
                                .speed(1.0)
                                .range(-180.0..=180.0)
                                .suffix(" deg"),
                        );
                        ui.label("Margin");
                        ui.add(
                            egui::DragValue::new(&mut operation.params.fill_margin_mm)
                                .speed(0.1)
                                .range(0.0..=50.0)
                                .suffix(" mm"),
                        );
                    });
                    ui.horizontal(|ui| {
                        ui.label("Pattern offset");
                        ui.add(
                            egui::DragValue::new(&mut operation.params.fill_offset_mm)
                                .speed(0.1)
                                .range(-50.0..=50.0)
                                .suffix(" mm"),
                        );
                        ui.checkbox(&mut operation.params.fill_alternate, "Alternate direction");
                    });
                    ui.weak("Fill spacing is the line interval above. Offset fill uses the interval between loops.");
                }
                ui.horizontal(|ui| {
                    if ui.button("Copy parameters").clicked() {
                        copy_parameters = true;
                    }
                    if ui
                        .add_enabled(
                            self.operation_params.is_some(),
                            egui::Button::new("Paste parameters"),
                        )
                        .clicked()
                    {
                        paste_parameters = true;
                    }
                    if ui.button("Save as profile").clicked() {
                        save_as_profile = true;
                    }
                });
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
                if ui
                    .add_enabled(
                        !self.editor.document.objects.is_empty(),
                        egui::Button::new("Assign to active layer"),
                    )
                    .clicked()
                {
                    let layer = self.editor.active_layer_id.clone();
                    let changed = self.editor.assign_operation_to_layer(&layer, Some(&active));
                    self.status = format!("Assigned operation to {changed} layer object(s)");
                }
                if ui
                    .add_enabled(
                        !self.editor.document.objects.is_empty(),
                        egui::Button::new("Clear layer assignment"),
                    )
                    .clicked()
                {
                    let layer = self.editor.active_layer_id.clone();
                    let changed = self.editor.assign_operation_to_layer(&layer, None);
                    self.status = format!("Cleared operation on {changed} layer object(s)");
                }
            });
        }
        if let Some(choice) = dependency_choice {
            if let Some(active) = self.active_operation.clone() {
                self.editor
                    .set_operation_dependency(&active, choice.as_deref());
            }
        }
        if let Some(active) = self.active_operation.clone() {
            if copy_parameters {
                if let Some(operation) = self
                    .editor
                    .document
                    .operations
                    .iter()
                    .find(|operation| operation.id == active)
                {
                    self.operation_params = Some(operation.params.clone());
                    self.status = "Operation parameters copied".to_string();
                }
            }
            if paste_parameters {
                if let Some(params) = self.operation_params.clone() {
                    if let Some(operation) = self
                        .editor
                        .document
                        .operations
                        .iter_mut()
                        .find(|operation| operation.id == active)
                    {
                        operation.params = params;
                        self.status = "Operation parameters pasted".to_string();
                    }
                }
            }
            if save_as_profile {
                if let Some(operation) = self
                    .editor
                    .document
                    .operations
                    .iter()
                    .find(|operation| operation.id == active)
                {
                    let mut profile = crate::profiles::OperationProfile::new(
                        format!("{} profile", operation.name),
                        operation.op_type,
                    );
                    profile.params = operation.params.clone();
                    profile.notes = "Saved from an operation".to_string();
                    self.profiles.add_operation(profile);
                    self.save_profiles();
                    self.status = "Operation saved as a profile".to_string();
                }
            }
        }
        if self.editor.document != before {
            self.editor.history.record(&before, "Edit operation");
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
                    if self.editor.is_editing_nodes() {
                        ui.separator();
                        self.node_properties(ui);
                    }
                });
                if self.editor.document != before {
                    self.editor.history.record(&before, "Edit properties");
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
        let mut relink_request = false;
        let mut select_same_color: Option<String> = None;
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
                    let mut width = max.x - min.x;
                    if ui
                        .add(
                            egui::DragValue::new(&mut width)
                                .speed(0.1)
                                .range(0.01..=100_000.0),
                        )
                        .changed()
                    {
                        let factor = width / (max.x - min.x).max(1e-9);
                        let vertical = if self.link_proportions { factor } else { 1.0 };
                        let center =
                            Point2::new(f64::midpoint(min.x, max.x), f64::midpoint(min.y, max.y));
                        object.transform = egui_transform_scale(factor, vertical, center)
                            .compose(&object.transform);
                    }
                    ui.end_row();
                    ui.label("Height");
                    let mut height = max.y - min.y;
                    if ui
                        .add(
                            egui::DragValue::new(&mut height)
                                .speed(0.1)
                                .range(0.01..=100_000.0),
                        )
                        .changed()
                    {
                        let factor = height / (max.y - min.y).max(1e-9);
                        let horizontal = if self.link_proportions { factor } else { 1.0 };
                        let center =
                            Point2::new(f64::midpoint(min.x, max.x), f64::midpoint(min.y, max.y));
                        object.transform = egui_transform_scale(horizontal, factor, center)
                            .compose(&object.transform);
                    }
                    ui.end_row();
                    ui.label("Proportions");
                    ui.checkbox(&mut self.link_proportions, "Link width and height");
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
                        let mut font = text.font.clone();
                        if font_picker(ui, "object_text_font", &mut font) {
                            text.font.clone_from(&font);
                            self.preferences.general.text_font = font;
                        }
                        ui.end_row();
                        ui.label("Size");
                        ui.add(
                            egui::DragValue::new(&mut text.size)
                                .speed(0.1)
                                .range(0.1..=10000.0),
                        );
                        ui.end_row();
                        ui.label("Letter spacing");
                        ui.add(
                            egui::DragValue::new(&mut text.letter_spacing)
                                .speed(0.05)
                                .range(-10.0..=50.0)
                                .suffix(" mm"),
                        );
                        ui.end_row();
                        ui.label("Line spacing");
                        ui.add(
                            egui::Slider::new(&mut text.line_spacing, 0.5..=5.0).fixed_decimals(2),
                        );
                        ui.end_row();
                        ui.label("Align");
                        let mut align = text.align;
                        egui::ComboBox::from_id_salt("text_align")
                            .selected_text(align.label())
                            .show_ui(ui, |ui| {
                                for candidate in [
                                    crate::objects::TextAlign::Left,
                                    crate::objects::TextAlign::Center,
                                    crate::objects::TextAlign::Right,
                                ] {
                                    ui.selectable_value(&mut align, candidate, candidate.label());
                                }
                            });
                        text.align = align;
                        ui.end_row();
                    });
                ui.weak("Use Shift+Enter in the content field for a second line.");
            }
            Shape::Image(image) => {
                use crate::objects::ImageMode;
                ui.label("Image");
                ui.horizontal(|ui| {
                    ui.text_edit_singleline(&mut image.path);
                    if ui.button("Relink...").clicked() {
                        relink_request = true;
                    }
                });
                ui.weak(format!(
                    "{:.1} x {:.1} mm, source {} x {} px at {:.0} dpi",
                    image.width_mm(),
                    image.height_mm(),
                    image.width_px,
                    image.height_px,
                    image.dpi
                ));
                let (work_width, work_height) = image.effective_size_px();
                ui.weak(format!(
                    "Working: {work_width} x {work_height} px, {:.0} dpi effective",
                    image.effective_dpi()
                ));
                if image.effective_dpi() < 100.0 {
                    ui.colored_label(
                        Color32::from_rgb(180, 120, 0),
                        "Low working resolution: details may be lost.",
                    );
                }
                ui.separator();
                ui.label("Transform (non-destructive)");
                ui.horizontal(|ui| {
                    ui.label("Rotation");
                    let mut rotation = image.settings.rotation_degrees;
                    egui::ComboBox::from_id_salt("image_rotation")
                        .selected_text(format!("{rotation:.0} deg"))
                        .show_ui(ui, |ui| {
                            for candidate in [0.0, 90.0, 180.0, 270.0] {
                                ui.selectable_value(
                                    &mut rotation,
                                    candidate,
                                    format!("{candidate:.0} deg"),
                                );
                            }
                        });
                    image.settings.rotation_degrees = rotation;
                });
                ui.horizontal(|ui| {
                    ui.checkbox(&mut image.settings.flip_horizontal, "Flip horizontal");
                    ui.checkbox(&mut image.settings.flip_vertical, "Flip vertical");
                });
                ui.horizontal(|ui| {
                    ui.label("Working size");
                    ui.add(
                        egui::DragValue::new(&mut image.settings.target_width_px)
                            .speed(1.0)
                            .range(0..=20000)
                            .suffix(" px X"),
                    );
                    ui.add(
                        egui::DragValue::new(&mut image.settings.target_height_px)
                            .speed(1.0)
                            .range(0..=20000)
                            .suffix(" px Y"),
                    );
                });
                ui.weak("0 x 0 keeps the source resolution.");
                let mut crop_x = image.crop.map_or(0, |crop| crop.x);
                let mut crop_y = image.crop.map_or(0, |crop| crop.y);
                let mut crop_w = image.crop.map_or(image.width_px, |crop| crop.width);
                let mut crop_h = image.crop.map_or(image.height_px, |crop| crop.height);
                ui.horizontal(|ui| {
                    ui.label("Crop");
                    ui.add(egui::DragValue::new(&mut crop_x).speed(1.0).prefix("X "));
                    ui.add(egui::DragValue::new(&mut crop_y).speed(1.0).prefix("Y "));
                    ui.add(egui::DragValue::new(&mut crop_w).speed(1.0).prefix("W "));
                    ui.add(egui::DragValue::new(&mut crop_h).speed(1.0).prefix("H "));
                });
                ui.horizontal(|ui| {
                    if ui.button("Apply crop").clicked() {
                        image.crop = Some(crate::objects::ImageCrop {
                            x: crop_x,
                            y: crop_y,
                            width: crop_w,
                            height: crop_h,
                        });
                    }
                    if ui.button("Clear crop").clicked() {
                        image.crop = None;
                    }
                });
                if let Some(crop) = image.crop {
                    ui.weak(format!(
                        "Cropped to {} x {} px at ({}, {})",
                        crop.width, crop.height, crop.x, crop.y
                    ));
                }
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
                if matches!(image.settings.mode, ImageMode::Sketch) {
                    ui.weak("In sketch mode the threshold sets the edge strength.");
                }
                if let Err(error) = image.settings.validate() {
                    ui.colored_label(Color32::from_rgb(200, 60, 60), error);
                }
                if ui.button("Reset preparation").clicked() {
                    image.settings = crate::objects::ImageSettings::default();
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
        ui.horizontal(|ui| {
            if let Some(color) = object.style.stroke.clone() {
                if ui.button("Select same stroke").clicked() {
                    select_same_color = Some(color);
                }
            }
            if let Some(color) = object.style.fill.clone() {
                if ui.button("Select same fill").clicked() {
                    select_same_color = Some(color);
                }
            }
        });
        if let Some(color) = select_same_color {
            self.editor.select_by_color(&color);
        }
        if relink_request {
            self.relink_target = Some(id.to_string());
            self.open_dialog(FileMode::RelinkImage);
        }
    }

    fn node_properties(&mut self, ui: &mut Ui) {
        ui.heading("Path nodes");
        let count = self.editor.node_points().len();
        ui.label(format!(
            "{count} node(s), {} selected",
            self.editor.node_selection.len()
        ));
        if self.editor.node_selection.len() == 1 {
            let index = self.editor.node_selection[0];
            if let Some(point) = self.editor.node_points().get(index).copied() {
                let mut x = point.x;
                let mut y = point.y;
                ui.horizontal(|ui| {
                    ui.label("X");
                    if ui
                        .add(egui::DragValue::new(&mut x).speed(0.1).suffix(" mm"))
                        .changed()
                    {
                        self.editor
                            .set_selected_node_position(Point2::new(x, point.y));
                    }
                    ui.label("Y");
                    if ui
                        .add(egui::DragValue::new(&mut y).speed(0.1).suffix(" mm"))
                        .changed()
                    {
                        self.editor
                            .set_selected_node_position(Point2::new(point.x, y));
                    }
                });
            }
        }
        let has_nodes = !self.editor.node_selection.is_empty();
        ui.horizontal(|ui| {
            if ui
                .add_enabled(has_nodes, egui::Button::new("Corner"))
                .clicked()
            {
                self.editor.convert_selected_nodes(false);
            }
            if ui
                .add_enabled(has_nodes, egui::Button::new("Smooth"))
                .clicked()
            {
                self.editor.convert_selected_nodes(true);
            }
            if ui
                .add_enabled(has_nodes, egui::Button::new("Delete node"))
                .clicked()
            {
                let removed = self.editor.delete_selected_nodes();
                self.status = format!("Deleted {removed} node(s)");
            }
        });
        ui.horizontal(|ui| {
            if ui
                .add_enabled(has_nodes, egui::Button::new("Align handles"))
                .clicked()
            {
                let aligned = self.editor.align_selected_node_handles();
                self.status = format!("Aligned {aligned} node handle(s)");
            }
            if ui
                .add_enabled(
                    self.editor.node_selection.len() == 1,
                    egui::Button::new("Open at node"),
                )
                .clicked()
            {
                self.editor.open_path_at_selected_node();
            }
            if ui
                .add_enabled(
                    self.editor.node_selection.len() == 1,
                    egui::Button::new("Split at node"),
                )
                .clicked()
                && self.editor.split_path_at_selected_node().is_some()
            {
                self.status = "Path split".to_string();
            }
        });
        ui.weak("Double-click a segment to insert a node.");
    }

    fn multi_selection_properties(&mut self, ui: &mut Ui) {
        ui.label(format!("{} objects selected", self.editor.selection.len()));
        if let Some((min, max)) = self.editor.selected_bounds() {
            let mut left = min.x;
            let mut bottom = min.y;
            let mut width = max.x - min.x;
            let mut height = max.y - min.y;
            let mut changed = false;
            egui::Grid::new("multi_bounds")
                .num_columns(2)
                .spacing([8.0, 6.0])
                .show(ui, |ui| {
                    ui.label("Left");
                    changed |= ui
                        .add(egui::DragValue::new(&mut left).speed(0.1).suffix(" mm"))
                        .changed();
                    ui.end_row();
                    ui.label("Bottom");
                    changed |= ui
                        .add(egui::DragValue::new(&mut bottom).speed(0.1).suffix(" mm"))
                        .changed();
                    ui.end_row();
                    ui.label("Width");
                    changed |= ui
                        .add(
                            egui::DragValue::new(&mut width)
                                .speed(0.1)
                                .range(0.01..=100_000.0)
                                .suffix(" mm"),
                        )
                        .changed();
                    ui.end_row();
                    ui.label("Height");
                    changed |= ui
                        .add(
                            egui::DragValue::new(&mut height)
                                .speed(0.1)
                                .range(0.01..=100_000.0)
                                .suffix(" mm"),
                        )
                        .changed();
                    ui.end_row();
                });
            if changed {
                if !self.multi_props_recorded {
                    self.editor.record_action("Multi-selection transform");
                    self.multi_props_recorded = true;
                }
                let dx = left - min.x;
                let dy = bottom - min.y;
                if dx.abs() > 1e-9 || dy.abs() > 1e-9 {
                    self.editor.move_selection(dx, dy);
                }
                if (width - (max.x - min.x)).abs() > 1e-9 || (height - (max.y - min.y)).abs() > 1e-9
                {
                    self.editor.set_selection_size(width, height, false);
                }
            }
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
                if self.zoom_to_selection_request {
                    self.editor
                        .zoom_to_selection(f64::from(rect.width()), f64::from(rect.height()));
                    self.zoom_to_selection_request = false;
                }
                let ctx = ui.ctx().clone();
                let modal = self.dialog.is_some()
                    || self.confirm.is_some()
                    || self.about_open
                    || self.task.is_some()
                    || self.machines_open
                    || self.machine_wizard_open
                    || self.cuts_open;
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
                            let step = if self.editor.scroll_zoom_step > 1.0 {
                                self.editor.scroll_zoom_step
                            } else {
                                1.15
                            };
                            let factor = if scroll > 0.0 { step } else { 1.0 / step };
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

                if self.editor.tool != Tool::Bezier && !self.pen_nodes.is_empty() {
                    self.pen_nodes.clear();
                    self.pen_drag_start = None;
                }
                if self.editor.tool == Tool::Bezier && !modal {
                    if primary_pressed {
                        if let Some(pos) = pointer.filter(|pos| rect.contains(*pos)) {
                            let point = self.doc_point(rect, pos);
                            let tolerance = 8.0 / self.editor.canvas.zoom.max(0.01);
                            if self.pen_nodes.len() >= 2
                                && self.pen_nodes[0].point.distance(point) <= tolerance
                            {
                                self.finish_pen(true);
                            } else {
                                self.pen_drag_start = Some(point);
                            }
                        }
                    } else if primary_released {
                        if let Some(start) = self.pen_drag_start.take() {
                            if let Some(pos) = pointer.filter(|pos| rect.contains(*pos)) {
                                let point = self.doc_point(rect, pos);
                                let (handle_in, handle_out) = if start.distance(point) > 0.5 {
                                    (
                                        Some(Point2::new(
                                            2.0 * start.x - point.x,
                                            2.0 * start.y - point.y,
                                        )),
                                        Some(point),
                                    )
                                } else {
                                    (None, None)
                                };
                                self.pen_nodes.push(crate::geometry::path::Node {
                                    point: start,
                                    handle_in,
                                    handle_out,
                                });
                                self.status =
                                    "Pen: click to add nodes, Enter to finish, Esc to cancel"
                                        .to_string();
                            }
                        }
                    }
                }

                if primary_pressed && pointer_in_canvas {
                    response.request_focus();
                    if let Some(pos) = pointer {
                        let point = self.doc_point(rect, pos);
                        if self.editor.is_editing_nodes() {
                            self.begin_node_interaction(point, shift);
                            if primary_released {
                                self.drag = None;
                            }
                        } else if self.editor.tool == Tool::Select
                            && let Some(kind) = self.selection_handle_at(rect, pos)
                        {
                            let centre = self.selection_doc_centre().unwrap_or(point);
                            self.drag = Some(DragState::Transform {
                                kind,
                                last: point,
                                centre,
                                recorded: false,
                                constrain: shift,
                            });
                        } else {
                            match self.editor.tool {
                                Tool::Select => self.begin_select(point, shift),
                                Tool::Bezier => {}
                                Tool::Rect
                                | Tool::Ellipse
                                | Tool::Line
                                | Tool::Polygon
                                | Tool::Star
                                | Tool::Text
                                | Tool::Measure
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
                                            constrain: shift,
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
                }

                if self.editor.is_editing_nodes() && pointer_in_canvas && response.double_clicked()
                {
                    if let Some(pos) = pointer {
                        let point = self.doc_point(rect, pos);
                        if let Some((segment, t)) = self.editor.nearest_segment_on_path(point) {
                            if self.editor.insert_node_at(segment, t).is_some() {
                                self.status = "Node inserted".to_string();
                                self.drag = None;
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
                    Some(DragState::Draw {
                        tool, start, end, ..
                    }) => Some(match tool {
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
                let preview = if self.editor.tool == Tool::Bezier && self.pen_nodes.len() >= 2 {
                    Some(PreviewShape::Polyline(
                        self.pen_nodes.iter().map(|node| node.point).collect(),
                    ))
                } else {
                    preview
                };
                let theme = Theme {
                    dark: self.dark_mode,
                };
                let painter = ui.painter_at(rect);
                let toolpaths = if self.preview_toolpaths
                    || (self.preview_open && self.preview_order)
                {
                    let (paths, _) = crate::gcode::build_toolpaths(&self.editor.document);
                    Some(
                        paths
                            .into_iter()
                            .filter(|path| {
                                path.operation_id.as_deref().is_none_or(|id| {
                                    self.editor
                                        .document
                                        .operations
                                        .iter()
                                        .find(|operation| operation.id == id)
                                        .is_none_or(|operation| operation.params.show_in_preview)
                                })
                            })
                            .collect::<Vec<_>>(),
                    )
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
                let nest_overlay = if self.nest_open {
                    self.nest_proposal
                        .as_ref()
                        .map(|proposal| proposal.placements.as_slice())
                } else {
                    None
                };
                let overlays = crate::editor::renderer::CanvasOverlays {
                    toolpaths: toolpaths.as_deref(),
                    images: Some(&textures),
                    playback: playback_overlay,
                    nest: nest_overlay,
                    show_order: self.preview_open && self.preview_order,
                    guides: Some(self.smart_guides.as_slice()),
                };
                paint_canvas(&painter, rect, &self.editor, theme, preview, &overlays);

                if pointer_in_canvas {
                    let cursor = match self.editor.tool {
                        Tool::Pan => egui::CursorIcon::Grab,
                        Tool::Select => {
                            match pointer.and_then(|pos| self.selection_handle_at(rect, pos)) {
                                Some(HandleKind::Move) => egui::CursorIcon::Grab,
                                Some(HandleKind::Rotate) => egui::CursorIcon::Crosshair,
                                Some(HandleKind::Scale(index)) => match index {
                                    0 | 4 => egui::CursorIcon::ResizeNwSe,
                                    2 | 6 => egui::CursorIcon::ResizeNeSw,
                                    1 | 5 => egui::CursorIcon::ResizeVertical,
                                    _ => egui::CursorIcon::ResizeHorizontal,
                                },
                                None => egui::CursorIcon::Default,
                            }
                        }
                        _ => egui::CursorIcon::Crosshair,
                    };
                    ctx.set_cursor_icon(cursor);
                }
                if !modal && response.secondary_clicked() {
                    if let Some(pos) = response.interact_pointer_pos() {
                        let point = self.doc_point(rect, pos);
                        let tolerance = 8.0 / self.editor.canvas.zoom.max(0.01);
                        let hit = self.editor.object_at(point, tolerance);
                        if let Some(id) = &hit {
                            if !self.editor.selection.contains(id) {
                                self.editor.select_at(point, false, tolerance);
                            }
                        }
                        self.context_object = hit;
                        self.context_point = Some(point);
                    }
                }
                if !modal {
                    response.context_menu(|ui| {
                        if self.context_object.is_some() {
                            self.object_context_menu(ui);
                        } else {
                            self.canvas_context_menu(ui);
                        }
                    });
                }
                // Canvas edits are applied after the side panels have already
                // been painted; request one more frame so they show the result.
                if primary_pressed || primary_released || self.drag.is_some() {
                    ctx.request_repaint();
                }
            });
    }

    fn object_context_menu(&mut self, ui: &mut Ui) {
        let count = self.editor.selection.len();
        let first = self.editor.selection.ids().first().cloned();
        let text_selected = count == 1
            && first.as_ref().is_some_and(|id| {
                self.editor
                    .document
                    .objects
                    .iter()
                    .any(|object| &object.id == id && matches!(object.shape, Shape::Text(_)))
            });
        let image_selected = count == 1
            && first.as_ref().is_some_and(|id| {
                self.editor
                    .document
                    .objects
                    .iter()
                    .any(|object| &object.id == id && matches!(object.shape, Shape::Image(_)))
            });
        let has_selection = count > 0;
        for action in object_context_actions(count, text_selected, image_selected) {
            if matches!(
                action,
                ContextAction::ToFront
                    | ContextAction::Lock
                    | ContextAction::EditNodes
                    | ContextAction::ExportSvg
            ) {
                ui.separator();
            }
            let enabled = match action {
                ContextAction::Cut
                | ContextAction::Copy
                | ContextAction::Duplicate
                | ContextAction::Delete
                | ContextAction::ToFront
                | ContextAction::ToBack
                | ContextAction::Lock
                | ContextAction::Unlock
                | ContextAction::Ungroup
                | ContextAction::ExportSvg
                | ContextAction::ExportDxf
                | ContextAction::ExportGcode => has_selection,
                ContextAction::Group => count >= 2,
                ContextAction::EditNodes => count == 1,
                ContextAction::TextToOutlines => text_selected,
                ContextAction::AdjustImage => image_selected,
                _ => true,
            };
            if icons::button_enabled(ui, enabled, action.icon(), action.label()).clicked() {
                self.run_context_action(action);
                ui.close();
            }
        }
    }

    fn canvas_context_menu(&mut self, ui: &mut Ui) {
        for action in canvas_context_actions() {
            match action {
                ContextAction::Paste => {
                    if icons::button_enabled(ui, !self.clipboard.is_empty(), Icon::Paste, "Paste")
                        .clicked()
                    {
                        self.run_context_action(action);
                        ui.close();
                    }
                }
                _ => {
                    if icons::button(ui, action.icon(), action.label()).clicked() {
                        self.run_context_action(action);
                        ui.close();
                    }
                }
            }
        }
        ui.separator();
        icons::checkbox(
            ui,
            &mut self.editor.canvas.grid_visible,
            Icon::Grid,
            "Show grid",
        );
        if icons::checkbox(
            ui,
            &mut self.editor.show_selection,
            Icon::Eye,
            "Show selection",
        )
        .changed()
        {
            self.preferences.drawing.show_selection = self.editor.show_selection;
        }
        icons::checkbox(
            ui,
            &mut self.editor.show_rulers,
            Icon::Rulers,
            "Show rulers",
        );
        icons::checkbox(
            ui,
            &mut self.editor.guides_visible,
            Icon::Guides,
            "Show guides",
        );
        icons::checkbox(
            ui,
            &mut self.editor.snap_enabled,
            Icon::Snap,
            "Snap to grid",
        );
    }

    fn run_context_action(&mut self, action: ContextAction) {
        match action {
            ContextAction::Cut => self.cut(),
            ContextAction::Copy => self.copy(),
            ContextAction::Paste => self.paste(),
            ContextAction::Duplicate => {
                self.editor.duplicate_selection();
            }
            ContextAction::Delete => {
                self.editor.delete_selection();
            }
            ContextAction::ToFront => {
                let count = self.editor.bring_selection_to_front();
                self.status = format!("Brought {count} object(s) to the front");
            }
            ContextAction::ToBack => {
                let count = self.editor.send_selection_to_back();
                self.status = format!("Sent {count} object(s) to the back");
            }
            ContextAction::Group => {
                self.editor.group_selection();
            }
            ContextAction::Ungroup => {
                self.editor.ungroup_selection();
            }
            ContextAction::Lock => {
                let count = self.editor.lock_selection();
                self.status = format!("Locked {count} object(s)");
            }
            ContextAction::Unlock => {
                let count = self.editor.unlock_selection();
                self.status = format!("Unlocked {count} object(s)");
            }
            ContextAction::EditNodes => {
                if let Some(id) = self.editor.selection.ids().first().cloned() {
                    match self.editor.begin_node_edit(&id) {
                        Ok(()) => {
                            self.status =
                                "Editing nodes: drag to move, double-click a segment to insert"
                                    .to_string();
                        }
                        Err(error) => self.status = format!("Cannot edit nodes: {error}"),
                    }
                }
            }
            ContextAction::TextToOutlines => self.apply_text_outlines(),
            ContextAction::AdjustImage => self.open_image_adjust(),
            ContextAction::ExportSvg => {
                self.export_subset = true;
                self.open_dialog(FileMode::ExportSvg);
            }
            ContextAction::ExportDxf => {
                self.export_subset = true;
                self.open_dialog(FileMode::ExportDxf);
            }
            ContextAction::ExportGcode => {
                self.export_subset = true;
                self.open_dialog(FileMode::ExportGcode);
            }
            ContextAction::SelectAll => {
                self.editor.select_all();
            }
            ContextAction::ZoomIn => self.editor.zoom_in(),
            ContextAction::ZoomOut => self.editor.zoom_out(),
            ContextAction::FitPage => self.needs_fit = true,
            ContextAction::ZoomSelection => self.zoom_to_selection_request = true,
            ContextAction::Guides => self.guides_open = true,
            ContextAction::DocumentProperties => self.open_document_properties(),
            ContextAction::Preview => self.open_preview(),
            ContextAction::Preferences => self.preferences_open = true,
        }
    }

    fn remember_recent(&mut self, path: &Path) {
        let entry = path.display().to_string();
        let mut recent = std::mem::take(&mut self.preferences.general.recent_projects);
        recent.retain(|existing| existing != &entry);
        recent.insert(0, entry);
        recent.truncate(10);
        self.preferences.general.recent_projects = recent;
        let _ = self.preferences.save(&self.preferences_path);
    }

    fn shortcuts_window(&mut self, ctx: &egui::Context) {
        let mut open = self.help_shortcuts_open;
        egui::Window::new("Keyboard shortcuts")
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(520.0)
            .show(ctx, |ui| {
                egui::Grid::new("shortcut_table")
                    .num_columns(2)
                    .striped(true)
                    .spacing([20.0, 4.0])
                    .show(ui, |ui| {
                        for (keys, action) in [
                            ("Ctrl+N / Ctrl+O / Ctrl+S", "New, open and save"),
                            ("Ctrl+Shift+S", "Save as"),
                            ("Ctrl+Z / Ctrl+Y / Ctrl+Shift+Z", "Undo and redo"),
                            ("Ctrl+X / Ctrl+C / Ctrl+V", "Cut, copy and paste"),
                            ("Ctrl+D", "Duplicate"),
                            ("Ctrl+A", "Select all"),
                            ("Delete / Backspace", "Delete the selection or selected nodes"),
                            ("Ctrl+G / Ctrl+Shift+G", "Group and ungroup"),
                            ("Arrows / Shift+Arrows", "Nudge by 1 mm or 0.1 mm"),
                            ("Escape", "Cancel the drag, leave node editing or clear the selection"),
                            ("V / L / R / E / S / T / P / B", "Select, line, rectangle, ellipse, star, text, polygon, freehand"),
                            ("F", "Fit the page"),
                            ("G", "Toggle the grid"),
                            ("+ / -", "Zoom in and out"),
                            ("Ctrl+P", "Preview and G-code"),
                            ("F9 / F10", "Layers and properties panels"),
                            ("Enter", "Finish a Bezier path"),
                        ] {
                            ui.label(keys);
                            ui.label(action);
                            ui.end_row();
                        }
                    });
                ui.separator();
                ui.weak(
                    "Ctrl and Shift combinations work with the keyboard layout in use; the tools never capture keys while a text field has focus.",
                );
            });
        self.help_shortcuts_open = open;
    }

    fn open_cuts(&mut self) {
        self.cuts_snapshot = Some(self.editor.document.clone());
        if self.active_operation.is_none() {
            self.active_operation = self
                .editor
                .document
                .operations
                .first()
                .map(|operation| operation.id.clone());
        }
        self.cuts_open = true;
    }

    fn cuts_window(&mut self, ctx: &egui::Context) {
        use crate::operations::OperationType;

        let mut open = self.cuts_open;
        let mut close_requested = false;
        let mut operations = self.editor.document.operations.clone();
        egui::Window::new("Cuts / Layers")
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_size([760.0, 560.0])
            .show(ctx, |ui| {
                ui.label("One row per cut. Output controls the job, Show controls the preview, Air emits M8/M9.");
                ui.separator();
                let mut row_centers: Vec<(usize, f32)> = Vec::new();
                egui::ScrollArea::vertical()
                    .max_height(240.0)
                    .show(ui, |ui| {
                        egui::Grid::new("cuts_table")
                            .num_columns(7)
                            .striped(true)
                            .spacing([12.0, 6.0])
                            .show(ui, |ui| {
                                for header in ["#", "Layer", "Mode", "Spd/Pwr", "Output", "Show", "Air"] {
                                    ui.strong(header);
                                }
                                ui.end_row();
                                for (index, operation) in operations.iter_mut().enumerate() {
                                    let number = ui.add(
                                        egui::Label::new(format!("{:02}", index + 1))
                                            .sense(egui::Sense::drag()),
                                    );
                                    row_centers.push((index, number.rect.center().y));
                                    if number.drag_started() {
                                        self.cuts_drag = Some(index);
                                    }
                                    ui.horizontal(|ui| {
                                        let rgb = parse_hex_color(&operation.params.color)
                                            .unwrap_or((0, 0, 0));
                                        let mut srgb = [rgb.0, rgb.1, rgb.2];
                                        if ui.color_edit_button_srgb(&mut srgb).changed() {
                                            operation.params.color =
                                                to_hex(srgb[0], srgb[1], srgb[2]);
                                        }
                                        let selected = self.active_operation.as_deref()
                                            == Some(operation.id.as_str());
                                        if ui
                                            .selectable_label(selected, &operation.name)
                                            .clicked()
                                        {
                                            self.active_operation =
                                                Some(operation.id.clone());
                                        }
                                    });
                                    let mut op_type = operation.op_type;
                                    egui::ComboBox::from_id_salt(("cut_mode", index))
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
                                    operation.op_type = op_type;
                                    ui.label(format!(
                                        "{:.0} / {:.0}",
                                        operation.params.speed, operation.params.power
                                    ));
                                    ui.checkbox(&mut operation.enabled, "");
                                    ui.checkbox(&mut operation.params.show_in_preview, "");
                                    ui.checkbox(&mut operation.params.air_assist, "");
                                    ui.end_row();
                                }
                            });
                    });
                if self.cuts_drag.is_some() {
                    if let Some(pos) =
                        ui.ctx().input(|input| input.pointer.interact_pos())
                    {
                        self.cuts_drag_target = row_centers
                            .iter()
                            .min_by(|left, right| {
                                (left.1 - pos.y).abs().total_cmp(&(right.1 - pos.y).abs())
                            })
                            .map(|(index, _)| *index);
                    }
                }
                if ui.ctx().input(|input| input.pointer.any_released()) {
                    if let (Some(from), Some(to)) =
                        (self.cuts_drag.take(), self.cuts_drag_target.take())
                    {
                        if from != to {
                            let operation = operations.remove(from);
                            let target = to.min(operations.len());
                            operations.insert(target, operation);
                        }
                    }
                }
                ui.separator();
                if let Some(id) = self.active_operation.clone() {
                    if let Some(index) = operations
                        .iter()
                        .position(|operation| operation.id == id)
                    {
                        let operation = &mut operations[index];
                        ui.heading("Cut settings");
                        egui::Grid::new("cut_settings")
                            .num_columns(2)
                            .spacing([12.0, 6.0])
                            .show(ui, |ui| {
                                ui.label("Name");
                                ui.text_edit_singleline(&mut operation.name);
                                ui.end_row();
                                ui.label("Colour");
                                let rgb = parse_hex_color(&operation.params.color)
                                    .unwrap_or((0, 0, 0));
                                let mut srgb = [rgb.0, rgb.1, rgb.2];
                                if ui.color_edit_button_srgb(&mut srgb).changed() {
                                    operation.params.color =
                                        to_hex(srgb[0], srgb[1], srgb[2]);
                                }
                                ui.end_row();
                                ui.label("Mode");
                                let mut op_type = operation.op_type;
                                egui::ComboBox::from_id_salt("cut_settings_mode")
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
                                operation.op_type = op_type;
                                ui.end_row();
                                ui.label("Output");
                                ui.checkbox(&mut operation.enabled, "");
                                ui.end_row();
                                ui.label("Air assist");
                                ui.checkbox(&mut operation.params.air_assist, "");
                                ui.end_row();
                                ui.label("Show in preview");
                                ui.checkbox(&mut operation.params.show_in_preview, "");
                                ui.end_row();
                                ui.label("Speed (mm/min)");
                                ui.add(
                                    egui::DragValue::new(&mut operation.params.speed)
                                        .speed(10.0)
                                        .range(1.0..=100_000.0),
                                );
                                ui.end_row();
                                ui.label("Max power (%)");
                                ui.add(
                                    egui::DragValue::new(&mut operation.params.power)
                                        .speed(1.0)
                                        .range(0.0..=100.0),
                                );
                                ui.end_row();
                                ui.label("Number of passes");
                                ui.add(
                                    egui::DragValue::new(&mut operation.params.passes)
                                        .range(1..=1000),
                                );
                                ui.end_row();
                                ui.label("Line interval (mm)");
                                ui.add(
                                    egui::DragValue::new(
                                        &mut operation.params.line_interval_mm,
                                    )
                                    .speed(0.01)
                                    .range(0.01..=2.0),
                                );
                                ui.end_row();
                                ui.label("Lines per inch");
                                ui.weak(format!(
                                    "{:.2}",
                                    25.4 / operation.params.line_interval_mm.max(0.001)
                                ));
                                ui.end_row();
                                ui.label("Scan angle (deg)");
                                ui.add(
                                    egui::DragValue::new(
                                        &mut operation.params.fill_angle_degrees,
                                    )
                                    .speed(1.0)
                                    .range(-180.0..=180.0),
                                );
                                ui.end_row();
                                ui.label("Bi-directional fill");
                                ui.checkbox(&mut operation.params.fill_alternate, "");
                                ui.end_row();
                                ui.label("Cross-hatch");
                                ui.checkbox(&mut operation.params.cross_hatch, "");
                                ui.end_row();
                                ui.label("Overscan (mm)");
                                ui.add(
                                    egui::DragValue::new(&mut operation.params.overscan_mm)
                                        .speed(0.1)
                                        .range(0.0..=50.0),
                                );
                                ui.end_row();
                                ui.label("Constant power mode");
                                ui.checkbox(&mut operation.params.constant_power, "");
                                ui.end_row();
                            });
                    }
                } else {
                    ui.weak("No cut selected. Add one below.");
                }
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("+ Cut").clicked() {
                        let operation = crate::operations::Operation::new(
                            format!("Cut {}", operations.len() + 1),
                            OperationType::OutlineCutting,
                        );
                        self.active_operation = Some(operation.id.clone());
                        operations.push(operation);
                    }
                    if ui.button("Duplicate").clicked() {
                        if let Some(id) = self.active_operation.clone() {
                            if let Some(index) = operations
                                .iter()
                                .position(|operation| operation.id == id)
                            {
                                let mut copy = operations[index].clone();
                                copy.id = uuid::Uuid::new_v4().to_string();
                                copy.name = format!("{} copy", copy.name);
                                self.active_operation = Some(copy.id.clone());
                                operations.insert(index + 1, copy);
                            }
                        }
                    }
                    if ui.button("Delete").clicked() {
                        if let Some(id) = self.active_operation.clone() {
                            operations.retain(|operation| operation.id != id);
                            self.active_operation = operations
                                .first()
                                .map(|operation| operation.id.clone());
                        }
                    }
                    if ui.button("Up").clicked() {
                        if let Some(id) = self.active_operation.clone() {
                            if let Some(index) = operations
                                .iter()
                                .position(|operation| operation.id == id)
                            {
                                if index > 0 {
                                    operations.swap(index, index - 1);
                                }
                            }
                        }
                    }
                    if ui.button("Down").clicked() {
                        if let Some(id) = self.active_operation.clone() {
                            if let Some(index) = operations
                                .iter()
                                .position(|operation| operation.id == id)
                            {
                                if index + 1 < operations.len() {
                                    operations.swap(index, index + 1);
                                }
                            }
                        }
                    }
                    if ui.button("Close").clicked() {
                        close_requested = true;
                    }
                });
            });
        self.editor.document.operations = operations;
        if close_requested {
            open = false;
        }
        self.cuts_open = open;
        if !open {
            if let Some(snapshot) = self.cuts_snapshot.take() {
                if snapshot != self.editor.document {
                    self.editor.history.record(&snapshot, "Cuts and layers");
                }
            }
            self.cuts_open = false;
        }
    }

    fn handle_live_request(&mut self, request: &str) -> String {
        let request: serde_json::Value = match serde_json::from_str(request) {
            Ok(value) => value,
            Err(error) => {
                return serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": null,
                    "error": {"code": -32700, "message": format!("invalid JSON: {error}")}
                })
                .to_string();
            }
        };
        let placeholder = crate::editor::EditorState::new(Document::default());
        let app_editor = std::mem::replace(&mut self.editor, placeholder);
        let old = self.live_server.replace_editor(app_editor);
        let response = self.live_server.handle(&request);
        let app_editor = self.live_server.replace_editor(old);
        self.editor = app_editor;
        response.to_string()
    }

    fn machine_wizard(&mut self, ctx: &egui::Context) {
        let mut create = false;
        let mut skip = false;
        egui::Window::new("Welcome - machine profile")
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label(
                    "Save the work area of your machine so new projects start with the right size.",
                );
                ui.label(
                    "This is preparation data only: BeamWright never connects to, configures or controls a machine.",
                );
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label("Name");
                    ui.text_edit_singleline(&mut self.machine_editor.name);
                });
                ui.horizontal(|ui| {
                    ui.label("Work width");
                    ui.add(
                        egui::DragValue::new(&mut self.machine_editor.work_width_mm)
                            .speed(1.0)
                            .range(10.0..=100_000.0)
                            .suffix(" mm"),
                    );
                    ui.label("Height");
                    ui.add(
                        egui::DragValue::new(&mut self.machine_editor.work_height_mm)
                            .speed(1.0)
                            .range(10.0..=100_000.0)
                            .suffix(" mm"),
                    );
                });
                if let Some(error) = &self.machine_error {
                    ui.colored_label(Color32::from_rgb(200, 60, 60), error);
                }
                ui.horizontal(|ui| {
                    if ui.button("Create profile").clicked() {
                        create = true;
                    }
                    if ui.button("Skip for now").clicked() {
                        skip = true;
                    }
                });
            });
        if create {
            match self.machines.add(self.machine_editor.clone()) {
                Ok(()) => {
                    self.machines.first_run_done = true;
                    let _ = self.machines.save(&self.machines_path);
                    self.apply_machine_work_area();
                    self.machine_wizard_open = false;
                    self.status = format!("Machine profile '{}' created", self.machine_editor.name);
                }
                Err(error) => self.machine_error = Some(error),
            }
        } else if skip {
            self.machines.first_run_done = true;
            let _ = self.machines.save(&self.machines_path);
            self.machine_wizard_open = false;
        }
    }

    fn machines_window(&mut self, ctx: &egui::Context) {
        let mut open = self.machines_open;
        egui::Window::new("Machine profiles")
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(520.0)
            .show(ctx, |ui| {
                ui.label(
                    "Work areas used to prepare projects. No connection or controller settings are stored.",
                );
                ui.horizontal(|ui| {
                    if ui.button("Import...").clicked() {
                        self.open_dialog(FileMode::ImportMachines);
                    }
                    if ui.button("Export...").clicked() {
                        self.open_dialog(FileMode::ExportMachines);
                    }
                });
                ui.separator();
                let machines = self.machines.machines.clone();
                ui.horizontal_top(|ui| {
                    ui.vertical(|ui| {
                        ui.heading("Saved");
                        for machine in &machines {
                            if ui
                                .selectable_label(
                                    self.machine_edit_id.as_deref() == Some(&machine.id),
                                    format!(
                                        "{} ({:.0} x {:.0} mm)",
                                        machine.name, machine.work_width_mm, machine.work_height_mm
                                    ),
                                )
                                .clicked()
                            {
                                self.machine_edit_id = Some(machine.id.clone());
                                self.machine_editor = machine.clone();
                                self.machine_error = None;
                            }
                        }
                        if machines.is_empty() {
                            ui.weak("No profiles yet.");
                        }
                    });
                    ui.separator();
                    ui.vertical(|ui| {
                        ui.heading("Profile");
                        ui.horizontal(|ui| {
                            ui.label("Name");
                            ui.text_edit_singleline(&mut self.machine_editor.name);
                        });
                        ui.horizontal(|ui| {
                            ui.label("Work width");
                            ui.add(
                                egui::DragValue::new(&mut self.machine_editor.work_width_mm)
                                    .speed(1.0)
                                    .range(10.0..=100_000.0)
                                    .suffix(" mm"),
                            );
                        });
                        ui.horizontal(|ui| {
                            ui.label("Work height");
                            ui.add(
                                egui::DragValue::new(&mut self.machine_editor.work_height_mm)
                                    .speed(1.0)
                                    .range(10.0..=100_000.0)
                                    .suffix(" mm"),
                            );
                        });
                        ui.label("Notes");
                        ui.text_edit_multiline(&mut self.machine_editor.notes);
                        if let Some(error) = &self.machine_error {
                            ui.colored_label(Color32::from_rgb(200, 60, 60), error);
                        }
                        ui.horizontal(|ui| {
                            if ui.button("New").clicked() {
                                self.machine_editor = crate::machines::MachineProfile::default();
                                self.machine_edit_id = None;
                                self.machine_error = None;
                            }
                            if ui.button("Save profile").clicked() {
                                match self.machine_editor.validate() {
                                    Ok(()) => {
                                        if let Some(id) = &self.machine_edit_id {
                                            if let Some(index) = self
                                                .machines
                                                .machines
                                                .iter()
                                                .position(|machine| &machine.id == id)
                                            {
                                                self.machines.machines[index] =
                                                    self.machine_editor.clone();
                                            }
                                        } else {
                                            match self
                                                .machines
                                                .add(self.machine_editor.clone())
                                            {
                                                Ok(()) => {
                                                    self.machine_edit_id =
                                                        Some(self.machine_editor.id.clone());
                                                }
                                                Err(error) => {
                                                    self.machine_error = Some(error);
                                                }
                                            }
                                        }
                                        match self.machines.validate() {
                                            Ok(()) => match self.machines.save(&self.machines_path)
                                            {
                                                Ok(()) => {
                                                    self.status = "Machine profiles saved"
                                                        .to_string();
                                                    self.machine_error = None;
                                                }
                                                Err(error) => {
                                                    self.machine_error = Some(error);
                                                }
                                            },
                                            Err(error) => self.machine_error = Some(error),
                                        }
                                    }
                                    Err(error) => self.machine_error = Some(error),
                                }
                            }
                            if ui.button("Use work area").clicked() {
                                self.apply_machine_work_area();
                            }
                            if ui.button("Delete").clicked() {
                                if let Some(id) = self.machine_edit_id.take() {
                                    self.machines.remove(&id);
                                    let _ = self.machines.save(&self.machines_path);
                                    self.machine_editor =
                                        crate::machines::MachineProfile::default();
                                }
                            }
                        });
                    });
                });
            });
        self.machines_open = open;
    }

    fn apply_machine_work_area(&mut self) {
        self.editor.record_action("Machine work area");
        self.editor.document.width_mm = self.machine_editor.work_width_mm;
        self.editor.document.height_mm = self.machine_editor.work_height_mm;
        self.needs_fit = true;
        self.status = format!(
            "Work area set to {:.0} x {:.0} mm",
            self.machine_editor.work_width_mm, self.machine_editor.work_height_mm
        );
    }

    fn open_image_adjust(&mut self) {
        let Some(id) = self.editor.selection.ids().first().cloned() else {
            return;
        };
        let is_image = self.editor.document.objects.iter().any(|object| {
            object.id == id && matches!(object.shape, crate::document::Shape::Image(_))
        });
        if !is_image {
            return;
        }
        self.image_adjust_snapshot = Some(self.editor.document.clone());
        self.image_adjust = Some(id);
    }

    #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
    fn original_image_texture(
        &mut self,
        ctx: &egui::Context,
        image: &crate::objects::ImageObject,
    ) -> Option<egui::TextureHandle> {
        let key = format!(
            "{}|{:?}|{}",
            image.path, image.crop, image.settings.rotation_degrees
        );
        if let Some(texture) = self.image_original_previews.get(&key) {
            return Some(texture.clone());
        }
        let mut picture = image::open(&image.path).ok()?;
        if let Some(crop) = image.crop {
            picture = picture.crop_imm(crop.x, crop.y, crop.width, crop.height);
        }
        let rotation = image.settings.rotation_degrees;
        if (rotation - 90.0).abs() < f64::EPSILON {
            picture = picture.rotate90();
        } else if (rotation - 180.0).abs() < f64::EPSILON {
            picture = picture.rotate180();
        } else if (rotation - 270.0).abs() < f64::EPSILON {
            picture = picture.rotate270();
        }
        let width = picture.width().max(1);
        let height = picture.height().max(1);
        let scale = (480.0 / width.max(height) as f32).min(1.0);
        if scale < 1.0 {
            let target_width = ((width as f32) * scale).max(1.0) as u32;
            let target_height = ((height as f32) * scale).max(1.0) as u32;
            picture = picture.resize(
                target_width,
                target_height,
                image::imageops::FilterType::Triangle,
            );
        }
        let rgba = picture.to_rgba8();
        let color = egui::ColorImage::from_rgba_unmultiplied(
            [rgba.width() as usize, rgba.height() as usize],
            rgba.as_raw(),
        );
        let texture = ctx.load_texture(
            format!("image-original-{key}"),
            color,
            egui::TextureOptions::LINEAR,
        );
        self.image_original_previews.insert(key, texture.clone());
        Some(texture)
    }

    fn image_adjust_window(&mut self, ctx: &egui::Context) {
        let Some(id) = self.image_adjust.clone() else {
            return;
        };
        let Some(index) = self
            .editor
            .document
            .objects
            .iter()
            .position(|object| object.id == id)
        else {
            self.image_adjust = None;
            return;
        };
        let crate::document::Shape::Image(image) = &self.editor.document.objects[index].shape
        else {
            self.image_adjust = None;
            return;
        };
        let mut image = image.clone();
        let object_name = self.editor.document.objects[index].name.clone();
        let original = image.clone();
        let original_texture = self.original_image_texture(ctx, &image);
        let processed_texture = self
            .image_previews
            .get(&id)
            .map(|preview| preview.texture.clone());

        let mut close = false;
        let mut cancel = false;
        let mut reset = false;
        let mut preset: Option<crate::objects::ImageSettings> = None;
        egui::Window::new(format!("Adjust image - {object_name}"))
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .collapsible(false)
            .resizable(true)
            .default_width(900.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label("Original");
                        if let Some(texture) = &original_texture {
                            ui.add(
                                egui::Image::new(texture)
                                    .max_size(egui::vec2(410.0, 300.0))
                                    .shrink_to_fit(),
                            );
                        } else {
                            ui.weak("Cannot read the image file");
                        }
                        ui.weak(format!("{} x {} px", image.width_px, image.height_px));
                    });
                    ui.separator();
                    ui.vertical(|ui| {
                        ui.label("Preview");
                        if let Some(texture) = &processed_texture {
                            ui.add(
                                egui::Image::new(texture)
                                    .max_size(egui::vec2(410.0, 300.0))
                                    .shrink_to_fit(),
                            );
                        } else {
                            ui.weak("No preview available");
                        }
                        let (work_width, work_height) = image.effective_size_px();
                        ui.weak(format!(
                            "{work_width} x {work_height} px at {:.0} dpi",
                            image.effective_dpi()
                        ));
                    });
                });
                ui.separator();
                ui.horizontal(|ui| {
                    egui::Grid::new("image_adjust_layers")
                        .num_columns(2)
                        .spacing([12.0, 6.0])
                        .show(ui, |ui| {
                            ui.label("Image mode");
                            let mut mode = image.settings.mode;
                            egui::ComboBox::from_id_salt("image_adjust_mode")
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
                            image.settings.mode = mode;
                            ui.end_row();
                            ui.label("Negative image");
                            ui.checkbox(&mut image.settings.invert, "");
                            ui.end_row();
                            ui.label("Cells per inch");
                            ui.weak(format!(
                                "{:.1}",
                                25.4 / image.settings.line_interval_mm.max(0.001)
                            ));
                            ui.end_row();
                            ui.label("Line interval (mm)");
                            ui.add(
                                egui::DragValue::new(&mut image.settings.line_interval_mm)
                                    .speed(0.01)
                                    .range(0.01..=2.0),
                            );
                            ui.end_row();
                            ui.label("Halftone angle");
                            ui.add(
                                egui::DragValue::new(&mut image.settings.halftone_angle_degrees)
                                    .speed(0.5)
                                    .range(-360.0..=360.0)
                                    .suffix(" deg"),
                            );
                            ui.end_row();
                            ui.label("DPI");
                            ui.weak(format!("{:.0}", image.effective_dpi()));
                            ui.end_row();
                        });
                    ui.separator();
                    egui::Grid::new("image_adjust_settings")
                        .num_columns(2)
                        .spacing([12.0, 6.0])
                        .show(ui, |ui| {
                            ui.label("Contrast");
                            ui.add(
                                egui::Slider::new(&mut image.settings.contrast, 0.25..=3.0)
                                    .fixed_decimals(2),
                            );
                            ui.end_row();
                            ui.label("Brightness");
                            ui.add(
                                egui::Slider::new(&mut image.settings.brightness, -100..=100)
                                    .fixed_decimals(0),
                            );
                            ui.end_row();
                            ui.label("Gamma");
                            ui.add(
                                egui::Slider::new(&mut image.settings.gamma, 0.2..=3.0)
                                    .fixed_decimals(2),
                            );
                            ui.end_row();
                            ui.label("Enhance radius");
                            ui.add(
                                egui::DragValue::new(&mut image.settings.enhance_radius_px)
                                    .range(0..=32)
                                    .suffix(" px"),
                            );
                            ui.end_row();
                            ui.label("Enhance amount");
                            ui.add(
                                egui::DragValue::new(&mut image.settings.enhance_amount)
                                    .speed(0.05)
                                    .range(0.0..=5.0)
                                    .fixed_decimals(2),
                            );
                            ui.end_row();
                        });
                });
                ui.separator();
                ui.horizontal(|ui| {
                    egui::ComboBox::from_id_salt("image_adjust_presets")
                        .selected_text("Presets")
                        .width(180.0)
                        .show_ui(ui, |ui| {
                            for profile in &self.profiles.images {
                                if ui.selectable_label(false, &profile.name).clicked() {
                                    preset = Some(profile.settings);
                                }
                            }
                        });
                    if ui.button("Reset all").clicked() {
                        reset = true;
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("OK").clicked() {
                            close = true;
                        }
                        if ui.button("Cancel").clicked() {
                            cancel = true;
                        }
                    });
                });
            });

        if let Some(preset) = preset {
            image.settings = preset;
        }
        if reset {
            image.settings = crate::objects::ImageSettings::default();
        }
        if cancel {
            self.image_adjust_snapshot = None;
            if let Some(current) = self
                .editor
                .document
                .objects
                .iter_mut()
                .find(|object| object.id == id)
            {
                if let crate::document::Shape::Image(current) = &mut current.shape {
                    current.settings = original.settings;
                }
            }
            self.image_adjust = None;
            return;
        }
        let settings_changed = image.settings != original.settings;
        if let Some(current) = self
            .editor
            .document
            .objects
            .iter_mut()
            .find(|object| object.id == id)
        {
            if let crate::document::Shape::Image(current) = &mut current.shape {
                current.settings = image.settings;
            }
        }
        if settings_changed {
            ctx.request_repaint();
        }
        if close {
            if let Some(snapshot) = self.image_adjust_snapshot.take() {
                if snapshot != self.editor.document {
                    self.editor.history.record(&snapshot, "Adjust image");
                }
            }
            self.image_adjust = None;
        }
    }

    fn refresh_image_previews(&mut self, ctx: &egui::Context) {
        use crate::document::Shape;

        let mut needed: Vec<(String, crate::objects::ImageObject)> = Vec::new();
        for object in &self.editor.document.objects {
            if let Shape::Image(image) = &object.shape {
                needed.push((object.id.clone(), image.clone()));
            }
        }
        self.image_previews.retain(|id, preview| {
            needed.iter().any(|(needed_id, image)| {
                needed_id == id
                    && preview.path == image.path
                    && preview.settings == image.settings
                    && preview.crop == image.crop
            })
        });
        for (id, image) in needed {
            if self.image_previews.contains_key(&id) {
                continue;
            }
            let Ok(bitmap) =
                crate::raster::process_object(std::path::Path::new(&image.path), &image)
            else {
                continue;
            };
            let path = image.path.clone();
            let settings = image.settings;
            let crop = image.crop;
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
                    crop,
                },
            );
        }
    }

    fn selection_doc_centre(&self) -> Option<Point2> {
        self.editor
            .selected_bounds()
            .map(|(min, max)| Point2::new(f64::midpoint(min.x, max.x), f64::midpoint(min.y, max.y)))
    }

    fn selection_handle_at(&self, rect: Rect, pos: Pos2) -> Option<HandleKind> {
        if !self.editor.show_selection || self.editor.selection.is_empty() {
            return None;
        }
        let (min, max) = self.editor.selected_bounds()?;
        let p0 = crate::editor::renderer::screen_pos(rect, &self.editor, Point2::new(min.x, max.y));
        let p1 = crate::editor::renderer::screen_pos(rect, &self.editor, Point2::new(max.x, min.y));
        let bounds = Rect::from_two_pos(p0, p1).expand(2.0);
        let centre = bounds.center();
        if bounds.width() > 48.0 && bounds.height() > 48.0 {
            for corner in [
                bounds.left_top(),
                bounds.right_top(),
                bounds.right_bottom(),
                bounds.left_bottom(),
            ] {
                let direction = corner - centre;
                let length = direction.length();
                if length > 1.0 && (corner + direction / length * 18.0 - pos).length() <= 9.0 {
                    return Some(HandleKind::Rotate);
                }
            }
        }
        let handles = [
            bounds.left_top(),
            bounds.center_top(),
            bounds.right_top(),
            bounds.right_center(),
            bounds.right_bottom(),
            bounds.center_bottom(),
            bounds.left_bottom(),
            bounds.left_center(),
        ];
        for (index, handle) in handles.iter().enumerate() {
            if (*handle - pos).length() <= 8.0 {
                return Some(HandleKind::Scale(index));
            }
        }
        if (centre - pos).length() <= 9.0 {
            return Some(HandleKind::Move);
        }
        None
    }

    fn doc_point(&self, rect: Rect, pos: Pos2) -> Point2 {
        let (x, y) = self
            .editor
            .canvas
            .screen_to_doc(f64::from(pos.x - rect.min.x), f64::from(pos.y - rect.min.y));
        self.editor.snap(Point2::new(x, y))
    }

    fn nearest_node_handle(&self, point: Point2, tolerance: f64) -> Option<(usize, bool)> {
        let mut best: Option<(usize, bool, f64)> = None;
        for (index, (handle_in, handle_out)) in self.editor.node_handles().iter().enumerate() {
            for (outgoing, handle) in [(false, handle_in), (true, handle_out)] {
                let Some(handle) = handle else {
                    continue;
                };
                let distance = handle.distance(point);
                if distance <= tolerance && best.is_none_or(|(_, _, current)| distance < current) {
                    best = Some((index, outgoing, distance));
                }
            }
        }
        best.map(|(index, outgoing, _)| (index, outgoing))
    }

    fn begin_node_interaction(&mut self, point: Point2, shift: bool) {
        let tolerance = 6.0 / self.editor.canvas.zoom.max(0.01);
        if let Some((node, outgoing)) = self.nearest_node_handle(point, tolerance) {
            self.drag = Some(DragState::Handle {
                node,
                outgoing,
                recorded: false,
            });
            return;
        }
        if self
            .editor
            .select_node_at(point, tolerance, shift)
            .is_some()
        {
            self.drag = Some(DragState::NodeMove {
                last: point,
                recorded: false,
            });
        }
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
        self.smart_guides.clear();
        let Some(drag) = &mut self.drag else {
            return;
        };
        match drag {
            DragState::Move { last, recorded } => {
                let dx = point.x - last.x;
                let dy = point.y - last.y;
                let (dx, dy, guides) = alignment_snap(
                    &self.editor.document,
                    self.editor.selection.ids(),
                    self.editor.selected_bounds(),
                    self.editor.canvas.zoom,
                    dx,
                    dy,
                );
                self.smart_guides = guides;
                if dx.abs() > 1e-9 || dy.abs() > 1e-9 {
                    if !*recorded {
                        self.editor.record_action("Move");
                        *recorded = true;
                    }
                    self.editor.move_selection(dx, dy);
                    *last = point;
                }
            }
            DragState::NodeMove { last, recorded } => {
                let dx = point.x - last.x;
                let dy = point.y - last.y;
                if dx.abs() > 1e-9 || dy.abs() > 1e-9 {
                    if !*recorded {
                        self.editor.record_action("Move node");
                        *recorded = true;
                    }
                    self.editor.move_selected_nodes(dx, dy);
                    *last = point;
                }
            }
            DragState::Handle {
                node,
                outgoing,
                recorded,
            } => {
                if !*recorded {
                    self.editor.record_action("Edit handle");
                    *recorded = true;
                }
                self.editor.set_node_handle(*node, *outgoing, point);
            }
            DragState::Draw {
                end,
                start,
                tool,
                constrain,
            } => {
                *end = match (*constrain, tool) {
                    (true, Tool::Rect | Tool::Ellipse) => {
                        crate::geometry::point::square_corner(*start, point)
                    }
                    (true, Tool::Line) => {
                        crate::geometry::point::constrain_angle(*start, point, 45.0)
                    }
                    _ => point,
                };
            }
            DragState::Marquee { end, .. } => {
                *end = point;
            }
            DragState::Sketch { points } => {
                if points.last().is_none_or(|last| last.distance(point) >= 0.5) {
                    points.push(point);
                }
            }
            DragState::Transform {
                kind,
                last,
                centre,
                recorded,
                constrain,
            } => {
                let (dx, dy) = (point.x - last.x, point.y - last.y);
                let action = match kind {
                    HandleKind::Move => "Move",
                    HandleKind::Scale(_) => "Scale",
                    HandleKind::Rotate => "Rotate",
                };
                if !*recorded && (dx.abs() > 1e-9 || dy.abs() > 1e-9) {
                    self.editor.record_action(action);
                    *recorded = true;
                }
                match *kind {
                    HandleKind::Move => {
                        let (dx, dy, guides) = alignment_snap(
                            &self.editor.document,
                            self.editor.selection.ids(),
                            self.editor.selected_bounds(),
                            self.editor.canvas.zoom,
                            dx,
                            dy,
                        );
                        self.smart_guides = guides;
                        self.editor.move_selection(dx, dy);
                    }
                    HandleKind::Rotate => {
                        let before = (last.y - centre.y).atan2(last.x - centre.x);
                        let now = (point.y - centre.y).atan2(point.x - centre.x);
                        let mut delta = now - before;
                        while delta > std::f64::consts::PI {
                            delta -= std::f64::consts::TAU;
                        }
                        while delta < -std::f64::consts::PI {
                            delta += std::f64::consts::TAU;
                        }
                        if delta.abs() > 1e-9 {
                            self.editor.rotate_selection(delta);
                        }
                    }
                    HandleKind::Scale(index) => {
                        let uses_x = !matches!(index, 1 | 5);
                        let uses_y = !matches!(index, 3 | 7);
                        let last_x = last.x - centre.x;
                        let last_y = last.y - centre.y;
                        let mut factor_x = if uses_x && last_x.abs() > 1e-9 {
                            (point.x - centre.x) / last_x
                        } else {
                            1.0
                        };
                        let mut factor_y = if uses_y && last_y.abs() > 1e-9 {
                            (point.y - centre.y) / last_y
                        } else {
                            1.0
                        };
                        if *constrain {
                            let factor = if (factor_x - 1.0).abs() > (factor_y - 1.0).abs() {
                                factor_x
                            } else {
                                factor_y
                            };
                            factor_x = factor;
                            factor_y = factor;
                        }
                        if factor_x.is_finite()
                            && factor_y.is_finite()
                            && factor_x > 0.01
                            && factor_y > 0.01
                        {
                            self.editor.scale_selection(factor_x, factor_y);
                        }
                    }
                }
                *last = point;
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
            DragState::Draw {
                tool,
                start,
                mut end,
                ..
            } => {
                if self.exact_size && matches!(tool, Tool::Rect | Tool::Ellipse) {
                    end = crate::geometry::point::sized_corner(
                        start,
                        end,
                        self.exact_width,
                        self.exact_height,
                    );
                }
                match tool {
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
                            Point2::new(
                                f64::midpoint(start.x, end.x),
                                f64::midpoint(start.y, end.y),
                            ),
                            end,
                            self.polygon_sides,
                        );
                    }
                    Tool::Star => {
                        self.editor.add_star(
                            Point2::new(
                                f64::midpoint(start.x, end.x),
                                f64::midpoint(start.y, end.y),
                            ),
                            end,
                            self.star_points,
                            self.star_inner,
                        );
                    }
                    Tool::Text => {
                        if let Some(id) = self.editor.add_text(start) {
                            let font = self.preferences.general.text_font.clone();
                            if let Some(object) = self
                                .editor
                                .document
                                .objects
                                .iter_mut()
                                .find(|object| object.id == id)
                            {
                                if let Shape::Text(text) = &mut object.shape {
                                    text.font = font;
                                }
                            }
                        }
                    }
                    Tool::Measure => {
                        let distance = start.distance(end);
                        let angle = (end.y - start.y).atan2(end.x - start.x).to_degrees();
                        self.status = format!("Distance {distance:.2} mm at {angle:.1}°");
                    }
                    _ => {}
                }
            }
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
            DragState::Move { .. }
            | DragState::Pan { .. }
            | DragState::NodeMove { .. }
            | DragState::Handle { .. }
            | DragState::Transform { .. } => {}
        }
    }

    fn cut(&mut self) {
        let objects = self.editor.cut_selection();
        self.status = format!("Cut {} object(s)", objects.len());
        self.clipboard = objects;
    }

    fn copy(&mut self) {
        self.clipboard = self.editor.copy_selection();
        self.status = format!("Copied {} object(s)", self.clipboard.len());
    }

    fn paste(&mut self) {
        let count = self.editor.paste_objects(&self.clipboard);
        self.status = format!("Pasted {count} object(s)");
    }

    fn select_profile(&mut self, id: &str) {
        self.profiles_open = true;
        if self.profiles.material(id).is_some() {
            self.selected_material = Some(id.to_string());
        } else if self.profiles.operation(id).is_some() {
            self.selected_operation = Some(id.to_string());
        } else if self.profiles.image(id).is_some() {
            self.selected_image_profile = Some(id.to_string());
        } else if self.profiles.project(id).is_some() {
            self.selected_project_profile = Some(id.to_string());
        }
    }

    fn save_profiles(&mut self) {
        match crate::profiles::save(&self.profiles, &self.profiles_path) {
            Ok(()) => self.profile_message = "Profiles saved".to_string(),
            Err(error) => self.profile_message = format!("Cannot save profiles: {error}"),
        }
    }

    fn poll_task(&mut self, ctx: &egui::Context) {
        let Some(task) = &self.task else {
            return;
        };
        match task.poll() {
            crate::task::TaskPoll::Pending => {
                ctx.request_repaint_after(std::time::Duration::from_millis(100));
            }
            crate::task::TaskPoll::Cancelled => {
                self.task = None;
                self.status = "Cancelled".to_string();
            }
            crate::task::TaskPoll::Finished(TaskResult::Import { path, outcome }) => {
                self.task = None;
                match outcome {
                    Ok(outcome) => self.apply_import(&path, outcome),
                    Err(error) => self.status = format!("Import failed: {error}"),
                }
            }
            crate::task::TaskPoll::Finished(TaskResult::Export(result)) => {
                self.task = None;
                match result {
                    Ok(report) => {
                        self.status = report.headline();
                        self.show_report(report);
                    }
                    Err(error) if error == "Cancelled" => {
                        self.status = "Export cancelled".to_string();
                    }
                    Err(error) => self.status = format!("Export failed: {error}"),
                }
            }
        }
    }

    fn task_window(&mut self, ctx: &egui::Context) {
        let Some(task) = &self.task else {
            return;
        };
        let mut cancel = false;
        egui::Window::new("Working")
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(task.label());
                });
                ui.weak("You can stop waiting; the result will be discarded.");
                if ui.button("Cancel").clicked() {
                    cancel = true;
                }
            });
        if cancel {
            if let Some(task) = &self.task {
                task.cancel();
                self.status = format!("Cancelled: {}", task.label());
            }
            self.task = None;
        }
    }

    fn autosave(&mut self, ctx: &egui::Context) {
        if !self.is_dirty() {
            return;
        }
        let interval = std::time::Duration::from_secs(30);
        if self.last_autosave.elapsed() < interval {
            ctx.request_repaint_after(interval);
            return;
        }
        if crate::document::serialization::save_recovery(&self.editor.document, &self.recovery_path)
            .is_ok()
        {
            self.last_autosave = std::time::Instant::now();
        }
        ctx.request_repaint_after(interval);
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
                ui.horizontal(|ui| {
                    ui.checkbox(&mut self.preview_line_numbers, "Line numbers");
                    ui.checkbox(&mut self.preview_order, "Processing order on canvas");
                });
                ui.weak(format!("Estimated job time: {duration:.1} s"));
                ui.separator();
                ui.heading("G-code");
                let mut text = if self.preview_line_numbers {
                    crate::gcode::number_lines(&self.gcode_text)
                } else {
                    self.gcode_text.clone()
                };
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

    fn report_duplicates(&mut self) {
        let groups = self.editor.duplicate_groups();
        let mut report = FileReport::new("Duplicate geometry", "current document")
            .with_summary("Groups", groups.len().to_string());
        let mut total = 0;
        for (index, group) in groups.iter().enumerate() {
            total += group.len();
            let names: Vec<String> = group
                .iter()
                .filter_map(|id| self.editor.document.object(id))
                .map(|object| object.name.clone())
                .collect();
            report
                .warnings
                .push(format!("Group {}: {}", index + 1, names.join(", ")));
        }
        report
            .summary
            .push(("Objects".to_string(), total.to_string()));
        self.status = if groups.is_empty() {
            "No duplicate geometry".to_string()
        } else {
            format!("{} duplicate group(s)", groups.len())
        };
        self.show_report(report);
    }

    fn report_intersections(&mut self) {
        let hits = self.editor.intersections();
        let mut report = FileReport::new("Path intersections", "current document")
            .with_summary("Intersections", hits.len().to_string());
        let name = |id: &str| {
            self.editor
                .document
                .object(id)
                .map_or_else(|| id.to_string(), |object| object.name.clone())
        };
        for (index, hit) in hits.iter().enumerate() {
            if index >= 200 {
                report.warnings.push(format!(
                    "{} more intersection(s) not listed",
                    hits.len() - index
                ));
                break;
            }
            report.warnings.push(format!(
                "{} x {} at ({:.2}, {:.2}) mm",
                name(&hit.first),
                name(&hit.second),
                hit.point.x,
                hit.point.y
            ));
        }
        if hits.is_empty() {
            report
                .summary
                .push(("Pairs".to_string(), "none".to_string()));
            self.status = "No outline intersections".to_string();
        } else {
            self.status = format!("{} outline intersection(s)", hits.len());
        }
        self.show_report(report);
    }

    fn finish_pen(&mut self, closed: bool) {
        if self.pen_nodes.len() >= 2
            && self
                .editor
                .add_bezier_nodes(&self.pen_nodes, closed)
                .is_some()
        {
            self.status = if closed {
                "Closed Bézier path created".to_string()
            } else {
                "Bézier path created".to_string()
            };
        }
        self.pen_nodes.clear();
        self.pen_drag_start = None;
    }

    fn apply_text_on_path(&mut self) {
        let ids = self.editor.selection.ids().to_vec();
        let text = ids.iter().find(|id| {
            self.editor
                .document
                .object(id)
                .is_some_and(|object| matches!(object.shape, crate::document::Shape::Text(_)))
        });
        let Some(text) = text else {
            self.status = "Select a text object and a path".to_string();
            return;
        };
        let path = ids.iter().find(|id| *id != text);
        let Some(path) = path else {
            self.status = "Select a text object and a path".to_string();
            return;
        };
        match self.editor.fit_text_to_path(text, path) {
            Ok(outlines) => {
                self.status = format!("Fitted text to path ({} contour(s))", outlines.len());
            }
            Err(error) => self.status = format!("Fit failed: {error}"),
        }
    }

    fn apply_text_outlines(&mut self) {
        let Some(id) = self.editor.selection.ids().first().cloned() else {
            return;
        };
        let font = self.text_font_of(&id);
        let data = crate::textfont::read(&font);
        match self
            .editor
            .convert_text_to_outlines_with_font(&id, Some(&data))
        {
            Ok(outlines) => {
                self.status = format!("Converted text to {} outline(s)", outlines.len());
            }
            Err(error) => self.status = format!("Text conversion failed: {error}"),
        }
    }

    fn text_font_of(&self, id: &str) -> String {
        self.editor
            .document
            .objects
            .iter()
            .find(|object| object.id == id)
            .and_then(|object| match &object.shape {
                Shape::Text(text) => Some(text.font.clone()),
                _ => None,
            })
            .unwrap_or_else(|| crate::textfont::BUNDLED_NAME.to_string())
    }

    fn apply_trim(&mut self) {
        match self.editor.trim_selection() {
            Ok(count) => self.status = format!("Trimmed {count} object(s)"),
            Err(error) => self.status = format!("Trim failed: {error}"),
        }
    }

    fn apply_delete_overlaps(&mut self) {
        match self.editor.delete_overlapping_segments() {
            Ok(count) => self.status = format!("Removed overlaps in {count} object(s)"),
            Err(error) => self.status = format!("Overlap removal failed: {error}"),
        }
    }

    fn apply_outline_stroke(&mut self) {
        match self.editor.outline_selection() {
            Ok(count) => self.status = format!("Created {count} outline contour(s)"),
            Err(error) => self.status = format!("Outline failed: {error}"),
        }
    }

    fn apply_join_paths(&mut self) {
        match self.editor.join_paths() {
            Ok(_) => self.status = "Paths joined".to_string(),
            Err(error) => self.status = format!("Join failed: {error}"),
        }
    }

    fn apply_close_paths(&mut self) {
        match self.editor.close_paths() {
            Ok(count) => self.status = format!("Closed {count} path(s)"),
            Err(error) => self.status = format!("Close failed: {error}"),
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

    fn skew_window(&mut self, ctx: &egui::Context) {
        let mut open = self.skew_open;
        let mut apply = false;
        egui::Window::new("Skew")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Horizontal");
                    ui.add(
                        egui::DragValue::new(&mut self.skew_x)
                            .speed(1.0)
                            .range(-80.0..=80.0)
                            .suffix(" deg"),
                    );
                });
                ui.horizontal(|ui| {
                    ui.label("Vertical");
                    ui.add(
                        egui::DragValue::new(&mut self.skew_y)
                            .speed(1.0)
                            .range(-80.0..=80.0)
                            .suffix(" deg"),
                    );
                });
                ui.weak("The shear is applied around the selection centre.");
                if ui.button("Apply").clicked() {
                    apply = true;
                }
            });
        if apply {
            if self.editor.skew_selection(self.skew_x, self.skew_y) {
                self.status = "Selection skewed".to_string();
                self.skew_open = false;
            } else {
                self.status = "Skew failed: select something and use a non-zero angle".to_string();
            }
        } else {
            self.skew_open = open;
        }
    }

    fn repeat_window(&mut self, ctx: &egui::Context) {
        use crate::editor::RepeatMode;

        let mut open = self.repeat_open;
        let mut apply = false;
        egui::Window::new("Repeat selection")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                egui::ComboBox::from_label("Mode")
                    .selected_text(match self.repeat_options.mode {
                        RepeatMode::Linear => "Linear",
                        RepeatMode::Circular => "Circular",
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut self.repeat_options.mode,
                            RepeatMode::Linear,
                            "Linear",
                        );
                        ui.selectable_value(
                            &mut self.repeat_options.mode,
                            RepeatMode::Circular,
                            "Circular",
                        );
                    });
                ui.horizontal(|ui| {
                    ui.label("Copies");
                    ui.add(
                        egui::DragValue::new(&mut self.repeat_options.copies)
                            .speed(1.0)
                            .range(1..=100),
                    );
                });
                match self.repeat_options.mode {
                    RepeatMode::Linear => {
                        ui.horizontal(|ui| {
                            ui.label("Spacing X");
                            ui.add(
                                egui::DragValue::new(&mut self.repeat_options.dx)
                                    .speed(0.5)
                                    .suffix(" mm"),
                            );
                            ui.label("Y");
                            ui.add(
                                egui::DragValue::new(&mut self.repeat_options.dy)
                                    .speed(0.5)
                                    .suffix(" mm"),
                            );
                        });
                        ui.horizontal(|ui| {
                            ui.label("Rotation per copy");
                            ui.add(
                                egui::DragValue::new(&mut self.repeat_options.rotation_degrees)
                                    .speed(1.0)
                                    .suffix(" deg"),
                            );
                        });
                    }
                    RepeatMode::Circular => {
                        ui.horizontal(|ui| {
                            ui.label("Radius");
                            ui.add(
                                egui::DragValue::new(&mut self.repeat_options.radius)
                                    .speed(0.5)
                                    .range(0.0..=10000.0)
                                    .suffix(" mm"),
                            );
                        });
                        ui.checkbox(
                            &mut self.repeat_options.rotate_copies,
                            "Rotate copies along the orbit",
                        );
                    }
                }
                ui.weak("Copies are spaced evenly and stay separate from the original.");
                if ui.button("Apply").clicked() {
                    apply = true;
                }
            });
        if apply {
            match self.editor.repeat_selection(&self.repeat_options) {
                Ok(count) => {
                    self.status = format!("Created {count} copy/copies");
                    self.repeat_open = false;
                }
                Err(error) => self.status = format!("Repeat failed: {error}"),
            }
        } else {
            self.repeat_open = open;
        }
    }

    fn open_nest(&mut self) {
        self.nest_open = true;
        self.refresh_nest();
    }

    fn refresh_nest(&mut self) {
        match self
            .editor
            .nest_selection(self.nest_spacing, self.nest_margin, self.nest_rotation)
        {
            Ok(proposal) => {
                self.nest_message.clear();
                self.nest_proposal = Some(proposal);
            }
            Err(error) => {
                self.nest_message = error;
                self.nest_proposal = None;
            }
        }
    }

    fn nest_window(&mut self, ctx: &egui::Context) {
        let mut open = self.nest_open;
        let mut preview = false;
        let mut apply = false;
        egui::Window::new("Nest selection")
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(420.0)
            .show(ctx, |ui| {
                ui.label(format!(
                    "Work area: {:.0} x {:.0} mm",
                    self.editor.document.width_mm, self.editor.document.height_mm
                ));
                ui.horizontal(|ui| {
                    ui.label("Spacing");
                    if ui
                        .add(
                            egui::DragValue::new(&mut self.nest_spacing)
                                .speed(0.5)
                                .range(0.0..=100.0)
                                .suffix(" mm"),
                        )
                        .changed()
                    {
                        preview = true;
                    }
                    ui.label("Margin");
                    if ui
                        .add(
                            egui::DragValue::new(&mut self.nest_margin)
                                .speed(0.5)
                                .range(0.0..=100.0)
                                .suffix(" mm"),
                        )
                        .changed()
                    {
                        preview = true;
                    }
                    if ui
                        .checkbox(&mut self.nest_rotation, "Allow 90 deg rotation")
                        .changed()
                    {
                        preview = true;
                    }
                });
                ui.horizontal(|ui| {
                    if ui.button("Preview").clicked() {
                        preview = true;
                    }
                    let can_apply = self
                        .nest_proposal
                        .as_ref()
                        .is_some_and(|proposal| !proposal.placements.is_empty());
                    if ui
                        .add_enabled(can_apply, egui::Button::new("Apply"))
                        .clicked()
                    {
                        apply = true;
                    }
                    if ui.button("Cancel").clicked() {
                        self.nest_open = false;
                    }
                });
                if !self.nest_message.is_empty() {
                    ui.colored_label(Color32::from_rgb(190, 60, 50), &self.nest_message);
                }
                if let Some(proposal) = &self.nest_proposal {
                    ui.separator();
                    ui.label(format!(
                        "Placed {} of {} parts, {:.0} % of the available area",
                        proposal.placements.len(),
                        proposal.placements.len() + proposal.unplaced.len(),
                        proposal.utilization() * 100.0
                    ));
                    for placement in &proposal.placements {
                        let name = self
                            .editor
                            .document
                            .object(&placement.object_id)
                            .map_or("part", |object| object.name.as_str());
                        ui.weak(format!(
                            "{} -> ({:.1}, {:.1}) mm{}{}",
                            name,
                            placement.min.x,
                            placement.min.y,
                            if placement.rotated { ", rotated" } else { "" },
                            " - preview on the canvas"
                        ));
                    }
                    if !proposal.unplaced.is_empty() {
                        ui.colored_label(
                            Color32::from_rgb(180, 120, 0),
                            format!("{} part(s) do not fit", proposal.unplaced.len()),
                        );
                        for id in &proposal.unplaced {
                            if let Some(object) = self.editor.document.object(id) {
                                ui.colored_label(Color32::from_rgb(180, 120, 0), &object.name);
                            }
                        }
                    }
                }
            });
        if preview {
            self.refresh_nest();
        }
        if apply {
            if let Some(proposal) = self.nest_proposal.clone() {
                let moved = self.editor.apply_nest(&proposal);
                self.status = format!("Nested {moved} part(s)");
                self.nest_open = false;
                self.nest_proposal = None;
            }
        }
        if !self.nest_open {
            self.nest_proposal = None;
        }
        self.nest_open = self.nest_open && open;
    }

    fn simplify_window(&mut self, ctx: &egui::Context) {
        let mut open = self.simplify_open;
        let mut apply = false;
        egui::Window::new("Simplify path")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label("Tolerance");
                ui.add(
                    egui::DragValue::new(&mut self.simplify_tolerance)
                        .speed(0.05)
                        .range(0.001..=5.0)
                        .suffix(" mm"),
                );
                ui.weak("Curves are flattened and straightened within the tolerance.");
                if ui.button("Apply").clicked() {
                    apply = true;
                }
            });
        if apply {
            match self.editor.simplify_selected_path(self.simplify_tolerance) {
                Ok(nodes) => {
                    self.status = format!("Simplified to {nodes} node(s)");
                    self.simplify_open = false;
                }
                Err(error) => self.status = format!("Simplify failed: {error}"),
            }
        } else {
            self.simplify_open = open;
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
                let options = &mut self.grid_array;
                ui.horizontal(|ui| {
                    ui.label("Columns");
                    ui.add(
                        egui::DragValue::new(&mut options.columns)
                            .speed(1.0)
                            .range(1..=100),
                    );
                    ui.label("Rows");
                    ui.add(
                        egui::DragValue::new(&mut options.rows)
                            .speed(1.0)
                            .range(1..=100),
                    );
                });
                ui.horizontal(|ui| {
                    ui.label("Spacing X");
                    ui.add(
                        egui::DragValue::new(&mut options.dx)
                            .speed(0.5)
                            .range(-10000.0..=10000.0)
                            .suffix(" mm"),
                    );
                    ui.label("Y");
                    ui.add(
                        egui::DragValue::new(&mut options.dy)
                            .speed(0.5)
                            .range(-10000.0..=10000.0)
                            .suffix(" mm"),
                    );
                });
                ui.horizontal(|ui| {
                    ui.label("Margin X");
                    ui.add(
                        egui::DragValue::new(&mut options.margin_x)
                            .speed(0.5)
                            .range(-10000.0..=10000.0)
                            .suffix(" mm"),
                    );
                    ui.label("Y");
                    ui.add(
                        egui::DragValue::new(&mut options.margin_y)
                            .speed(0.5)
                            .range(-10000.0..=10000.0)
                            .suffix(" mm"),
                    );
                });
                ui.checkbox(&mut options.column_major, "Fill column by column");
                let options = *options;
                ui.label(format!(
                    "Creates {} copies of the {} selected object(s).",
                    options
                        .columns
                        .saturating_mul(options.rows)
                        .saturating_sub(1),
                    self.editor.selection.len()
                ));
                if ui
                    .add_enabled(
                        !self.editor.selection.is_empty(),
                        egui::Button::new("Create array"),
                    )
                    .clicked()
                {
                    let created = self.editor.duplicate_grid_with(&options);
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
                changed |= ui
                    .checkbox(&mut self.preferences.drawing.snap_nodes, "Snap to nodes")
                    .changed();
                changed |= ui
                    .checkbox(
                        &mut self.preferences.drawing.snap_endpoints,
                        "Snap to open-path endpoints",
                    )
                    .changed();
                changed |= ui
                    .checkbox(
                        &mut self.preferences.drawing.snap_centers,
                        "Snap to object centres",
                    )
                    .changed();
                changed |= ui
                    .checkbox(
                        &mut self.preferences.drawing.snap_midpoints,
                        "Snap to segment midpoints",
                    )
                    .changed();
                changed |= ui
                    .checkbox(
                        &mut self.preferences.drawing.snap_intersections,
                        "Snap to outline intersections",
                    )
                    .changed();
                changed |= ui
                    .checkbox(
                        &mut self.preferences.drawing.snap_limits,
                        "Snap to object bounds",
                    )
                    .changed();
                changed |= ui
                    .checkbox(
                        &mut self.preferences.drawing.show_selection,
                        "Show selection outlines and handles",
                    )
                    .changed();
                changed |= ui
                    .checkbox(&mut self.preferences.drawing.guides_visible, "Show guides")
                    .changed();
                changed |= ui
                    .checkbox(&mut self.preferences.drawing.snap_guides, "Snap to guides")
                    .changed();
                ui.separator();
                ui.heading("General");
                ui.horizontal(|ui| {
                    ui.label("Default unit");
                    let mut unit = self.preferences.general.default_unit.clone();
                    egui::ComboBox::from_id_salt("pref_default_unit")
                        .selected_text(unit.clone())
                        .show_ui(ui, |ui| {
                            for candidate in ["mm", "cm", "in", "px"] {
                                ui.selectable_value(&mut unit, candidate.to_string(), candidate);
                            }
                        });
                    if unit != self.preferences.general.default_unit {
                        self.preferences.general.default_unit = unit;
                        changed = true;
                    }
                });
                changed |= ui
                    .checkbox(
                        &mut self.preferences.general.confirm_destructive,
                        "Confirm before discarding unsaved changes",
                    )
                    .changed();
                changed |= ui
                    .checkbox(
                        &mut self.preferences.general.backup_enabled,
                        "Keep a .bak of the previous project version",
                    )
                    .changed();
                ui.horizontal(|ui| {
                    ui.label("Text font");
                    changed |= font_picker(
                        ui,
                        "preferences_text_font",
                        &mut self.preferences.general.text_font,
                    );
                });
                ui.horizontal(|ui| {
                    ui.label("Default directory");
                    changed |= ui
                        .text_edit_singleline(&mut self.preferences.general.default_directory)
                        .changed();
                });
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
                    self.editor.snap_nodes = self.preferences.drawing.snap_nodes;
                    self.editor.snap_endpoints = self.preferences.drawing.snap_endpoints;
                    self.editor.snap_centers = self.preferences.drawing.snap_centers;
                    self.editor.snap_midpoints = self.preferences.drawing.snap_midpoints;
                    self.editor.snap_intersections = self.preferences.drawing.snap_intersections;
                    self.editor.snap_limits = self.preferences.drawing.snap_limits;
                    self.editor.show_rulers = self.preferences.drawing.show_rulers;
                    self.editor.show_selection = self.preferences.drawing.show_selection;
                    self.editor
                        .selection_color
                        .clone_from(&self.preferences.drawing.selection_color);
                    self.editor.handle_style = crate::editor::state::HandleStyle::from_label(
                        &self.preferences.drawing.handle_style,
                    );
                    self.editor.zoom_step = self.preferences.drawing.zoom_step;
                    self.editor.select_whole_group = self.preferences.drawing.select_whole_group;
                    self.editor.guides_visible = self.preferences.drawing.guides_visible;
                    self.editor.snap_guides = self.preferences.drawing.snap_guides;
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
                    if ui.button("Import profiles...").clicked() {
                        self.open_dialog(FileMode::ImportProfiles);
                    }
                    if ui.button("Export profiles...").clicked() {
                        self.open_dialog(FileMode::ExportProfiles);
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Search");
                    ui.text_edit_singleline(&mut self.profile_search);
                });
                let query = self.profile_search.trim().to_lowercase();
                ui.horizontal(|ui| {
                    ui.checkbox(&mut self.profile_favorites_only, "Favourites only");
                    ui.label("Category");
                    let selected = if self.profile_category_filter.is_empty() {
                        "All".to_string()
                    } else {
                        self.profile_category_filter.clone()
                    };
                    egui::ComboBox::from_id_salt("profile_category_filter")
                        .selected_text(selected)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut self.profile_category_filter,
                                String::new(),
                                "All",
                            );
                            for category in self.profiles.categories() {
                                ui.selectable_value(
                                    &mut self.profile_category_filter,
                                    category.clone(),
                                    category,
                                );
                            }
                        });
                    ui.checkbox(&mut self.profile_sort_by_date, "Newest first");
                });
                let filter = ProfileFilter {
                    query,
                    favorites_only: self.profile_favorites_only,
                    category: self.profile_category_filter.clone(),
                    sort_by_date: self.profile_sort_by_date,
                };
                ui.separator();
                ui.heading("Materials");
                let materials = profile_rows(
                    &self.profiles.materials,
                    |profile| (&profile.id, &profile.name, profile.builtin, &profile.meta),
                    |profile| format!("{} {}", profile.material, profile.notes),
                    &filter,
                );
                for row in &materials {
                    ui.horizontal(|ui| {
                        let selected = self.selected_material.as_deref() == Some(row.id.as_str());
                        if ui.selectable_label(selected, &row.name).clicked() {
                            self.selected_material = Some(row.id.clone());
                        }
                        if row.favorite {
                            ui.weak("\u{2605}");
                        }
                        if !row.category.is_empty() {
                            ui.weak(row.category.as_str());
                        }
                        if row.builtin {
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
                let operation_choices: Vec<(String, String)> = self
                    .profiles
                    .operations
                    .iter()
                    .map(|operation| (operation.id.clone(), operation.name.clone()))
                    .collect();
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
                            ui.separator();
                            ui.label("Recommended operations");
                            for (operation_id, operation_name) in &operation_choices {
                                let mut checked = profile
                                    .recommended_operations
                                    .iter()
                                    .any(|recommended| recommended == operation_id);
                                if ui.checkbox(&mut checked, operation_name).changed() {
                                    if checked {
                                        profile
                                            .recommended_operations
                                            .push(operation_id.clone());
                                    } else {
                                        profile.recommended_operations.retain(|recommended| {
                                            recommended != operation_id
                                        });
                                    }
                                    changed = true;
                                }
                            }
                            ui.weak("Missing operation profiles are ignored.");
                        });
                        changed |= profile_meta_editor(
                            ui,
                            &id,
                            &mut profile.meta,
                            editable,
                            &mut self.profile_tag_buffer,
                            &mut self.profile_tag_owner,
                        );
                        if !editable {
                            ui.weak("Built-in profile: duplicate it to edit.");
                        }
                    }
                }
                ui.separator();
                ui.heading("Operation profiles");
                let operations = profile_rows(
                    &self.profiles.operations,
                    |profile| (&profile.id, &profile.name, profile.builtin, &profile.meta),
                    |profile| profile.notes.clone(),
                    &filter,
                );
                for row in &operations {
                    ui.horizontal(|ui| {
                        let selected = self.selected_operation.as_deref() == Some(row.id.as_str());
                        if ui.selectable_label(selected, &row.name).clicked() {
                            self.selected_operation = Some(row.id.clone());
                        }
                        if row.favorite {
                            ui.weak("\u{2605}");
                        }
                        if !row.category.is_empty() {
                            ui.weak(row.category.as_str());
                        }
                        if row.builtin {
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
                        changed |= profile_meta_editor(
                            ui,
                            &id,
                            &mut profile.meta,
                            editable,
                            &mut self.profile_tag_buffer,
                            &mut self.profile_tag_owner,
                        );
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
                        if ui
                            .add_enabled(
                                self.active_operation.is_some(),
                                egui::Button::new("Apply to active operation"),
                            )
                            .clicked()
                        {
                            if let Some(operation) = self.active_operation.clone() {
                                self.profile_pending =
                                    Some((operation, profile_clone.clone()));
                                self.profile_confirm_open = true;
                            }
                        }
                    }
                }
                ui.separator();
                ui.heading("Image profiles");
                let image_profiles = profile_rows(
                    &self.profiles.images,
                    |profile| (&profile.id, &profile.name, profile.builtin, &profile.meta),
                    |profile| profile.notes.clone(),
                    &filter,
                );
                for row in &image_profiles {
                    ui.horizontal(|ui| {
                        let selected =
                            self.selected_image_profile.as_deref() == Some(row.id.as_str());
                        if ui.selectable_label(selected, &row.name).clicked() {
                            self.selected_image_profile = Some(row.id.clone());
                        }
                        if row.favorite {
                            ui.weak("\u{2605}");
                        }
                        if !row.category.is_empty() {
                            ui.weak(row.category.as_str());
                        }
                        if row.builtin {
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
                        changed |= profile_meta_editor(
                            ui,
                            &id,
                            &mut profile.meta,
                            editable,
                            &mut self.profile_tag_buffer,
                            &mut self.profile_tag_owner,
                        );
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
                let project_profiles = profile_rows(
                    &self.profiles.projects,
                    |profile| (&profile.id, &profile.name, profile.builtin, &profile.meta),
                    |profile| profile.notes.clone(),
                    &filter,
                );
                for row in &project_profiles {
                    ui.horizontal(|ui| {
                        let selected =
                            self.selected_project_profile.as_deref() == Some(row.id.as_str());
                        if ui.selectable_label(selected, &row.name).clicked() {
                            self.selected_project_profile = Some(row.id.clone());
                        }
                        if row.favorite {
                            ui.weak("\u{2605}");
                        }
                        if !row.category.is_empty() {
                            ui.weak(row.category.as_str());
                        }
                        if row.builtin {
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
                        changed |= profile_meta_editor(
                            ui,
                            &id,
                            &mut profile.meta,
                            editable,
                            &mut self.profile_tag_buffer,
                            &mut self.profile_tag_owner,
                        );
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
        if self.machine_wizard_open {
            self.machine_wizard(ctx);
        }
        if self.machines_open {
            self.machines_window(ctx);
        }
        if self.cuts_open {
            self.cuts_window(ctx);
        }
        if self.help_shortcuts_open {
            self.shortcuts_window(ctx);
        }
        if self.image_adjust.is_some() {
            self.image_adjust_window(ctx);
        }
        if let Some(dialog) = &mut self.dialog {
            let result = dialog.show(ctx);
            match result {
                DialogResult::Pending => {}
                DialogResult::Cancelled => self.dialog = None,
                DialogResult::Accepted(path) => {
                    let mode = dialog.mode;
                    self.dialog = None;
                    let overwrites = path.exists()
                        && !matches!(
                            mode,
                            FileMode::OpenProject | FileMode::ImportFile | FileMode::RelinkImage
                        );
                    if overwrites {
                        self.pending_file = Some((mode, path));
                    } else {
                        self.handle_file(mode, &path);
                    }
                }
            }
        }
        if self.task.is_some() {
            self.task_window(ctx);
        }
        if self.recovery_prompt_open {
            let mut open_draft = false;
            let mut discard = false;
            egui::Window::new("Recover unsaved work")
                .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label(if self.recovery_timestamp.is_empty() {
                        "An autosaved draft was found.".to_string()
                    } else {
                        format!(
                            "An autosaved draft from {} was found.",
                            self.recovery_timestamp
                        )
                    });
                    ui.label("It was written because the application closed with unsaved changes.");
                    ui.horizontal(|ui| {
                        if ui.button("Open draft").clicked() {
                            open_draft = true;
                        }
                        if ui.button("Discard").clicked() {
                            discard = true;
                        }
                    });
                });
            if open_draft {
                match crate::document::serialization::load_from_file(&self.recovery_path) {
                    Ok(document) => {
                        self.editor.set_document(document);
                        self.current_path = None;
                        self.saved_snapshot = None;
                        self.needs_fit = true;
                        self.status = "Recovered the autosaved draft".to_string();
                        self.recovery_prompt_open = false;
                    }
                    Err(error) => {
                        self.status = format!("Cannot read the draft: {error}");
                    }
                }
            } else if discard {
                let _ = std::fs::remove_file(&self.recovery_path);
                self.recovery_prompt_open = false;
                self.status = "Discarded the autosaved draft".to_string();
            }
        }
        if let Some((mode, path)) = self.pending_file.clone() {
            let mut replace = false;
            let mut cancel = false;
            egui::Window::new("Replace file?")
                .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label(format!("{} already exists.", path.display()));
                    ui.label("Replacing it cannot be undone.");
                    ui.horizontal(|ui| {
                        if ui.button("Replace").clicked() {
                            replace = true;
                        }
                        if ui.button("Cancel").clicked() {
                            cancel = true;
                        }
                    });
                });
            if replace {
                self.pending_file = None;
                self.handle_file(mode, &path);
            } else if cancel {
                self.pending_file = None;
            }
        }
        if self.confirm.is_some() {
            let mut action = None;
            let mut cancel = false;
            egui::Window::new("Unsaved changes")
                .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
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
        if self.simplify_open {
            self.simplify_window(ctx);
        }
        if self.nest_open {
            self.nest_window(ctx);
        }
        if self.repeat_open {
            self.repeat_window(ctx);
        }
        if self.skew_open {
            self.skew_window(ctx);
        }
        if self.doc_properties_open {
            self.document_properties_window(ctx);
        }
        if self.operations_compare_open {
            self.operations_compare_window(ctx);
        }
        if self.profile_confirm_open {
            self.profile_confirm_window(ctx);
        }
        if self.guides_open {
            self.guides_window(ctx);
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
                .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
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
            && self.preferences.general.confirm_destructive
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.confirm = Some(PendingAction::Quit);
        }
        let document_fonts: Vec<String> = self
            .editor
            .document
            .objects
            .iter()
            .filter_map(|object| match &object.shape {
                Shape::Text(text) => Some(text.font.clone()),
                _ => None,
            })
            .collect();
        crate::textfont::ensure_loaded(&ctx, &document_fonts);
        ui.ctx().set_visuals(if self.dark_mode {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        });
        self.shortcuts(ui);
        self.menu_bar(ui);
        self.toolbar(ui);
        self.status_bar(ui);
        self.tool_strip(ui);
        if self.show_layers_panel {
            self.layers_panel(ui);
        }
        if self.show_properties_panel {
            self.properties_panel(ui);
        }
        self.canvas(ui);
        let ctx = ui.ctx().clone();
        self.dialogs(&ctx);
        if self.quit_requested {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let requests = self
            .bridge
            .as_ref()
            .map(crate::bridge::LiveBridge::drain)
            .unwrap_or_default();
        if !requests.is_empty() {
            for pending in requests {
                let response = self.handle_live_request(&pending.request);
                let _ = pending.reply.send(response);
            }
            ctx.request_repaint();
        }
        if !ctx.input(|input| input.pointer.any_down()) {
            self.multi_props_recorded = false;
        }
        if self.drag.is_none() {
            self.smart_guides.clear();
        }
        self.advance_playback(ctx);
        self.autosave(ctx);
        self.poll_task(ctx);
        let title = format!("BeamWright - {}", self.title());
        let current = ctx.input(|input| input.viewport().title.clone());
        if current.as_deref() != Some(title.as_str()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title));
        }
    }
}

/// Font picker shared by the text properties and the preferences window.
fn font_picker(ui: &mut Ui, id: &str, value: &mut String) -> bool {
    let mut changed = false;
    let choices = crate::textfont::names();
    egui::ComboBox::from_id_salt(id)
        .selected_text(value.clone())
        .width(240.0)
        .show_ui(ui, |ui| {
            egui::ScrollArea::vertical()
                .max_height(320.0)
                .show(ui, |ui| {
                    for name in &choices {
                        if ui
                            .selectable_value(value, name.clone(), name.as_str())
                            .changed()
                        {
                            changed = true;
                        }
                    }
                });
        });
    changed
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
    use crate::geometry::Point2;

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

    #[test]
    fn object_context_menu_matches_the_selection() {
        let none = object_context_actions(0, false, false);
        assert!(none.contains(&ContextAction::Cut));
        assert!(!none.contains(&ContextAction::Group));
        assert!(!none.contains(&ContextAction::EditNodes));
        assert!(!none.contains(&ContextAction::TextToOutlines));

        let pair = object_context_actions(2, false, false);
        assert!(pair.contains(&ContextAction::Group));
        assert!(!pair.contains(&ContextAction::EditNodes));

        let text = object_context_actions(1, true, false);
        assert!(text.contains(&ContextAction::EditNodes));
        assert!(text.contains(&ContextAction::TextToOutlines));
        assert!(!text.contains(&ContextAction::Group));

        let image = object_context_actions(1, false, true);
        assert!(image.contains(&ContextAction::AdjustImage));
        assert!(!image.contains(&ContextAction::TextToOutlines));
    }

    #[test]
    fn canvas_context_menu_offers_work_area_commands() {
        let actions = canvas_context_actions();
        assert!(actions.contains(&ContextAction::Paste));
        assert!(actions.contains(&ContextAction::SelectAll));
        assert!(actions.contains(&ContextAction::ZoomIn));
        assert!(actions.contains(&ContextAction::FitPage));
        assert!(actions.contains(&ContextAction::DocumentProperties));
        assert!(actions.contains(&ContextAction::Preferences));
        assert!(!actions.contains(&ContextAction::Cut));
        assert!(!actions.contains(&ContextAction::Delete));
    }

    #[test]
    fn undo_status_names_the_action() {
        let mut app = BeamWrightApp::new(None);
        app.editor
            .add_rectangle(Point2::new(0.0, 0.0), Point2::new(10.0, 10.0));
        app.editor.record_action("Move");
        app.editor.move_selection(2.0, 0.0);

        assert_eq!(app.undo_label(), "Undo Move");
        app.undo();
        assert_eq!(app.status, "Undo Move");
        assert_eq!(app.redo_label(), "Redo Move");
        app.redo();
        assert_eq!(app.status, "Redo Move");
    }
}
