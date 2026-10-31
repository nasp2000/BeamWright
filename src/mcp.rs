use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::document::{Document, Object, Shape};
use crate::editor::EditorState;
use crate::errors::{BeamError, Result};
use crate::geometry::{Point2, Transform2};
use crate::operations::OperationType;

/// MCP server state: one open document, mirroring the editor's commands.
///
/// The protocol is JSON-RPC 2.0 over stdio. Every tool calls the same library
/// code as the GUI, so what an agent tests here is what the application does.
#[derive(Debug)]
pub struct McpServer {
    editor: EditorState,
    path: Option<PathBuf>,
    clipboard: Vec<Object>,
}

impl Default for McpServer {
    fn default() -> Self {
        Self::new()
    }
}

impl McpServer {
    fn font_of(&self, id: &str) -> String {
        self.editor
            .document
            .objects
            .iter()
            .find(|object| object.id == id)
            .and_then(|object| match &object.shape {
                crate::document::Shape::Text(text) => Some(text.font.clone()),
                _ => None,
            })
            .unwrap_or_else(|| crate::textfont::BUNDLED_NAME.to_string())
    }

    /// Swap the editor used by the server, returning the previous one.
    ///
    /// Used by the live bridge to execute a request against the document that is
    /// open in the GUI, then put it back.
    pub fn replace_editor(&mut self, editor: EditorState) -> EditorState {
        std::mem::replace(&mut self.editor, editor)
    }

    pub fn new() -> Self {
        Self {
            editor: EditorState::new(Document::default()),
            path: None,
            clipboard: Vec::new(),
        }
    }

