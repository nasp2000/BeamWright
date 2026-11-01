use std::path::PathBuf;

use clap::{Parser, Subcommand};

use crate::document::{Document, serialization};
use crate::errors::{BeamError, Result};
use crate::export::export_svg;
use crate::import::format::open_document;

#[derive(Parser, Debug)]
#[command(
    name = "BeamWright",
    version,
    about = "Vector drawing and laser job preparation"
)]
pub struct Cli {
    /// Project or SVG file to open in the editor.
    pub file: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Print document information
    Info { path: PathBuf },
    /// Import an SVG and save it as a native .bwp project
    Import { input: PathBuf, output: PathBuf },
    /// Export a project or SVG as SVG
    ExportSvg { input: PathBuf, output: PathBuf },
    /// Export a project or SVG as DXF
    ExportDxf { input: PathBuf, output: PathBuf },
    /// Export a project or SVG as PNG
    ExportPng {
        input: PathBuf,
        output: PathBuf,
        /// Output resolution in dots per inch
        #[arg(long, default_value_t = 96.0)]
        dpi: f64,
    },
    /// Export a GRBL-compatible G-code job
    ExportGcode { input: PathBuf, output: PathBuf },
    /// Print the estimated job duration and G-code size
    Estimate { path: PathBuf },
    /// List layers in a project
    ListLayers { path: PathBuf },
    /// Recover a project from its backup
    Recover { path: PathBuf },
    /// Combine two closed shapes and save the project
    Boolean {
        input: PathBuf,
        output: PathBuf,
        /// union, difference, intersection or xor
        operation: String,
        /// Comma-separated object ids (defaults to the first two objects)
        #[arg(long)]
        ids: Option<String>,
    },
    /// Offset closed shapes and save the project
    Offset {
        input: PathBuf,
        output: PathBuf,
        /// Distance in millimetres; positive expands, negative contracts
        distance: f64,
        /// Comma-separated object ids (defaults to every object)
        #[arg(long)]
        ids: Option<String>,
    },
    /// Run the MCP server on stdin/stdout
    Mcp {
        /// Control the running editor instead of a separate document
        #[arg(long)]
        live: bool,
        /// Live bridge host
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
        /// Live bridge port
        #[arg(long, default_value_t = crate::bridge::DEFAULT_PORT)]
        port: u16,
        /// Bridge token for a non-loopback host
        #[arg(long)]
        token: Option<String>,
    },
    /// Run MCP over HTTP with Bearer authentication (remote agents)
    McpHttp {
        /// Address to listen on, e.g. 100.64.0.5:8766
        #[arg(long, default_value = "127.0.0.1:8766")]
        listen: String,
        /// Bearer token; required for a non-loopback address
        #[arg(long)]
        token: Option<String>,
    },
    /// Send one JSON-RPC request to the running editor
    LiveRequest {
        /// JSON-RPC request object
        json: String,
        /// Live bridge host
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
        /// Live bridge port
        #[arg(long, default_value_t = crate::bridge::DEFAULT_PORT)]
        port: u16,
        /// Bridge token for a non-loopback host
        #[arg(long)]
        token: Option<String>,
    },
    /// Describe a project or drawing: layers, objects, operations and parameters
    Inspect {
        path: PathBuf,
        /// Print the report as JSON
        #[arg(long)]
        json: bool,
        /// Limit the object list to these ids
        #[arg(long, value_delimiter = ',')]
        ids: Vec<String>,
    },
    /// Measure lengths, areas, bounds and job distances
    Measure {
        path: PathBuf,
        /// Print the report as JSON
        #[arg(long)]
        json: bool,
        /// Limit the measurement to these ids
        #[arg(long, value_delimiter = ',')]
        ids: Vec<String>,
        /// Include hidden objects
        #[arg(long)]
        include_hidden: bool,
    },
    /// Preflight a project or drawing without exporting
    Check {
        path: PathBuf,
        /// Print the full report as JSON
        #[arg(long)]
        json: bool,
    },
    /// Apply a JSON batch of tool calls and save the project
    Apply {
        input: PathBuf,
        /// JSON plan with a "steps" array (or a bare array)
        #[arg(long)]
        batch: PathBuf,
        /// Write the result here instead of overwriting the input
        #[arg(long)]
        output: Option<PathBuf>,
        /// Validate and report without saving
        #[arg(long)]
        dry_run: bool,
    },
    /// Measure build, export, save and G-code times on a synthetic project
    Bench {
        /// Number of objects to create
        #[arg(long, default_value_t = 500)]
        objects: usize,
    },
}

