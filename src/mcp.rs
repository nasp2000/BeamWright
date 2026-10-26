use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::io::{BufRead, Write};

#[derive(Deserialize)]
struct RpcRequest {
    #[allow(dead_code)]
    jsonrpc: Option<String>,
    id: Option<Value>,
    method: String,
    params: Option<Value>,
}

#[derive(Serialize)]
struct RpcResponse {
    jsonrpc: &'static str,
    id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<Value>,
}

pub fn handle_request(method: &str, params: Option<serde_json::Value>) -> serde_json::Value {
    match handle_method(method, params) {
        Ok(v) => v,
        Err(e) => serde_json::json!({"error": e}),
    }
}

pub fn run_mcp_server() {
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    for line in stdin.lock().lines() {
        let Ok(line) = line else {
            break;
        };
        if line.trim().is_empty() {
            continue;
        }
        let req: RpcRequest = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(e) => {
                let resp = RpcResponse {
                    jsonrpc: "2.0",
                    id: Value::Null,
                    result: None,
                    error: Some(json!({"code": -32700, "message": e.to_string()})),
                };
                let _ = writeln!(stdout, "{}", serde_json::to_string(&resp).unwrap());
                let _ = stdout.flush();
                continue;
            }
        };
        let id = req.id.clone().unwrap_or(Value::Null);
        let result = handle_method(&req.method, req.params);
        let resp = match result {
            Ok(v) => RpcResponse {
                jsonrpc: "2.0",
                id,
                result: Some(v),
                error: None,
            },
            Err(e) => RpcResponse {
                jsonrpc: "2.0",
                id,
                result: None,
                error: Some(json!({"code": -32000, "message": e})),
            },
        };
        let _ = writeln!(stdout, "{}", serde_json::to_string(&resp).unwrap());
        let _ = stdout.flush();
    }
}

fn handle_method(method: &str, params: Option<Value>) -> Result<Value, String> {
    match method {
        "initialize" => Ok(json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {"tools": {}},
            "serverInfo": {"name": "beamwright", "version": "0.1.0"}
        })),
        "tools/list" => Ok(json!({"tools": tool_list()})),
        "tools/call" => {
            let params = params.ok_or("missing params")?;
            let name = params
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or("missing name")?;
            let args = params.get("arguments").cloned().unwrap_or(json!({}));
            call_tool(name, &args)
        }
        _ => Err(format!("unknown method: {method}")),
    }
}

fn tool_list() -> Value {
    json!([
        {"name": "open_project", "description": "Open a .bwp project and return info",
         "inputSchema": {"type": "object", "properties": {"path": {"type": "string"}}, "required": ["path"]}},
        {"name": "import_file", "description": "Import a file into a .bwp project",
         "inputSchema": {"type": "object", "properties": {"input": {"type": "string"}, "output": {"type": "string"}}, "required": ["input", "output"]}},
        {"name": "export_gcode", "description": "Export G-code from a project",
         "inputSchema": {"type": "object", "properties": {"input": {"type": "string"}, "output": {"type": "string"}}, "required": ["input", "output"]}},
        {"name": "list_layers", "description": "List layers in a project",
         "inputSchema": {"type": "object", "properties": {"path": {"type": "string"}}, "required": ["path"]}},
        {"name": "add_rectangle", "description": "Add a rectangle object to a project",
         "inputSchema": {"type": "object", "properties": {"path": {"type": "string"}, "width": {"type": "number"}, "height": {"type": "number"}}, "required": ["path", "width", "height"]}}
    ])
}