    /// Handle one JSON-RPC request and return the response object.
    pub fn handle(&mut self, request: &Value) -> Value {
        let id = request.get("id").cloned().unwrap_or(Value::Null);
        let method = request
            .get("method")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let params = request.get("params").cloned().unwrap_or(Value::Null);
        let result = match method {
            "initialize" => Ok(json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {"tools": {}},
                "serverInfo": {"name": "beamwright", "version": env!("CARGO_PKG_VERSION")}
            })),
            "tools/list" => Ok(json!({"tools": tool_definitions()})),
            "tools/call" => {
                let name = params.get("name").and_then(Value::as_str).unwrap_or("");
                let arguments = params.get("arguments").cloned().unwrap_or(json!({}));
                self.call_tool(name, &arguments)
            }
            "ping" => Ok(json!({})),
            other => Err(BeamError::InvalidInput(format!("unknown method: {other}"))),
        };
        match result {
            Ok(value) => json!({"jsonrpc": "2.0", "id": id, "result": value}),
            Err(error) => json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": {"code": -32000, "message": error.to_string()}
            }),
        }
    }

    #[allow(clippy::too_many_lines)]
    fn call_tool(&mut self, name: &str, arguments: &Value) -> Result<Value> {
        match name {
            "get_status" => Ok(self.status()),
            "new_project" => {
                let width = number(arguments, "width_mm").unwrap_or(300.0);
                let height = number(arguments, "height_mm").unwrap_or(200.0);
                let project = string(arguments, "name").unwrap_or_else(|| "Untitled".to_string());
                let document = Document::new(project, width, height);
                document.validate().map_err(BeamError::InvalidInput)?;
                self.editor.set_document(document);
                self.path = None;
                Ok(self.status())
            }
            "open_project" => {
                let path = required_path(arguments, "path")?;
                self.editor.load(&path)?;
                self.path = Some(path);
                Ok(self.status())
            }
            "save_project" => {
                let path = path_argument(arguments, "path").or_else(|| self.path.clone());
                let path = path.ok_or_else(|| {
                    BeamError::InvalidInput(
                        "no project path: pass 'path' or open a project".to_string(),
                    )
                })?;
                self.editor.save(&path)?;
                self.path = Some(path.clone());
                Ok(json!({"status": "saved", "path": path.display().to_string()}))
            }
            "import_file" => {
                let path = required_path(arguments, "path")?;
                let outcome = crate::import::format::import_with_report(&path)?;
                let mut document = outcome.document;
                if let Some(stem) = path.file_stem().and_then(|value| value.to_str()) {
                    document.name = stem.to_string();
                }
                self.editor.set_document(document);
                Ok(json!({
                    "objects": self.editor.document.objects.len(),
                    "warnings": outcome.warnings
                }))
            }
            "add_rectangle" => {
                let width = positive(arguments, "width")?;
                let height = positive(arguments, "height")?;
                let x = number(arguments, "x").unwrap_or(0.0);
                let y = number(arguments, "y").unwrap_or(0.0);
                let id = self.add_shape(
                    Shape::Rect {
                        width,
                        height,
                        corner_radius: number(arguments, "corner_radius").unwrap_or(0.0),
                    },
                    Transform2::translate(x, y),
                )?;
                Ok(json!({"id": id, "objects": self.editor.document.objects.len()}))
            }
            "add_circle" => {
                let radius = positive(arguments, "radius")?;
                let x = number(arguments, "x").unwrap_or(0.0);
                let y = number(arguments, "y").unwrap_or(0.0);
                let id = self.add_shape(
                    Shape::Ellipse {
                        rx: radius,
                        ry: radius,
                    },
                    Transform2::translate(x, y),
                )?;
                Ok(json!({"id": id, "objects": self.editor.document.objects.len()}))
            }
            "add_text" => {
                let x = number(arguments, "x").unwrap_or(0.0);
                let y = number(arguments, "y").unwrap_or(0.0);
                self.editor.selection.clear();
                let id = self
                    .editor
                    .add_text(Point2::new(x, y))
                    .ok_or_else(|| BeamError::InvalidInput("cannot add text".to_string()))?;
                if let Some(content) = string(arguments, "content") {
                    if let Some(object) = self.editor.document.object_mut(&id) {
                        if let Shape::Text(text) = &mut object.shape {
                            text.content = content;
                        }
                    }
                }
                if let Some(size) = number(arguments, "size") {
                    if let Some(object) = self.editor.document.object_mut(&id) {
                        if let Shape::Text(text) = &mut object.shape {
                            text.size = size.max(0.1);
                        }
                    }
                }
                Ok(json!({"id": id, "objects": self.editor.document.objects.len()}))
            }
            "add_line" => {
                let x1 = number(arguments, "x1").ok_or_else(|| missing("x1"))?;
                let y1 = number(arguments, "y1").ok_or_else(|| missing("y1"))?;
                let x2 = number(arguments, "x2").ok_or_else(|| missing("x2"))?;
                let y2 = number(arguments, "y2").ok_or_else(|| missing("y2"))?;
                let mut path = crate::geometry::Path::new();
                path.push(crate::geometry::path::Segment::Line {
                    from: Point2::new(x1, y1),
                    to: Point2::new(x2, y2),
                });
                let id = self.add_shape(Shape::Path(path), Transform2::identity())?;
                Ok(json!({"id": id, "objects": self.editor.document.objects.len()}))
            }
            "list_objects" => {
                let objects: Vec<Value> = self
                    .editor
                    .document
                    .objects
                    .iter()
                    .map(|object| {
                        let bounds = object.bounds().map(|(min, max)| {
                            json!({
                                "min": [min.x, min.y],
                                "max": [max.x, max.y]
                            })
                        });
                        json!({
                            "id": object.id,
                            "name": object.name,
                            "type": object.shape.kind_name(),
                            "layer": object.layer_id,
                            "operation": object.operation_id,
                            "bounds": bounds
                        })
                    })
                    .collect();
                Ok(json!({"objects": objects}))
            }
            "list_layers" => {
                let layers: Vec<Value> = self
                    .editor
                    .document
                    .layers
                    .iter()
                    .map(|layer| {
                        json!({
                            "id": layer.id,
                            "name": layer.name,
                            "color": layer.color,
                            "visible": layer.visible,
                            "locked": layer.locked,
                            "objects": self.editor.document.objects_on_layer(&layer.id).len()
                        })
                    })
                    .collect();
                Ok(json!({"layers": layers}))
            }
            "add_layer" => {
                let name = string(arguments, "name").ok_or_else(|| missing("name"))?;
                let id = self.editor.document.add_layer(name).id.clone();
                Ok(json!({"id": id, "layers": self.editor.document.layers.len()}))
            }
            "delete_objects" => {
                let ids = arguments
                    .get("ids")
                    .and_then(Value::as_array)
                    .ok_or_else(|| missing("ids"))?;
                let mut removed = 0;
                for id in ids.iter().filter_map(Value::as_str) {
                    if self.editor.document.remove_object(id).is_some() {
                        removed += 1;
                    }
                }
                Ok(json!({"removed": removed, "objects": self.editor.document.objects.len()}))
            }
            "create_operation" => {
                let name = string(arguments, "name").unwrap_or_else(|| "Operation".to_string());
                let op_type = parse_operation_type(
                    string(arguments, "type")
                        .as_deref()
                        .unwrap_or("outline_cutting"),
                )?;
                let id = self.editor.add_operation(name, op_type);
                if let Some(operation) = self
                    .editor
                    .document
                    .operations
                    .iter_mut()
                    .find(|operation| operation.id == id)
                {
                    if let Some(speed) = number(arguments, "speed") {
                        operation.params.speed = speed;
                    }
                    if let Some(power) = number(arguments, "power") {
                        operation.params.power = power;
                    }
                    if let Some(passes) = number(arguments, "passes") {
                        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                        let passes = passes.max(1.0) as u32;
                        operation.params.passes = passes;
                    }
                    if let Some(interval) = number(arguments, "line_interval_mm") {
                        operation.params.line_interval_mm = interval;
                    }
                    if let Some(angle) = number(arguments, "fill_angle_degrees") {
                        operation.params.fill_angle_degrees = angle;
                    }
                    if let Some(margin) = number(arguments, "fill_margin_mm") {
                        operation.params.fill_margin_mm = margin;
                    }
                    if let Some(offset) = number(arguments, "fill_offset_mm") {
                        operation.params.fill_offset_mm = offset;
                    }
                    if let Some(alternate) =
                        arguments.get("fill_alternate").and_then(Value::as_bool)
                    {
                        operation.params.fill_alternate = alternate;
                    }
                    operation.validate().map_err(BeamError::InvalidInput)?;
                }
                Ok(json!({"id": id, "operations": self.editor.document.operations.len()}))
            }
            "assign_operation_to_layer" => {
                let operation =
                    string(arguments, "operation").ok_or_else(|| missing("operation"))?;
                let layer = string(arguments, "layer")
                    .unwrap_or_else(|| self.editor.active_layer_id.clone());
                let assigned = self
                    .editor
                    .assign_operation_to_layer(&layer, Some(&operation));
                Ok(json!({"assigned": assigned, "layer": layer}))
            }
            "set_operation_dependency" => {
                let operation =
                    string(arguments, "operation").ok_or_else(|| missing("operation"))?;
                let depends_on = arguments.get("depends_on").and_then(Value::as_str);
                if !self.editor.set_operation_dependency(&operation, depends_on) {
                    return Err(BeamError::InvalidInput(
                        "cannot set that operation dependency".to_string(),
                    ));
                }
                Ok(json!({"operation": operation}))
            }
            "duplicate_operation" => {
                let id = string(arguments, "id").ok_or_else(|| missing("id"))?;
                let copy = self
                    .editor
                    .duplicate_operation(&id)
                    .ok_or_else(|| BeamError::InvalidInput(format!("unknown operation: {id}")))?;
                Ok(json!({
                    "id": copy,
                    "operations": self.editor.document.operations.len()
                }))
            }
            "assign_operation" => {
                let operation =
                    string(arguments, "operation").ok_or_else(|| missing("operation"))?;
                let ids: Vec<String> = arguments.get("ids").and_then(Value::as_array).map_or_else(
                    || {
                        self.editor
                            .document
                            .objects
                            .iter()
                            .map(|object| object.id.clone())
                            .collect()
                    },
                    |values| {
                        values
                            .iter()
                            .filter_map(Value::as_str)
                            .map(str::to_string)
                            .collect()
                    },
                );
                let mut assigned = 0;
                for id in ids {
                    if let Some(object) = self.editor.document.object_mut(&id) {
                        object.operation_id = Some(operation.clone());
                        assigned += 1;
                    }
                }
                Ok(json!({"assigned": assigned}))
            }
            "boolean" => {
                let operation =
                    string(arguments, "operation").ok_or_else(|| missing("operation"))?;
                let op = parse_boolean_op(&operation)?;
                let ids = arguments
                    .get("ids")
                    .and_then(Value::as_array)
                    .ok_or_else(|| missing("ids"))?;
                self.select_ids(ids)?;
                let contours = self
                    .editor
                    .boolean_selection(op)
                    .map_err(BeamError::InvalidInput)?;
                Ok(json!({
                    "contours": contours,
                    "objects": self.editor.document.objects.len()
                }))
            }
            "offset" => {
                let distance = number(arguments, "distance").ok_or_else(|| missing("distance"))?;
                self.select_scope(arguments)?;
                let count = self
                    .editor
                    .offset_selection(distance)
                    .map_err(BeamError::InvalidInput)?;
                Ok(json!({
                    "offset": count,
                    "objects": self.editor.document.objects.len()
                }))
            }
            "resize_selection" => {
                self.select_scope(arguments)?;
                let width = number(arguments, "width").ok_or_else(|| missing("width"))?;
                let height = number(arguments, "height").ok_or_else(|| missing("height"))?;
                let preserve = arguments
                    .get("preserve_proportions")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                if !self.editor.set_selection_size(width, height, preserve) {
                    return Err(BeamError::InvalidInput(
                        "cannot resize the selection".to_string(),
                    ));
                }
                Ok(self.status())
            }
            "skew_selection" => {
                self.select_scope(arguments)?;
                let x = number(arguments, "x_degrees").unwrap_or(0.0);
                let y = number(arguments, "y_degrees").unwrap_or(0.0);
                if !self.editor.skew_selection(x, y) {
                    return Err(BeamError::InvalidInput(
                        "cannot skew the selection".to_string(),
                    ));
                }
                Ok(self.status())
            }
            "reset_transforms" => {
                self.select_scope(arguments)?;
                let count = self.editor.reset_transforms();
                Ok(json!({"reset": count}))
            }
            "bring_to_front" => {
                self.select_scope(arguments)?;
                let changed = self.editor.bring_selection_to_front();
                Ok(json!({"moved": changed}))
            }
            "send_to_back" => {
                self.select_scope(arguments)?;
                let changed = self.editor.send_selection_to_back();
                Ok(json!({"moved": changed}))
            }
            "center_on_page" => {
                self.select_scope(arguments)?;
                let centered = self.editor.center_selection_on_page();
                Ok(json!({"centered": centered}))
            }
            "distribute_on_page" => {
                self.select_scope(arguments)?;
                let horizontal = string(arguments, "axis").as_deref() != Some("vertical");
                let distributed = self.editor.distribute_selection_on_page(horizontal);
                Ok(json!({"distributed": distributed}))
            }
            "copy_objects" => {
                self.select_scope(arguments)?;
                self.clipboard = self.editor.copy_selection();
                Ok(json!({"copied": self.clipboard.len()}))
            }
            "cut_objects" => {
                self.select_scope(arguments)?;
                self.clipboard = self.editor.cut_selection();
                Ok(json!({
                    "cut": self.clipboard.len(),
                    "objects": self.editor.document.objects.len()
                }))
            }
            "paste_objects" => {
                let pasted = self.editor.paste_objects(&self.clipboard);
                Ok(json!({
                    "pasted": pasted,
                    "objects": self.editor.document.objects.len()
                }))
            }
            "invert_selection" => {
                let count = self.editor.invert_selection();
                Ok(json!({"selected": count}))
            }
            "lock_objects" => {
                self.select_scope(arguments)?;
                let count = self.editor.lock_selection();
                Ok(json!({"locked": count}))
            }
            "unlock_objects" => {
                self.select_scope(arguments)?;
                let count = self.editor.unlock_selection();
                Ok(json!({"unlocked": count}))
            }
            "duplicate_grid" => {
                self.select_scope(arguments)?;
                let options = crate::editor::GridOptions {
                    columns: usize::try_from(
                        arguments
                            .get("columns")
                            .and_then(Value::as_u64)
                            .unwrap_or(2),
                    )
                    .unwrap_or(2),
                    rows: usize::try_from(
                        arguments.get("rows").and_then(Value::as_u64).unwrap_or(2),
                    )
                    .unwrap_or(2),
                    dx: number(arguments, "dx").unwrap_or(10.0),
                    dy: number(arguments, "dy").unwrap_or(10.0),
                    margin_x: number(arguments, "margin_x").unwrap_or(0.0),
                    margin_y: number(arguments, "margin_y").unwrap_or(0.0),
                    column_major: arguments
                        .get("column_major")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                };
                let created = self.editor.duplicate_grid_with(&options);
                Ok(json!({
                    "created": created,
                    "objects": self.editor.document.objects.len()
                }))
            }
            "repeat" => {
                let mode = match string(arguments, "mode").as_deref().unwrap_or("linear") {
                    "linear" => crate::editor::RepeatMode::Linear,
                    "circular" => crate::editor::RepeatMode::Circular,
                    other => {
                        return Err(BeamError::InvalidInput(format!(
                            "unknown repeat mode: {other}"
                        )));
                    }
                };
                self.select_scope(arguments)?;
                let copies = number(arguments, "copies").unwrap_or(3.0).max(1.0);
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let copies = copies.min(100.0) as usize;
                let options = crate::editor::RepeatOptions {
                    mode,
                    copies,
                    dx: number(arguments, "dx").unwrap_or(10.0),
                    dy: number(arguments, "dy").unwrap_or(0.0),
                    radius: number(arguments, "radius").unwrap_or(20.0),
                    rotation_degrees: number(arguments, "rotation_degrees").unwrap_or(0.0),
                    rotate_copies: arguments
                        .get("rotate_copies")
                        .and_then(Value::as_bool)
                        .unwrap_or(true),
                };
                let created = self
                    .editor
                    .repeat_selection(&options)
                    .map_err(BeamError::InvalidInput)?;
                Ok(json!({
                    "created": created,
                    "objects": self.editor.document.objects.len()
                }))
            }
            "nest" => {
                let spacing = number(arguments, "spacing").unwrap_or(1.0);
                let margin = number(arguments, "margin").unwrap_or(0.0);
                let rotation = arguments
                    .get("rotation")
                    .and_then(Value::as_bool)
                    .unwrap_or(true);
                let apply = arguments
                    .get("apply")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                if let Some(ids) = arguments.get("ids").and_then(Value::as_array) {
                    self.select_ids(ids)?;
                } else {
                    self.editor.selection.clear();
                    for object in &self.editor.document.objects {
                        if object.bounds().is_some() {
                            self.editor.selection.add(object.id.clone());
                        }
                    }
                }
                let proposal = self
                    .editor
                    .nest_selection(spacing, margin, rotation)
                    .map_err(BeamError::InvalidInput)?;
                let placements: Vec<Value> = proposal
                    .placements
                    .iter()
                    .map(|placement| {
                        json!({
                            "id": placement.object_id,
                            "min": [placement.min.x, placement.min.y],
                            "width": placement.width,
                            "height": placement.height,
                            "rotated": placement.rotated
                        })
                    })
                    .collect();
                let moved = if apply {
                    self.editor.apply_nest(&proposal)
                } else {
                    0
                };
                Ok(json!({
                    "placements": placements,
                    "unplaced": proposal.unplaced,
                    "utilization": proposal.utilization(),
                    "moved": moved,
                    "objects": self.editor.document.objects.len()
                }))
            }
            "begin_node_edit" => {
                let id = string(arguments, "id").ok_or_else(|| missing("id"))?;
                self.editor
                    .begin_node_edit(&id)
                    .map_err(BeamError::InvalidInput)?;
                Ok(self.node_state())
            }
            "end_node_edit" => {
                self.editor.end_node_edit();
                Ok(json!({"editing": false}))
            }
            "list_nodes" => Ok(self.node_state()),
            "move_node" => {
                let index = index_argument(arguments, "index")?;
                let x = number(arguments, "x").ok_or_else(|| missing("x"))?;
                let y = number(arguments, "y").ok_or_else(|| missing("y"))?;
                self.editor.node_selection.clear();
                self.editor.node_selection.push(index);
                if !self.editor.set_selected_node_position(Point2::new(x, y)) {
                    return Err(BeamError::InvalidInput(format!("cannot move node {index}")));
                }
                Ok(self.node_state())
            }
            "insert_node" => {
                let segment = index_argument(arguments, "segment")?;
                let t = number(arguments, "t").unwrap_or(0.5);
                let index = self.editor.insert_node_at(segment, t).ok_or_else(|| {
                    BeamError::InvalidInput(format!("cannot insert into segment {segment}"))
                })?;
                Ok(json!({
                    "node": index,
                    "nodes": self.editor.node_points().len()
                }))
            }
            "delete_node" => {
                let index = index_argument(arguments, "index")?;
                self.editor.node_selection.clear();
                self.editor.node_selection.push(index);
                let removed = self.editor.delete_selected_nodes();
                Ok(json!({
                    "removed": removed,
                    "nodes": self.editor.node_points().len(),
                    "editing": self.editor.is_editing_nodes()
                }))
            }
            "convert_node" => {
                let index = index_argument(arguments, "index")?;
                let smooth = arguments
                    .get("smooth")
                    .and_then(Value::as_bool)
                    .unwrap_or(true);
                self.editor.node_selection.clear();
                self.editor.node_selection.push(index);
                let converted = self.editor.convert_selected_nodes(smooth);
                Ok(json!({
                    "converted": converted,
                    "nodes": self.editor.node_points().len()
                }))
            }
            "simplify_path" => {
                let tolerance =
                    number(arguments, "tolerance").ok_or_else(|| missing("tolerance"))?;
                let nodes = self
                    .editor
                    .simplify_selected_path(tolerance)
                    .map_err(BeamError::InvalidInput)?;
                Ok(json!({"nodes": nodes}))
            }
            "align_handles" => {
                let index = index_argument(arguments, "index")?;
                self.editor.node_selection.clear();
                self.editor.node_selection.push(index);
                let aligned = self.editor.align_selected_node_handles();
                Ok(json!({"aligned": aligned}))
            }
            "fit_text_to_path" => {
                let text = string(arguments, "text").ok_or_else(|| missing("text"))?;
                let path = string(arguments, "path").ok_or_else(|| missing("path"))?;
                let font = self.font_of(&text);
                let data = crate::textfont::read(&font);
                let outlines = self
                    .editor
                    .fit_text_to_path_with_font(&text, &path, Some(&data))
                    .map_err(BeamError::InvalidInput)?;
                Ok(json!({
                    "outlines": outlines.len(),
                    "objects": self.editor.document.objects.len()
                }))
            }
            "text_to_outlines" => {
                let id = string(arguments, "id").ok_or_else(|| missing("id"))?;
                let font = self.font_of(&id);
                let data = crate::textfont::read(&font);
                let outlines = self
                    .editor
                    .convert_text_to_outlines_with_font(&id, Some(&data))
                    .map_err(BeamError::InvalidInput)?;
                Ok(json!({
                    "outlines": outlines.len(),
                    "objects": self.editor.document.objects.len()
                }))
            }
            "outline_stroke" => {
                self.select_scope(arguments)?;
                let count = self
                    .editor
                    .outline_selection()
                    .map_err(BeamError::InvalidInput)?;
                Ok(json!({
                    "contours": count,
                    "objects": self.editor.document.objects.len()
                }))
            }
            "trim" => {
                self.select_scope(arguments)?;
                let count = self
                    .editor
                    .trim_selection()
                    .map_err(BeamError::InvalidInput)?;
                Ok(json!({
                    "trimmed": count,
                    "objects": self.editor.document.objects.len()
                }))
            }
            "delete_overlapping_segments" => {
                self.select_scope(arguments)?;
                let count = self
                    .editor
                    .delete_overlapping_segments()
                    .map_err(BeamError::InvalidInput)?;
                Ok(json!({
                    "changed": count,
                    "objects": self.editor.document.objects.len()
                }))
            }
            "join_paths" => {
                self.select_scope(arguments)?;
                let id = self.editor.join_paths().map_err(BeamError::InvalidInput)?;
                Ok(json!({"id": id, "objects": self.editor.document.objects.len()}))
            }
            "close_paths" => {
                self.select_scope(arguments)?;
                let count = self.editor.close_paths().map_err(BeamError::InvalidInput)?;
                Ok(json!({"closed": count}))
            }
            "find_duplicates" => {
                let groups = self.editor.duplicate_groups();
                Ok(json!({"groups": groups, "count": groups.len()}))
            }
            "remove_duplicates" => {
                let removed = self.editor.remove_duplicates();
                Ok(json!({
                    "removed": removed,
                    "objects": self.editor.document.objects.len()
                }))
            }
            "find_intersections" => {
                let hits: Vec<Value> = self
                    .editor
                    .intersections()
                    .iter()
                    .map(|hit| {
                        json!({
                            "first": hit.first,
                            "second": hit.second,
                            "point": [hit.point.x, hit.point.y]
                        })
                    })
                    .collect();
                Ok(json!({"intersections": hits, "count": hits.len()}))
            }
            "export_svg" => {
                let path = required_path(arguments, "path")?;
                let text = crate::export::export_svg(&self.editor.document)?;
                std::fs::write(&path, text)?;
                Ok(json!({"path": path.display().to_string()}))
            }
            "export_dxf" => {
                let path = required_path(arguments, "path")?;
                let text = crate::export::export_dxf(&self.editor.document)?;
                std::fs::write(&path, text)?;
                Ok(json!({"path": path.display().to_string()}))
            }
            "export_gcode" => {
                let path = required_path(arguments, "path")?;
                let report = crate::gcode::export_job(
                    &self.editor.document,
                    &path,
                    &crate::gcode::JobOptions::default(),
                )?;
                Ok(json!({
                    "path": path.display().to_string(),
                    "lines": report.lines,
                    "bytes": report.bytes,
                    "paths": report.paths,
                    "seconds": report.seconds,
                    "warnings": report.warnings
                }))
            }
            "estimate" => {
                let (_, report) = crate::gcode::render_job(
                    &self.editor.document,
                    &crate::gcode::JobOptions::default(),
                )?;
                Ok(json!({
                    "paths": report.paths,
                    "lines": report.lines,
                    "bytes": report.bytes,
                    "seconds": report.seconds,
                    "warnings": report.warnings
                }))
            }
            other => Err(BeamError::InvalidInput(format!("unknown tool: {other}"))),
        }
    }

    /// Select the objects named by `ids`, or every object when it is absent.
    /// Node editing state as JSON for the tools above.
    fn node_state(&self) -> Value {
        let points = self.editor.node_points();
        let handles = self.editor.node_handles();
        let nodes: Vec<Value> = points
            .iter()
            .enumerate()
            .map(|(index, point)| {
                let (handle_in, handle_out) = handles.get(index).copied().unwrap_or((None, None));
                json!({
                    "index": index,
                    "point": [point.x, point.y],
                    "handle_in": handle_in.map(|handle| [handle.x, handle.y]),
                    "handle_out": handle_out.map(|handle| [handle.x, handle.y]),
                    "selected": self.editor.node_is_selected(index)
                })
            })
            .collect();
        json!({
            "editing": self.editor.is_editing_nodes(),
            "object": self.editor.node_object,
            "nodes": nodes
        })
    }

    fn select_scope(&mut self, arguments: &Value) -> Result<()> {
        if let Some(ids) = arguments.get("ids").and_then(Value::as_array) {
            return self.select_ids(ids);
        }
        self.editor.selection.clear();
        for object in &self.editor.document.objects {
            self.editor.selection.add(object.id.clone());
        }
        Ok(())
    }

    fn select_ids(&mut self, ids: &[Value]) -> Result<()> {
        let mut resolved = Vec::with_capacity(ids.len());
        for id in ids {
            let id = id
                .as_str()
                .ok_or_else(|| BeamError::InvalidInput("ids must be strings".to_string()))?;
            if self.editor.document.object(id).is_none() {
                return Err(BeamError::InvalidInput(format!("unknown object id: {id}")));
            }
            resolved.push(id.to_string());
        }
        self.editor.selection.clear();
        for id in resolved {
            self.editor.selection.add(id);
        }
        Ok(())
    }

    fn add_shape(&mut self, shape: Shape, transform: Transform2) -> Result<String> {
        let layer_id = self
            .editor
            .document
            .layers
            .iter()
            .find(|layer| !layer.locked)
            .map(|layer| layer.id.clone())
            .ok_or_else(|| BeamError::InvalidInput("no unlocked layer".to_string()))?;
        self.editor
            .record_action(&format!("Draw {}", shape.kind_name()));
        let object = Object::new(layer_id, shape).with_transform(transform);
        let id = object.id.clone();
        self.editor.document.add_object(object);
        Ok(id)
    }

    fn status(&self) -> Value {
        let document = &self.editor.document;
        json!({
            "name": document.name,
            "width_mm": document.width_mm,
            "height_mm": document.height_mm,
            "unit": document.unit.label(),
            "layers": document.layers.len(),
            "objects": document.objects.len(),
            "operations": document.operations.len(),
            "path": self.path.as_ref().map(|path| path.display().to_string()),
            "dirty": self.editor.history.can_undo()
        })
    }

    /// Serve JSON-RPC over stdin/stdout until end of input.
    pub fn serve_stdio(&mut self) -> Result<()> {
        let stdin = std::io::stdin();
        let mut stdout = std::io::stdout();
        for line in stdin.lock().lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let response = match serde_json::from_str::<Value>(&line) {
                Ok(request) => self.handle(&request),
                Err(error) => json!({
                    "jsonrpc": "2.0",
                    "id": Value::Null,
                    "error": {"code": -32700, "message": format!("parse error: {error}")}
                }),
            };
            writeln!(stdout, "{response}")?;
            stdout.flush()?;
        }
        Ok(())
    }
}