#[allow(clippy::too_many_lines)]
pub fn run(cli: Cli) -> Result<()> {
    match cli.command {
        None => Ok(()),
        Some(Commands::Info { path }) => {
            let document = open_document(&path)?;
            print_info(&document);
            Ok(())
        }
        Some(Commands::Import { input, output }) => {
            let outcome = crate::import::format::import_with_report(&input)?;
            serialization::save_to_file(&outcome.document, &output)?;
            println!(
                "Imported {} -> {} ({} objects)",
                input.display(),
                output.display(),
                outcome.document.objects.len()
            );
            for warning in outcome.warnings {
                eprintln!("warning: {warning}");
            }
            Ok(())
        }
        Some(Commands::ExportSvg { input, output }) => {
            let document = open_document(&input)?;
            let svg = export_svg(&document)?;
            std::fs::write(&output, svg)?;
            println!("Exported SVG -> {}", output.display());
            Ok(())
        }
        Some(Commands::ExportDxf { input, output }) => {
            let document = open_document(&input)?;
            let dxf = crate::export::export_dxf(&document)?;
            std::fs::write(&output, dxf)?;
            println!("Exported DXF -> {}", output.display());
            Ok(())
        }
        Some(Commands::ExportPng { input, output, dpi }) => {
            let document = open_document(&input)?;
            let options = crate::export::PngOptions {
                dpi,
                transparent: false,
            };
            let report = crate::export::export_png(&document, &output, &options)?;
            println!(
                "Exported PNG -> {} ({} x {} px)",
                output.display(),
                report.width_px,
                report.height_px
            );
            for warning in report.warnings {
                eprintln!("warning: {warning}");
            }
            Ok(())
        }
        Some(Commands::ExportGcode { input, output }) => {
            let document = open_document(&input)?;
            let report =
                crate::gcode::export_job(&document, &output, &crate::gcode::JobOptions::default())?;
            println!(
                "Exported G-code -> {} ({} paths, {} lines, {:.1} s)",
                output.display(),
                report.paths,
                report.lines,
                report.seconds
            );
            for warning in report.warnings {
                eprintln!("warning: {warning}");
            }
            Ok(())
        }
        Some(Commands::Estimate { path }) => {
            let document = open_document(&path)?;
            let (text, report) =
                crate::gcode::render_job(&document, &crate::gcode::JobOptions::default())?;
            println!("Paths: {}", report.paths);
            println!("Lines: {}", report.lines);
            println!("Bytes: {}", text.len());
            println!("Estimated duration: {:.1} s", report.seconds);
            for warning in report.warnings {
                println!("Warning: {warning}");
            }
            Ok(())
        }
        Some(Commands::Boolean {
            input,
            output,
            operation,
            ids,
        }) => {
            let document = open_document(&input)?;
            let mut editor = crate::editor::EditorState::new(document);
            let op = parse_boolean_operation(&operation)?;
            select_objects(&mut editor, ids.as_deref(), 2)?;
            let contours = editor
                .boolean_selection(op)
                .map_err(BeamError::InvalidInput)?;
            serialization::save_to_file(&editor.document, &output)?;
            println!(
                "Boolean {} -> {} contour(s) in {}",
                op.label(),
                contours,
                output.display()
            );
            Ok(())
        }
        Some(Commands::Offset {
            input,
            output,
            distance,
            ids,
        }) => {
            let document = open_document(&input)?;
            let mut editor = crate::editor::EditorState::new(document);
            select_objects(&mut editor, ids.as_deref(), usize::MAX)?;
            let count = editor
                .offset_selection(distance)
                .map_err(BeamError::InvalidInput)?;
            serialization::save_to_file(&editor.document, &output)?;
            println!(
                "Offset {distance} mm applied to {count} shape(s) -> {}",
                output.display()
            );
            Ok(())
        }
        Some(Commands::Recover { path }) => {
            let (document, note) = serialization::load_with_recovery(&path)?;
            serialization::save_to_file(&document, &path)?;
            println!(
                "Recovered {} ({})",
                path.display(),
                note.unwrap_or_else(|| "project was already valid".to_string())
            );
            Ok(())
        }
        Some(Commands::Mcp {
            live,
            host,
            port,
            token,
        }) => {
            if live {
                let token = token.or_else(crate::bridge::configured_token).or_else(|| {
                    if host == "127.0.0.1" || host == "localhost" {
                        crate::bridge::stored_token()
                    } else {
                        None
                    }
                });
                crate::bridge::proxy_stdio(&host, port, token).map_err(BeamError::InvalidInput)?;
            } else {
                let mut server = crate::mcp::McpServer::new();
                server.serve_stdio()?;
            }
            Ok(())
        }
        Some(Commands::McpHttp { listen, token }) => {
            let token = token.or_else(crate::bridge::configured_token).or_else(|| {
                if crate::mcp_http::is_loopback(&listen) {
                    crate::bridge::stored_token()
                } else {
                    None
                }
            });
            let Some(token) = token else {
                return Err(BeamError::InvalidInput(format!(
                    "refusing to listen on {listen} without --token"
                )));
            };
            crate::mcp_http::serve(
                &listen,
                &token,
                "127.0.0.1",
                crate::bridge::configured_port(),
            )
            .map_err(BeamError::InvalidInput)?;
            Ok(())
        }
        Some(Commands::LiveRequest {
            json,
            host,
            port,
            token,
        }) => {
            let token = token.or_else(crate::bridge::configured_token).or_else(|| {
                if host == "127.0.0.1" || host == "localhost" {
                    crate::bridge::stored_token()
                } else {
                    None
                }
            });
            println!(
                "{}",
                crate::bridge::request(&host, port, token, &json)
                    .map_err(BeamError::InvalidInput)?
            );
            Ok(())
        }
        Some(Commands::Inspect { path, json, ids }) => {
            let document = open_document(&path)?;
            let filter = (!ids.is_empty()).then_some(ids.as_slice());
            let report = crate::inspection::inspect(&document, filter);
            if json {
                println!("{}", pretty(&report)?);
            } else {
                print_inspection(&report);
            }
            Ok(())
        }
        Some(Commands::Measure {
            path,
            json,
            ids,
            include_hidden,
        }) => {
            let document = open_document(&path)?;
            let filter = (!ids.is_empty()).then_some(ids.as_slice());
            let report = crate::inspection::measure(&document, filter, include_hidden);
            if json {
                println!("{}", pretty(&report)?);
            } else {
                print_measurement(&report);
            }
            Ok(())
        }
        Some(Commands::Check { path, json }) => {
            let outcome = crate::import::format::import_with_report(&path)?;
            let report = crate::preflight::check(&outcome.document);
            if json {
                let text = serde_json::to_string_pretty(&report.to_json())
                    .map_err(|error| BeamError::InvalidInput(error.to_string()))?;
                println!("{text}");
            } else {
                println!(
                    "{}: {} error(s), {} warning(s)",
                    if report.is_ok() { "ready" } else { "not ready" },
                    report.errors.len(),
                    report.warnings.len()
                );
                for issue in &report.errors {
                    println!("error [{}] {}", issue.code, issue.message);
                }
                for issue in &report.warnings {
                    println!("warning [{}] {}", issue.code, issue.message);
                }
            }
            if !report.is_ok() {
                std::process::exit(1);
            }
            Ok(())
        }
        Some(Commands::Apply {
            input,
            batch,
            output,
            dry_run,
        }) => {
            let text = std::fs::read_to_string(&batch)?;
            let plan: serde_json::Value = serde_json::from_str(&text)
                .map_err(|error| BeamError::InvalidInput(format!("invalid batch JSON: {error}")))?;
            let steps = plan
                .get("steps")
                .and_then(serde_json::Value::as_array)
                .or_else(|| plan.as_array())
                .cloned()
                .ok_or_else(|| {
                    BeamError::InvalidInput(
                        "the batch must be an array or an object with a steps array".to_string(),
                    )
                })?;
            let mut server = crate::mcp::McpServer::new();
            let opened = server.handle(&serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "tools/call",
                "params": {
                    "name": "open_project",
                    "arguments": {"path": input.to_string_lossy()}
                }
            }));
            if let Some(message) = response_error(&opened) {
                return Err(BeamError::InvalidInput(message));
            }
            let result = server.apply_batch(&steps)?;
            if !dry_run {
                let target = output.unwrap_or_else(|| input.clone());
                let saved = server.handle(&serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": 2,
                    "method": "tools/call",
                    "params": {
                        "name": "save_project",
                        "arguments": {"path": target.to_string_lossy()}
                    }
                }));
                if let Some(message) = response_error(&saved) {
                    return Err(BeamError::InvalidInput(message));
                }
            }
            let text = serde_json::to_string_pretty(&result)
                .map_err(|error| BeamError::InvalidInput(error.to_string()))?;
            println!("{text}");
            Ok(())
        }
        Some(Commands::Bench { objects }) => {
            run_bench(objects);
            Ok(())
        }
        Some(Commands::ListLayers { path }) => {
            let document = open_document(&path)?;
            for layer in &document.layers {
                println!(
                    "{}: {} (visible={}, locked={}, objects={})",
                    layer.id,
                    layer.name,
                    layer.visible,
                    layer.locked,
                    document.objects_on_layer(&layer.id).len()
                );
            }
            Ok(())
        }
    }
}

