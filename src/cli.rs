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
        /// Live bridge port
        #[arg(long, default_value_t = crate::bridge::DEFAULT_PORT)]
        port: u16,
    },
    /// Send one JSON-RPC request to the running editor
    LiveRequest {
        /// JSON-RPC request object
        json: String,
        /// Live bridge port
        #[arg(long, default_value_t = crate::bridge::DEFAULT_PORT)]
        port: u16,
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
        Some(Commands::Mcp { live, port }) => {
            if live {
                crate::bridge::proxy_stdio(port).map_err(BeamError::InvalidInput)?;
            } else {
                let mut server = crate::mcp::McpServer::new();
                server.serve_stdio()?;
            }
            Ok(())
        }
        Some(Commands::LiveRequest { json, port }) => {
            println!(
                "{}",
                crate::bridge::request(port, &json).map_err(BeamError::InvalidInput)?
            );
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
/// This is the measurement tool from PLAN.md §25.3: run it on the target
/// hardware to record import, export, save and G-code times.
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