#[allow(clippy::too_many_lines)]
fn tool_definitions() -> Vec<Value> {
    vec![
        tool(
            "get_status",
            "Document summary: size, layers, objects, operations and dirty state.",
        ),
        tool_with(
            "new_project",
            "Create a new empty project.",
            json!({"type": "object", "properties": {
                "name": {"type": "string"},
                "width_mm": {"type": "number"},
                "height_mm": {"type": "number"}
            }}),
        ),
        tool_with(
            "open_project",
            "Open a .bwp project.",
            json!({"type": "object", "properties": {"path": {"type": "string"}}, "required": ["path"]}),
        ),
        tool_with(
            "save_project",
            "Save the open project (optionally to a new path).",
            json!({"type": "object", "properties": {"path": {"type": "string"}}}),
        ),
        tool_with(
            "import_file",
            "Import an SVG or DXF file.",
            json!({"type": "object", "properties": {"path": {"type": "string"}}, "required": ["path"]}),
        ),
        tool_with(
            "add_rectangle",
            "Add a rectangle in millimetres.",
            json!({"type": "object", "properties": {
                "x": {"type": "number"}, "y": {"type": "number"},
                "width": {"type": "number"}, "height": {"type": "number"},
                "corner_radius": {"type": "number"}
            }, "required": ["width", "height"]}),
        ),
        tool_with(
            "add_circle",
            "Add a circle in millimetres.",
            json!({"type": "object", "properties": {
                "x": {"type": "number"}, "y": {"type": "number"}, "radius": {"type": "number"}
            }, "required": ["radius"]}),
        ),
        tool_with(
            "add_text",
            "Add a text object at a point, optionally with content and size.",
            json!({"type": "object", "properties": {
                "x": {"type": "number"},
                "y": {"type": "number"},
                "content": {"type": "string"},
                "size": {"type": "number"}
            }}),
        ),
        tool_with(
            "add_line",
            "Add a line in millimetres.",
            json!({"type": "object", "properties": {
                "x1": {"type": "number"}, "y1": {"type": "number"},
                "x2": {"type": "number"}, "y2": {"type": "number"}
            }, "required": ["x1", "y1", "x2", "y2"]}),
        ),
        tool(
            "list_objects",
            "List objects with type, layer, operation and bounds.",
        ),
        tool(
            "list_layers",
            "List layers with visibility, lock and object counts.",
        ),
        tool_with(
            "add_layer",
            "Add a layer.",
            json!({"type": "object", "properties": {"name": {"type": "string"}}, "required": ["name"]}),
        ),
        tool_with(
            "delete_objects",
            "Delete objects by id.",
            json!({"type": "object", "properties": {"ids": {"type": "array", "items": {"type": "string"}}}, "required": ["ids"]}),
        ),
        tool_with(
            "create_operation",
            "Create an operation and optionally set its parameters.",
            json!({"type": "object", "properties": {
                "name": {"type": "string"},
                "type": {"type": "string"},
                "speed": {"type": "number"},
                "power": {"type": "number"},
                "passes": {"type": "integer"},
                "line_interval_mm": {"type": "number"},
                "fill_angle_degrees": {"type": "number"},
                "fill_margin_mm": {"type": "number"},
                "fill_offset_mm": {"type": "number"},
                "fill_alternate": {"type": "boolean"}
            }}),
        ),
        tool_with(
            "assign_operation_to_layer",
            "Assign an operation to every object of a layer (active layer by default).",
            json!({"type": "object", "properties": {
                "operation": {"type": "string"},
                "layer": {"type": "string"}
            }, "required": ["operation"]}),
        ),
        tool_with(
            "set_operation_dependency",
            "Make an operation run after another one (omit depends_on to clear).",
            json!({"type": "object", "properties": {
                "operation": {"type": "string"},
                "depends_on": {"type": "string"}
            }, "required": ["operation"]}),
        ),
        tool_with(
            "duplicate_operation",
            "Duplicate an operation, keeping its parameters.",
            json!({"type": "object", "properties": {"id": {"type": "string"}}, "required": ["id"]}),
        ),
        tool_with(
            "assign_operation",
            "Assign an operation to objects (all objects when ids are omitted).",
            json!({"type": "object", "properties": {
                "operation": {"type": "string"},
                "ids": {"type": "array", "items": {"type": "string"}}
            }, "required": ["operation"]}),
        ),
        tool_with(
            "boolean",
            "Combine exactly two closed shapes (union, difference, intersection, xor).",
            json!({"type": "object", "properties": {
                "operation": {"type": "string"},
                "ids": {"type": "array", "items": {"type": "string"}}
            }, "required": ["operation", "ids"]}),
        ),
        tool_with(
            "offset",
            "Offset closed shapes in millimetres; positive expands, negative contracts.",
            json!({"type": "object", "properties": {
                "distance": {"type": "number"},
                "ids": {"type": "array", "items": {"type": "string"}}
            }, "required": ["distance"]}),
        ),
        tool_with(
            "resize_selection",
            "Scale the selection (all objects when ids are omitted) to an exact size.",
            json!({"type": "object", "properties": {
                "ids": {"type": "array", "items": {"type": "string"}},
                "width": {"type": "number"},
                "height": {"type": "number"},
                "preserve_proportions": {"type": "boolean"}
            }, "required": ["width", "height"]}),
        ),
        tool_with(
            "skew_selection",
            "Shear the selection (all objects when ids are omitted) around its centre.",
            json!({"type": "object", "properties": {
                "ids": {"type": "array", "items": {"type": "string"}},
                "x_degrees": {"type": "number"},
                "y_degrees": {"type": "number"}
            }}),
        ),
        tool(
            "reset_transforms",
            "Bake the selected objects' transforms into their geometry.",
        ),
        tool_with(
            "bring_to_front",
            "Move the selected objects to the top of the drawing order.",
            json!({"type": "object", "properties": {
                "ids": {"type": "array", "items": {"type": "string"}}
            }}),
        ),
        tool_with(
            "send_to_back",
            "Move the selected objects to the bottom of the drawing order.",
            json!({"type": "object", "properties": {
                "ids": {"type": "array", "items": {"type": "string"}}
            }}),
        ),
        tool_with(
            "center_on_page",
            "Centre the selected objects on the work area.",
            json!({"type": "object", "properties": {
                "ids": {"type": "array", "items": {"type": "string"}}
            }}),
        ),
        tool_with(
            "distribute_on_page",
            "Spread the selected objects evenly across the work area.",
            json!({"type": "object", "properties": {
                "ids": {"type": "array", "items": {"type": "string"}},
                "axis": {"type": "string"}
            }}),
        ),
        tool_with(
            "copy_objects",
            "Copy the selected objects (all when ids are omitted) to the in-app clipboard.",
            json!({"type": "object", "properties": {
                "ids": {"type": "array", "items": {"type": "string"}}
            }}),
        ),
        tool_with(
            "cut_objects",
            "Cut the selected objects to the in-app clipboard.",
            json!({"type": "object", "properties": {
                "ids": {"type": "array", "items": {"type": "string"}}
            }}),
        ),
        tool(
            "paste_objects",
            "Paste the in-app clipboard, offset by 5 mm.",
        ),
        tool(
            "invert_selection",
            "Select every object that is not selected.",
        ),
        tool_with(
            "lock_objects",
            "Lock the selected objects.",
            json!({"type": "object", "properties": {
                "ids": {"type": "array", "items": {"type": "string"}}
            }}),
        ),
        tool_with(
            "unlock_objects",
            "Unlock the selected objects.",
            json!({"type": "object", "properties": {
                "ids": {"type": "array", "items": {"type": "string"}}
            }}),
        ),
        tool_with(
            "duplicate_grid",
            "Duplicate the selection in a grid with spacing, margins and fill order.",
            json!({"type": "object", "properties": {
                "ids": {"type": "array", "items": {"type": "string"}},
                "columns": {"type": "integer"},
                "rows": {"type": "integer"},
                "dx": {"type": "number"},
                "dy": {"type": "number"},
                "margin_x": {"type": "number"},
                "margin_y": {"type": "number"},
                "column_major": {"type": "boolean"}
            }}),
        ),
        tool_with(
            "repeat",
            "Duplicate the selection linearly or around a circle.",
            json!({"type": "object", "properties": {
                "ids": {"type": "array", "items": {"type": "string"}},
                "mode": {"type": "string"},
                "copies": {"type": "integer"},
                "dx": {"type": "number"},
                "dy": {"type": "number"},
                "radius": {"type": "number"},
                "rotation_degrees": {"type": "number"},
                "rotate_copies": {"type": "boolean"}
            }}),
        ),
        tool_with(
            "nest",
            "Nest the selected parts (all objects when ids are omitted) into the work area.",
            json!({"type": "object", "properties": {
                "ids": {"type": "array", "items": {"type": "string"}},
                "spacing": {"type": "number"},
                "margin": {"type": "number"},
                "rotation": {"type": "boolean"},
                "apply": {"type": "boolean"}
            }}),
        ),
        tool_with(
            "begin_node_edit",
            "Start editing the nodes of one path (converts primitives to paths).",
            json!({"type": "object", "properties": {"id": {"type": "string"}}, "required": ["id"]}),
        ),
        tool("end_node_edit", "Stop node editing."),
        tool(
            "list_nodes",
            "List the edited path's nodes with points, handles and selection.",
        ),
        tool_with(
            "move_node",
            "Move one node to an absolute position in millimetres.",
            json!({"type": "object", "properties": {
                "index": {"type": "integer"},
                "x": {"type": "number"},
                "y": {"type": "number"}
            }, "required": ["index", "x", "y"]}),
        ),
        tool_with(
            "insert_node",
            "Insert a node on a segment at parameter t (0..1).",
            json!({"type": "object", "properties": {
                "segment": {"type": "integer"},
                "t": {"type": "number"}
            }, "required": ["segment"]}),
        ),
        tool_with(
            "delete_node",
            "Delete one node of the edited path.",
            json!({"type": "object", "properties": {"index": {"type": "integer"}}, "required": ["index"]}),
        ),
        tool_with(
            "convert_node",
            "Convert one node to a smooth curve (default) or a corner.",
            json!({"type": "object", "properties": {
                "index": {"type": "integer"},
                "smooth": {"type": "boolean"}
            }, "required": ["index"]}),
        ),
        tool_with(
            "simplify_path",
            "Simplify the edited path within a tolerance in millimetres.",
            json!({"type": "object", "properties": {
                "tolerance": {"type": "number"}
            }, "required": ["tolerance"]}),
        ),
        tool_with(
            "align_handles",
            "Align one node's two handles into a straight line.",
            json!({"type": "object", "properties": {"index": {"type": "integer"}}, "required": ["index"]}),
        ),
        tool_with(
            "fit_text_to_path",
            "Lay a text's first line along another object's path as outlines.",
            json!({"type": "object", "properties": {
                "text": {"type": "string"},
                "path": {"type": "string"}
            }, "required": ["text", "path"]}),
        ),
        tool_with(
            "text_to_outlines",
            "Replace a text object with vector outlines from the bundled font.",
            json!({"type": "object", "properties": {"id": {"type": "string"}}, "required": ["id"]}),
        ),
        tool_with(
            "outline_stroke",
            "Replace strokes by their outline contours (all objects when ids are omitted).",
            json!({"type": "object", "properties": {
                "ids": {"type": "array", "items": {"type": "string"}}
            }}),
        ),
        tool_with(
            "trim",
            "Trim the selected shapes with the last selected closed shape as the trimmer.",
            json!({"type": "object", "properties": {
                "ids": {"type": "array", "items": {"type": "string"}}
            }}),
        ),
        tool_with(
            "delete_overlapping_segments",
            "Remove segments shared exactly by two selected paths.",
            json!({"type": "object", "properties": {
                "ids": {"type": "array", "items": {"type": "string"}}
            }}),
        ),
        tool_with(
            "join_paths",
            "Join open paths into one path (all objects when ids are omitted).",
            json!({"type": "object", "properties": {
                "ids": {"type": "array", "items": {"type": "string"}}
            }}),
        ),
        tool_with(
            "close_paths",
            "Close open paths (all objects when ids are omitted).",
            json!({"type": "object", "properties": {
                "ids": {"type": "array", "items": {"type": "string"}}
            }}),
        ),
        tool(
            "find_duplicates",
            "List groups of objects whose outlines are identical.",
        ),
        tool(
            "remove_duplicates",
            "Remove duplicate outlines, keeping the first copy.",
        ),
        tool(
            "find_intersections",
            "List outline intersections between the selected objects or the whole document.",
        ),
        tool_with(
            "export_svg",
            "Export the current document as SVG.",
            json!({"type": "object", "properties": {"path": {"type": "string"}}, "required": ["path"]}),
        ),
        tool_with(
            "export_dxf",
            "Export the current document as DXF.",
            json!({"type": "object", "properties": {"path": {"type": "string"}}, "required": ["path"]}),
        ),
        tool_with(
            "export_gcode",
            "Export a GRBL-compatible G-code job.",
            json!({"type": "object", "properties": {"path": {"type": "string"}}, "required": ["path"]}),
        ),
        tool("estimate", "Estimate job paths, lines, bytes and duration."),
    ]
}