/// Build a synthetic project and time the main file and job operations.
///
/// This is the measurement tool: run it on the target
/// hardware to record import, export, save and G-code times.
fn pretty(value: &serde_json::Value) -> Result<String> {
    serde_json::to_string_pretty(value).map_err(|error| BeamError::InvalidInput(error.to_string()))
}

fn print_inspection(report: &serde_json::Value) {
    let project = &report["project"];
    println!(
        "{}: {} x {} {}",
        project["name"].as_str().unwrap_or("Untitled"),
        project["width_mm"],
        project["height_mm"],
        project["unit"].as_str().unwrap_or("mm")
    );
    let counts = &report["counts"];
    println!(
        "{} layer(s), {} object(s), {} operation(s), {} parameter(s)",
        counts["layers"], counts["objects"], counts["operations"], counts["parameters"]
    );
    for object in report["objects"].as_array().into_iter().flatten() {
        println!(
            "  {} [{}] layer {} operation {} length {:.3} mm area {:.3} mm2",
            object["name"].as_str().unwrap_or("?"),
            object["kind"].as_str().unwrap_or("?"),
            object["layer"].as_str().unwrap_or("(none)"),
            object["operation"].as_str().unwrap_or("(none)"),
            object["length_mm"].as_f64().unwrap_or(0.0),
            object["area_mm2"].as_f64().unwrap_or(0.0)
        );
    }
    for parameter in report["parameters"].as_array().into_iter().flatten() {
        match parameter["value"].as_f64() {
            Some(value) => println!(
                "  parameter {} = {} {}",
                parameter["name"].as_str().unwrap_or("?"),
                value,
                parameter["unit"].as_str().unwrap_or("")
            ),
            None => println!(
                "  parameter {} is invalid: {}",
                parameter["name"].as_str().unwrap_or("?"),
                parameter["error"].as_str().unwrap_or("")
            ),
        }
    }
}