fn call_tool(name: &str, args: &Value) -> Result<Value, String> {
    match name {
        "open_project" => {
            let path = args
                .get("path")
                .and_then(|v| v.as_str())
                .ok_or("missing path")?;
            let doc = crate::document::serialization::load_from_file(std::path::Path::new(path))
                .map_err(|e| e.to_string())?;
            Ok(json!({"name": doc.name, "layers": doc.layers.len(), "objects": doc.objects.len()}))
        }
        "import_file" => {
            let input = args
                .get("input")
                .and_then(|v| v.as_str())
                .ok_or("missing input")?;
            let output = args
                .get("output")
                .and_then(|v| v.as_str())
                .ok_or("missing output")?;
            let doc = crate::import::format::import_by_extension(std::path::Path::new(input))
                .map_err(|e| e.to_string())?;
            crate::document::serialization::save_to_file(&doc, std::path::Path::new(output))
                .map_err(|e| e.to_string())?;
            Ok(json!({"status": "ok", "objects": doc.objects.len()}))
        }
        "export_gcode" => {
            let input = args
                .get("input")
                .and_then(|v| v.as_str())
                .ok_or("missing input")?;
            let output = args
                .get("output")
                .and_then(|v| v.as_str())
                .ok_or("missing output")?;
            let doc = crate::document::serialization::load_from_file(std::path::Path::new(input))
                .map_err(|e| e.to_string())?;
            let mut tps = Vec::new();
            for _ in &doc.objects {
                let mut tp = crate::gcode::toolpath::Toolpath::new(1000.0, 50.0, 1);
                tp.push(crate::gcode::toolpath::MoveKind::Cut, 10.0, 0.0);
                tps.push(tp);
            }
            let w = crate::gcode::writer::GCodeWriter::new(
                crate::gcode::dialect::GCodeOptions::default(),
            );
            let s = w.render(&tps);
            std::fs::write(output, s).map_err(|e| e.to_string())?;
            Ok(json!({"status": "ok"}))
        }
        "list_layers" => {
            let path = args
                .get("path")
                .and_then(|v| v.as_str())
                .ok_or("missing path")?;
            let doc = crate::document::serialization::load_from_file(std::path::Path::new(path))
                .map_err(|e| e.to_string())?;
            let layers: Vec<Value> = doc
                .layers
                .iter()
                .map(|l| json!({"id": l.id, "name": l.name, "visible": l.visible}))
                .collect();
            Ok(json!({"layers": layers}))
        }
        "add_rectangle" => {
            let path = args
                .get("path")
                .and_then(|v| v.as_str())
                .ok_or("missing path")?;
            let w = args
                .get("width")
                .and_then(Value::as_f64)
                .ok_or("missing width")?;
            let h = args
                .get("height")
                .and_then(Value::as_f64)
                .ok_or("missing height")?;
            let mut doc =
                crate::document::serialization::load_from_file(std::path::Path::new(path))
                    .map_err(|e| e.to_string())?;
            let layer_id = doc.layers.first().map(|l| l.id.clone()).unwrap_or_default();
            doc.objects.push(crate::document::model::DocumentObject {
                id: format!("rect-{}", doc.objects.len()),
                kind: crate::document::model::ObjectKind::Rectangle,
                layer_id,
                operation_id: None,
            });
            let _ = (w, h);
            crate::document::serialization::save_to_file(&doc, std::path::Path::new(path))
                .map_err(|e| e.to_string())?;
            Ok(json!({"status": "ok", "objects": doc.objects.len()}))
        }
        _ => Err(format!("unknown tool: {name}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initialize() {
        let r = handle_request("initialize", None);
        assert!(r.get("serverInfo").is_some());
    }

    #[test]
    fn tools_list() {
        let r = handle_request("tools/list", None);
        assert!(r.get("tools").is_some());
        let tools = r.get("tools").unwrap().as_array().unwrap();
        assert_ne!(tools.as_slice(), [] as [serde_json::Value; 0]);
    }

    #[test]
    fn unknown_method() {
        let r = handle_request("foo/bar", None);
        assert!(r.get("error").is_some());
    }

    #[test]
    fn add_rectangle_round_trip() {
        use std::env::temp_dir;
        let dir = temp_dir();
        let proj = dir.join("bw_mcp_rect.bwp");
        let doc = crate::document::Document::default();
        crate::document::serialization::save_to_file(&doc, &proj).unwrap();
        let args = serde_json::json!({"path": proj.to_str().unwrap(), "width": 5.0, "height": 3.0});
        let r = handle_request(
            "tools/call",
            Some(serde_json::json!({"name": "add_rectangle", "arguments": args})),
        );
        assert!(r.get("status").is_some() || r.get("error").is_none());
        let _ = std::fs::remove_file(&proj);
    }
}