fn tool(name: &str, description: &str) -> Value {
    tool_with(
        name,
        description,
        json!({"type": "object", "properties": {}}),
    )
}

#[allow(clippy::needless_pass_by_value)]
fn tool_with(name: &str, description: &str, schema: Value) -> Value {
    json!({"name": name, "description": description, "inputSchema": schema})
}

fn number(arguments: &Value, key: &str) -> Option<f64> {
    arguments.get(key).and_then(Value::as_f64)
}

fn positive(arguments: &Value, key: &str) -> Result<f64> {
    let value = number(arguments, key).ok_or_else(|| missing(key))?;
    if value <= 0.0 {
        return Err(BeamError::InvalidInput(format!("'{key}' must be positive")));
    }
    Ok(value)
}

fn string(arguments: &Value, key: &str) -> Option<String> {
    arguments
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn missing(key: &str) -> BeamError {
    BeamError::InvalidInput(format!("missing argument: {key}"))
}

fn path_argument(arguments: &Value, key: &str) -> Option<PathBuf> {
    string(arguments, key)
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from)
}

fn required_path(arguments: &Value, key: &str) -> Result<PathBuf> {
    path_argument(arguments, key).ok_or_else(|| missing(key))
}

fn index_argument(arguments: &Value, key: &str) -> Result<usize> {
    let value = arguments
        .get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| missing(key))?;
    usize::try_from(value).map_err(|_| BeamError::InvalidInput(format!("{key} is too large")))
}