fn print_measurement(report: &serde_json::Value) {
    println!(
        "{} object(s): {:.3} mm of path, {:.3} mm2 gross, {:.3} mm2 net",
        report["objects"],
        report["length_mm"].as_f64().unwrap_or(0.0),
        report["area_mm2"].as_f64().unwrap_or(0.0),
        report["net_area_mm2"].as_f64().unwrap_or(0.0)
    );
    if let Some(bounds) = report["bounds_mm"].as_object() {
        println!(
            "  bounds {:.3} x {:.3} mm at ({:.3}, {:.3})",
            bounds["width"].as_f64().unwrap_or(0.0),
            bounds["height"].as_f64().unwrap_or(0.0),
            bounds["min_x"].as_f64().unwrap_or(0.0),
            bounds["min_y"].as_f64().unwrap_or(0.0)
        );
    }
    for entry in report["by_operation"].as_array().into_iter().flatten() {
        println!(
            "  {}: {} object(s), {:.3} mm",
            entry["operation"].as_str().unwrap_or("(none)"),
            entry["objects"],
            entry["length_mm"].as_f64().unwrap_or(0.0)
        );
    }
    if let Some(job) = report["job"].as_object() {
        println!(
            "  job: {} path(s), {:.3} mm cut, {:.3} mm travel",
            job["paths"],
            job["cut_mm"].as_f64().unwrap_or(0.0),
            job["travel_mm"].as_f64().unwrap_or(0.0)
        );
    }
}

