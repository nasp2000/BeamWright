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
}

impl Default for McpServer {
    fn default() -> Self {
        Self::new()
    }
}

impl McpServer {
    pub fn new() -> Self {
        Self {
            editor: EditorState::new(Document::default()),
            path: None,
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
                    operation.validate().map_err(BeamError::InvalidInput)?;
                }
                Ok(json!({"id": id, "operations": self.editor.document.operations.len()}))
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
        self.editor.record();
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
                "line_interval_mm": {"type": "number"}
            }}),
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