fn parse_boolean_op(value: &str) -> Result<crate::geometry::BooleanOp> {
    match value {
        "union" => Ok(crate::geometry::BooleanOp::Union),
        "difference" | "subtract" => Ok(crate::geometry::BooleanOp::Difference),
        "intersection" => Ok(crate::geometry::BooleanOp::Intersection),
        "xor" | "symmetric_difference" => Ok(crate::geometry::BooleanOp::Xor),
        other => Err(BeamError::InvalidInput(format!(
            "unknown boolean operation: {other}"
        ))),
    }
}

fn parse_operation_type(value: &str) -> Result<OperationType> {
    match value {
        "line" => Ok(OperationType::Line),
        "line_engraving" => Ok(OperationType::LineEngraving),
        "fill" => Ok(OperationType::Fill),
        "fill_with_outline" => Ok(OperationType::FillWithOutline),
        "fill_with_offset" => Ok(OperationType::FillWithOffset),
        "outline_cutting" => Ok(OperationType::OutlineCutting),
        "image_processing" => Ok(OperationType::ImageProcessing),
        "custom" => Ok(OperationType::Custom),
        other => Err(BeamError::InvalidInput(format!(
            "unknown operation type: {other}"
        ))),
    }
}

#[allow(dead_code)]
fn _unused(_: &Path) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(clippy::needless_pass_by_value)]
    fn call(server: &mut McpServer, name: &str, arguments: Value) -> Value {
        server.handle(&json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {"name": name, "arguments": arguments}
        }))
    }

    #[test]
    fn initialize_and_list_tools() {
        let mut server = McpServer::new();
        let init = server.handle(&json!({"jsonrpc": "2.0", "id": 1, "method": "initialize"}));
        assert_eq!(init["result"]["serverInfo"]["name"], "beamwright");
        let list = server.handle(&json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}));
        let tools = list["result"]["tools"].as_array().unwrap();
        assert!(tools.len() >= 15);
    }

    #[test]
    fn new_project_and_status() {
        let mut server = McpServer::new();
        let response = call(
            &mut server,
            "new_project",
            json!({"name": "MCP", "width_mm": 120.0, "height_mm": 80.0}),
        );
        assert_eq!(response["result"]["width_mm"], 120.0);
        assert_eq!(response["result"]["layers"], 1);
    }

    #[test]
    fn draw_list_and_delete() {
        let mut server = McpServer::new();
        let created = call(
            &mut server,
            "add_rectangle",
            json!({"x": 5.0, "y": 5.0, "width": 20.0, "height": 10.0}),
        );
        let id = created["result"]["id"].as_str().unwrap().to_string();
        let list = call(&mut server, "list_objects", json!({}));
        assert_eq!(list["result"]["objects"].as_array().unwrap().len(), 1);
        assert_eq!(list["result"]["objects"][0]["type"], "Rectangle");
        let deleted = call(&mut server, "delete_objects", json!({"ids": [id]}));
        assert_eq!(deleted["result"]["removed"], 1);
    }

    #[test]
    fn boolean_and_offset_tools() {
        let mut server = McpServer::new();
        let first = call(
            &mut server,
            "add_rectangle",
            json!({"width": 10.0, "height": 10.0}),
        )["result"]["id"]
            .as_str()
            .unwrap()
            .to_string();
        let second = call(
            &mut server,
            "add_rectangle",
            json!({"x": 5.0, "y": 5.0, "width": 10.0, "height": 10.0}),
        )["result"]["id"]
            .as_str()
            .unwrap()
            .to_string();
        let union = call(
            &mut server,
            "boolean",
            json!({"operation": "union", "ids": [first, second]}),
        );
        assert_eq!(union["result"]["contours"], 1);
        assert_eq!(union["result"]["objects"], 1);
        let id = server.editor.document.objects[0].id.clone();
        let offset = call(&mut server, "offset", json!({"distance": 1.0, "ids": [id]}));
        assert_eq!(offset["result"]["offset"], 1);
        assert_eq!(server.editor.document.objects.len(), 1);
    }

    #[test]
    fn text_outline_tool() {
        let mut server = McpServer::new();
        let id = call(
            &mut server,
            "add_text",
            json!({"x": 10.0, "y": 20.0, "content": "AB", "size": 10.0}),
        )["result"]["id"]
            .as_str()
            .unwrap()
            .to_string();
        let outlines = call(&mut server, "text_to_outlines", json!({"id": id}));
        assert!(outlines["result"]["outlines"].as_u64().unwrap() >= 2);
        assert!(server.editor.document.object(&id).is_none());
        let missing = call(&mut server, "text_to_outlines", json!({"id": "missing"}));
        assert!(missing.get("error").is_some());
    }

    #[test]
    fn fit_text_to_path_tool() {
        let mut server = McpServer::new();
        let text = call(
            &mut server,
            "add_text",
            json!({"x": 0.0, "y": 0.0, "content": "AB", "size": 8.0}),
        )["result"]["id"]
            .as_str()
            .unwrap()
            .to_string();
        let path = call(
            &mut server,
            "add_line",
            json!({"x1": 0.0, "y1": 0.0, "x2": 40.0, "y2": 0.0}),
        )["result"]["id"]
            .as_str()
            .unwrap()
            .to_string();
        let fitted = call(
            &mut server,
            "fit_text_to_path",
            json!({"text": text, "path": path}),
        );
        assert!(fitted["result"]["outlines"].as_u64().unwrap() >= 2);
        assert!(server.editor.document.object(&text).is_none());
        assert!(server.editor.document.object(&path).is_some());
        assert!(
            call(
                &mut server,
                "fit_text_to_path",
                json!({"text": "x", "path": path})
            )
            .get("error")
            .is_some()
        );
    }

    #[test]
    fn operation_dependency_tool() {
        let mut server = McpServer::new();
        let first = call(
            &mut server,
            "create_operation",
            json!({"name": "First", "type": "line", "speed": 500.0}),
        )["result"]["id"]
            .as_str()
            .unwrap()
            .to_string();
        let second = call(
            &mut server,
            "create_operation",
            json!({"name": "Second", "type": "fill", "speed": 900.0}),
        )["result"]["id"]
            .as_str()
            .unwrap()
            .to_string();
        let set = call(
            &mut server,
            "set_operation_dependency",
            json!({"operation": second, "depends_on": first}),
        );
        assert!(set.get("error").is_none());
        assert_eq!(
            server.editor.document.operations[1].depends_on,
            vec![first.clone()]
        );
        let clear = call(
            &mut server,
            "set_operation_dependency",
            json!({"operation": second}),
        );
        assert!(clear.get("error").is_none());
        assert_eq!(server.editor.document.operations[1].depends_on.len(), 0);
        let bad = call(
            &mut server,
            "set_operation_dependency",
            json!({"operation": second, "depends_on": "missing"}),
        );
        assert!(bad.get("error").is_some());
    }

    #[test]
    fn layer_operation_tool() {
        let mut server = McpServer::new();
        call(
            &mut server,
            "add_rectangle",
            json!({"width": 2.0, "height": 2.0}),
        );
        let operation = call(
            &mut server,
            "create_operation",
            json!({"name": "Cut", "type": "outline_cutting"}),
        )["result"]["id"]
            .as_str()
            .unwrap()
            .to_string();
        let assigned = call(
            &mut server,
            "assign_operation_to_layer",
            json!({"operation": operation}),
        );
        assert_eq!(assigned["result"]["assigned"], 1);
        assert!(server.editor.document.objects[0].operation_id.is_some());
    }

    #[test]
    fn duplicate_operation_tool() {
        let mut server = McpServer::new();
        let id = call(
            &mut server,
            "create_operation",
            json!({"name": "Cut", "type": "outline_cutting", "speed": 700.0}),
        )["result"]["id"]
            .as_str()
            .unwrap()
            .to_string();
        let copy = call(&mut server, "duplicate_operation", json!({"id": id}));
        assert_eq!(copy["result"]["operations"], 2);
        let copy_id = copy["result"]["id"].as_str().unwrap();
        let source = server
            .editor
            .document
            .operations
            .iter()
            .find(|operation| operation.id == id)
            .unwrap();
        let duplicated = server
            .editor
            .document
            .operations
            .iter()
            .find(|operation| operation.id == copy_id)
            .unwrap();
        assert_eq!(source.params, duplicated.params);
        assert!(
            call(&mut server, "duplicate_operation", json!({"id": "missing"}))
                .get("error")
                .is_some()
        );
    }

    #[test]
    fn transform_tools() {
        let mut server = McpServer::new();
        call(
            &mut server,
            "add_rectangle",
            json!({"x": 10.0, "y": 10.0, "width": 20.0, "height": 10.0}),
        );
        assert!(
            call(
                &mut server,
                "resize_selection",
                json!({"width": 40.0, "height": 20.0})
            )
            .get("error")
            .is_none()
        );
        let bounds =
            call(&mut server, "list_objects", json!({}))["result"]["objects"][0]["bounds"].clone();
        let width = bounds["max"][0].as_f64().unwrap() - bounds["min"][0].as_f64().unwrap();
        assert!((width - 40.0).abs() < 1e-6);
        assert!(
            call(&mut server, "skew_selection", json!({"x_degrees": 30.0}))
                .get("error")
                .is_none()
        );
        let reset = call(&mut server, "reset_transforms", json!({}));
        assert_eq!(reset["result"]["reset"], 1);
    }

    #[test]
    fn ordering_and_centering_tools() {
        let mut server = McpServer::new();
        let first = call(
            &mut server,
            "add_rectangle",
            json!({"width": 2.0, "height": 2.0}),
        )["result"]["id"]
            .as_str()
            .unwrap()
            .to_string();
        call(
            &mut server,
            "add_rectangle",
            json!({"x": 10.0, "width": 2.0, "height": 2.0}),
        );
        let front = call(&mut server, "bring_to_front", json!({"ids": [first]}));
        assert_eq!(front["result"]["moved"], 1);
        assert_eq!(server.editor.document.objects[1].id, first);
        let centered = call(&mut server, "center_on_page", json!({"ids": [first]}));
        assert_eq!(centered["result"]["centered"], true);
        let (min, max) = server
            .editor
            .document
            .object(&first)
            .unwrap()
            .bounds()
            .unwrap();
        assert!((f64::midpoint(min.x, max.x) - 150.0).abs() < 1e-6);
        let back = call(&mut server, "send_to_back", json!({"ids": [first]}));
        assert_eq!(back["result"]["moved"], 1);
        assert_eq!(server.editor.document.objects[0].id, first);
    }

    #[test]
    fn clipboard_and_lock_tools() {
        let mut server = McpServer::new();
        let id = call(
            &mut server,
            "add_rectangle",
            json!({"width": 4.0, "height": 4.0}),
        )["result"]["id"]
            .as_str()
            .unwrap()
            .to_string();
        assert_eq!(
            call(&mut server, "copy_objects", json!({"ids": [id]}))["result"]["copied"],
            1
        );
        assert_eq!(
            call(&mut server, "paste_objects", json!({}))["result"]["pasted"],
            1
        );
        assert_eq!(server.editor.document.objects.len(), 2);
        let locked = call(&mut server, "lock_objects", json!({"ids": [id]}));
        assert_eq!(locked["result"]["locked"], 1);
        assert!(server.editor.document.object(&id).unwrap().locked);
        let unlocked = call(&mut server, "unlock_objects", json!({"ids": [id]}));
        assert_eq!(unlocked["result"]["unlocked"], 1);
        let cut = call(&mut server, "cut_objects", json!({"ids": [id]}));
        assert_eq!(cut["result"]["cut"], 1);
        assert_eq!(cut["result"]["objects"], 1);
        assert_eq!(
            call(&mut server, "invert_selection", json!({}))["result"]["selected"],
            1
        );
    }

    #[test]
    fn grid_tool_uses_margins() {
        let mut server = McpServer::new();
        call(
            &mut server,
            "add_rectangle",
            json!({"width": 2.0, "height": 2.0}),
        );
        let grid = call(
            &mut server,
            "duplicate_grid",
            json!({"columns": 2, "rows": 1, "dx": 10.0, "margin_x": 5.0}),
        );
        assert_eq!(grid["result"]["created"], 1);
        let object = &server.editor.document.objects[1];
        let (min, _) = object.bounds().unwrap();
        assert!((min.x - 15.0).abs() < 1e-9);
    }

    #[test]
    fn repeat_tool_copies_objects() {
        let mut server = McpServer::new();
        let id = call(
            &mut server,
            "add_rectangle",
            json!({"width": 5.0, "height": 5.0}),
        )["result"]["id"]
            .as_str()
            .unwrap()
            .to_string();
        let linear = call(
            &mut server,
            "repeat",
            json!({"ids": [id], "mode": "linear", "copies": 2, "dx": 10.0}),
        );
        assert_eq!(linear["result"]["created"], 2);
        assert_eq!(linear["result"]["objects"], 3);
        let circular = call(
            &mut server,
            "repeat",
            json!({"ids": [id], "mode": "circular", "copies": 3, "radius": 10.0}),
        );
        assert_eq!(circular["result"]["created"], 3);
        let bad = call(&mut server, "repeat", json!({"mode": "spiral"}));
        assert!(bad.get("error").is_some());
    }

    #[test]
    fn nest_tool_proposes_and_applies() {
        let mut server = McpServer::new();
        call(
            &mut server,
            "add_rectangle",
            json!({"x": 10.0, "y": 10.0, "width": 20.0, "height": 15.0}),
        );
        call(
            &mut server,
            "add_rectangle",
            json!({"x": 50.0, "y": 50.0, "width": 10.0, "height": 20.0}),
        );
        let preview = call(&mut server, "nest", json!({"spacing": 1.0}));
        assert_eq!(preview["result"]["placements"].as_array().unwrap().len(), 2);
        assert_eq!(preview["result"]["moved"], 0);
        let applied = call(&mut server, "nest", json!({"spacing": 1.0, "apply": true}));
        assert_eq!(applied["result"]["moved"], 2);
        for object in &server.editor.document.objects {
            let (min, _) = object.bounds().unwrap();
            assert!(min.x >= 0.0 && min.y >= 0.0);
        }
    }

    #[test]
    fn node_editing_tools() {
        let mut server = McpServer::new();
        let id = call(
            &mut server,
            "add_rectangle",
            json!({"width": 10.0, "height": 10.0}),
        )["result"]["id"]
            .as_str()
            .unwrap()
            .to_string();
        let started = call(&mut server, "begin_node_edit", json!({"id": id}));
        assert_eq!(started["result"]["editing"], true);
        assert_eq!(started["result"]["nodes"].as_array().unwrap().len(), 4);
        let moved = call(
            &mut server,
            "move_node",
            json!({"index": 0, "x": -2.0, "y": -2.0}),
        );
        assert_eq!(moved["result"]["nodes"][0]["point"][0], -2.0);
        let inserted = call(&mut server, "insert_node", json!({"segment": 0, "t": 0.5}));
        assert_eq!(inserted["result"]["nodes"], 5);
        let converted = call(
            &mut server,
            "convert_node",
            json!({"index": 1, "smooth": true}),
        );
        assert_eq!(converted["result"]["converted"], 1);
        let deleted = call(&mut server, "delete_node", json!({"index": 1}));
        assert_eq!(deleted["result"]["removed"], 1);
        assert_eq!(deleted["result"]["nodes"], 4);
        let list = call(&mut server, "list_nodes", json!({}));
        assert_eq!(list["result"]["editing"], true);
        call(
            &mut server,
            "convert_node",
            json!({"index": 0, "smooth": true}),
        );
        let aligned = call(&mut server, "align_handles", json!({"index": 0}));
        assert_eq!(aligned["result"]["aligned"], 1);
        call(&mut server, "end_node_edit", json!({}));
        assert!(!server.editor.is_editing_nodes());
    }

    #[test]
    fn trim_and_overlap_tools() {
        let mut server = McpServer::new();
        let line = call(
            &mut server,
            "add_line",
            json!({"x1": 0.0, "y1": 0.0, "x2": 10.0, "y2": 0.0}),
        )["result"]["id"]
            .as_str()
            .unwrap()
            .to_string();
        let trimmer = call(
            &mut server,
            "add_rectangle",
            json!({"x": 4.0, "y": -1.0, "width": 2.0, "height": 2.0}),
        )["result"]["id"]
            .as_str()
            .unwrap()
            .to_string();
        let result = call(&mut server, "trim", json!({"ids": [line, trimmer]}));
        assert_eq!(result["result"]["trimmed"], 1);
        assert_eq!(result["result"]["objects"], 3);
        let error = call(&mut server, "delete_overlapping_segments", json!({}));
        assert!(error.get("error").is_some());
    }

    #[test]
    fn outline_tool_creates_contours() {
        let mut server = McpServer::new();
        let id = call(
            &mut server,
            "add_rectangle",
            json!({"width": 10.0, "height": 10.0}),
        )["result"]["id"]
            .as_str()
            .unwrap()
            .to_string();
        let outline = call(&mut server, "outline_stroke", json!({"ids": [id]}));
        assert_eq!(outline["result"]["contours"], 2);
        assert_eq!(outline["result"]["objects"], 2);
    }

    #[test]
    fn join_and_close_tools() {
        let mut server = McpServer::new();
        let first = call(
            &mut server,
            "add_line",
            json!({"x1": 0.0, "y1": 0.0, "x2": 10.0, "y2": 0.0}),
        )["result"]["id"]
            .as_str()
            .unwrap()
            .to_string();
        let second = call(
            &mut server,
            "add_line",
            json!({"x1": 10.0, "y1": 0.0, "x2": 10.0, "y2": 10.0}),
        )["result"]["id"]
            .as_str()
            .unwrap()
            .to_string();
        let joined = call(&mut server, "join_paths", json!({"ids": [first, second]}));
        assert_eq!(joined["result"]["objects"], 1);
        let close = call(&mut server, "close_paths", json!({}));
        assert_eq!(close["result"]["closed"], 1);
        assert!(matches!(
            server.editor.document.objects[0].shape,
            Shape::Path(ref path) if path.closed
        ));
    }

    #[test]
    fn duplicate_and_intersection_tools() {
        let mut server = McpServer::new();
        let first = call(
            &mut server,
            "add_rectangle",
            json!({"width": 4.0, "height": 4.0}),
        )["result"]["id"]
            .as_str()
            .unwrap()
            .to_string();
        call(
            &mut server,
            "add_rectangle",
            json!({"width": 4.0, "height": 4.0}),
        );
        call(
            &mut server,
            "add_rectangle",
            json!({"x": 2.0, "y": 2.0, "width": 4.0, "height": 4.0}),
        );
        let duplicates = call(&mut server, "find_duplicates", json!({}));
        assert_eq!(duplicates["result"]["count"], 1);
        assert_eq!(
            duplicates["result"]["groups"][0][0].as_str().unwrap(),
            first
        );
        let intersections = call(&mut server, "find_intersections", json!({}));
        // The two copies overlap each other collinearly and give no crossing,
        // but each one crosses the offset rectangle twice.
        assert_eq!(intersections["result"]["count"], 4);
        let removed = call(&mut server, "remove_duplicates", json!({}));
        assert_eq!(removed["result"]["removed"], 1);
        assert_eq!(removed["result"]["objects"], 2);
    }

    #[test]
    fn boolean_rejects_bad_input() {
        let mut server = McpServer::new();
        let id = call(
            &mut server,
            "add_rectangle",
            json!({"width": 4.0, "height": 4.0}),
        )["result"]["id"]
            .as_str()
            .unwrap()
            .to_string();
        let unknown = call(
            &mut server,
            "boolean",
            json!({"operation": "union", "ids": [id.clone(), "missing"]}),
        );
        assert!(unknown.get("error").is_some());
        let single = call(
            &mut server,
            "boolean",
            json!({"operation": "union", "ids": [id]}),
        );
        assert!(single.get("error").is_some());
        let operation = call(
            &mut server,
            "boolean",
            json!({"operation": "teleport", "ids": [id]}),
        );
        assert!(operation.get("error").is_some());
        assert_eq!(server.editor.document.objects.len(), 1);
    }

    #[test]
    fn invalid_dimensions_are_rejected() {
        let mut server = McpServer::new();
        let response = call(
            &mut server,
            "add_rectangle",
            json!({"width": 0.0, "height": 10.0}),
        );
        assert!(response.get("error").is_some());
        assert_eq!(server.editor.document.objects.len(), 0);
    }

    #[test]
    fn operation_and_gcode_flow() {
        let mut server = McpServer::new();
        call(
            &mut server,
            "add_rectangle",
            json!({"width": 10.0, "height": 5.0}),
        );
        let operation = call(
            &mut server,
            "create_operation",
            json!({"name": "Cut", "type": "outline_cutting", "speed": 800.0, "power": 60.0, "passes": 1}),
        );
        let operation_id = operation["result"]["id"].as_str().unwrap().to_string();
        let assigned = call(
            &mut server,
            "assign_operation",
            json!({"operation": operation_id}),
        );
        assert_eq!(assigned["result"]["assigned"], 1);
        let estimate = call(&mut server, "estimate", json!({}));
        assert_eq!(estimate["result"]["paths"], 1);
        assert!(estimate["result"]["seconds"].as_f64().unwrap() > 0.0);

        let path = std::env::temp_dir().join("beamwright_mcp_job.nc");
        let _ = std::fs::remove_file(&path);
        let exported = call(
            &mut server,
            "export_gcode",
            json!({"path": path.to_string_lossy()}),
        );
        assert!(exported.get("error").is_none());
        assert!(std::fs::read_to_string(&path).unwrap().contains("F800"));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn save_and_open_round_trip() {
        let path = std::env::temp_dir().join("beamwright_mcp_project.bwp");
        let _ = std::fs::remove_file(&path);
        let mut server = McpServer::new();
        call(
            &mut server,
            "add_circle",
            json!({"x": 10.0, "y": 10.0, "radius": 4.0}),
        );
        let saved = call(
            &mut server,
            "save_project",
            json!({"path": path.to_string_lossy()}),
        );
        assert_eq!(saved["result"]["status"], "saved");

        let mut reopened = McpServer::new();
        let opened = call(
            &mut reopened,
            "open_project",
            json!({"path": path.to_string_lossy()}),
        );
        assert_eq!(opened["result"]["objects"], 1);
        assert_eq!(
            reopened.editor.document.objects[0].shape.kind_name(),
            "Ellipse"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn unknown_tool_and_missing_arguments_error() {
        let mut server = McpServer::new();
        assert!(call(&mut server, "nope", json!({})).get("error").is_some());
        assert!(
            call(&mut server, "add_line", json!({"x1": 0.0}))
                .get("error")
                .is_some()
        );
        assert!(
            call(&mut server, "import_file", json!({}))
                .get("error")
                .is_some()
        );
    }

    #[test]
    fn parse_error_is_reported() {
        let mut server = McpServer::new();
        let response = server.handle(&json!({"jsonrpc": "2.0", "id": 9, "method": "unknown"}));
        assert_eq!(response["error"]["code"], -32000);
        assert_eq!(response["id"], 9);
    }

    #[test]
    fn import_svg_into_server() {
        let path = std::env::temp_dir().join("beamwright_mcp_import.svg");
        std::fs::write(
            &path,
            "<svg width=\"20\" height=\"10\"><rect width=\"5\" height=\"5\"/></svg>",
        )
        .unwrap();
        let mut server = McpServer::new();
        let response = call(
            &mut server,
            "import_file",
            json!({"path": path.to_string_lossy()}),
        );
        assert_eq!(response["result"]["objects"], 1);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn operation_type_is_validated() {
        let mut server = McpServer::new();
        let response = call(
            &mut server,
            "create_operation",
            json!({"name": "Bad", "type": "teleport"}),
        );
        assert!(response.get("error").is_some());
    }
}