fn response_error(response: &serde_json::Value) -> Option<String> {
    response
        .get("error")
        .and_then(|error| error.get("message"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
}

fn run_bench(count: usize) {
    use std::time::Instant;

    let count = count.clamp(1, 10_000);
    let start = Instant::now();
    let mut document = Document::new("Benchmark", 300.0, 200.0);
    let layer = document.layers[0].id.clone();
    for index in 0..count {
        #[allow(clippy::cast_precision_loss)]
        let column = index % 25;
        #[allow(clippy::cast_precision_loss)]
        let row = index / 25;
        #[allow(clippy::cast_precision_loss)]
        let x = 5.0 + column as f64 * 11.0;
        #[allow(clippy::cast_precision_loss)]
        let y = 5.0 + row as f64 * 11.0;
        let shape = if index % 5 == 0 {
            crate::document::Shape::Ellipse { rx: 4.0, ry: 3.0 }
        } else {
            crate::document::Shape::Rect {
                width: 8.0,
                height: 6.0,
                corner_radius: 0.0,
            }
        };
        let object = crate::document::Object::new(layer.clone(), shape)
            .with_transform(crate::geometry::Transform2::translate(x, y));
        document.add_object(object);
    }
    println!(
        "Build: {:.1} ms ({} objects)",
        start.elapsed().as_secs_f64() * 1000.0,
        count
    );

    let timer = Instant::now();
    match crate::export::export_svg(&document) {
        Ok(svg) => println!(
            "SVG export: {:.1} ms ({} bytes)",
            timer.elapsed().as_secs_f64() * 1000.0,
            svg.len()
        ),
        Err(error) => println!("SVG export failed: {error}"),
    }

    let timer = Instant::now();
    match crate::gcode::render_job(&document, &crate::gcode::JobOptions::default()) {
        Ok((_text, report)) => println!(
            "G-code: {:.1} ms ({} paths, {} lines, {:.1} s estimated)",
            timer.elapsed().as_secs_f64() * 1000.0,
            report.paths,
            report.lines,
            report.seconds
        ),
        Err(error) => println!("G-code failed: {error}"),
    }

    let timer = Instant::now();
    match serialization::to_json(&document) {
        Ok(json) => {
            println!(
                "Serialize: {:.1} ms ({} bytes)",
                timer.elapsed().as_secs_f64() * 1000.0,
                json.len()
            );
            let timer = Instant::now();
            match serialization::from_json(&json) {
                Ok(loaded) => println!(
                    "Deserialize: {:.1} ms ({} objects)",
                    timer.elapsed().as_secs_f64() * 1000.0,
                    loaded.objects.len()
                ),
                Err(error) => println!("Deserialize failed: {error}"),
            }
        }
        Err(error) => println!("Serialize failed: {error}"),
    }
}

fn parse_boolean_operation(value: &str) -> Result<crate::geometry::BooleanOp> {
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

/// Select objects by id, or the first `default_count` objects in the document.
fn select_objects(
    editor: &mut crate::editor::EditorState,
    ids: Option<&str>,
    default_count: usize,
) -> Result<()> {
    let selected: Vec<String> = match ids {
        Some(list) => {
            let mut resolved = Vec::new();
            for id in list.split(',').map(str::trim).filter(|id| !id.is_empty()) {
                if editor.document.object(id).is_none() {
                    return Err(BeamError::InvalidInput(format!("unknown object id: {id}")));
                }
                resolved.push(id.to_string());
            }
            resolved
        }
        None => editor
            .document
            .objects
            .iter()
            .take(default_count)
            .map(|object| object.id.clone())
            .collect(),
    };
    editor.selection.clear();
    for id in selected {
        editor.selection.add(id);
    }
    Ok(())
}

fn print_info(document: &Document) {
    println!("Name: {}", document.name);
    println!("Size: {} x {} mm", document.width_mm, document.height_mm);
    println!("Layers: {}", document.layers.len());
    println!("Objects: {}", document.objects.len());
    println!("Format version: {}", document.format_version);
}
