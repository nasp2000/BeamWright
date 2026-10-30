use std::path::PathBuf;

use clap::{Parser, Subcommand};

use crate::document::{Document, serialization};
use crate::errors::Result;
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
    /// Export a GRBL-compatible G-code job
    ExportGcode { input: PathBuf, output: PathBuf },
    /// Print the estimated job duration and G-code size
    Estimate { path: PathBuf },
    /// List layers in a project
    ListLayers { path: PathBuf },
}

pub fn run(cli: Cli) -> Result<()> {
    match cli.command {
        None => Ok(()),
        Some(Commands::Info { path }) => {
            let document = open_document(&path)?;
            print_info(&document);
            Ok(())
        }
        Some(Commands::Import { input, output }) => {
            let document = open_document(&input)?;
            serialization::save_to_file(&document, &output)?;
            println!("Imported {} -> {}", input.display(), output.display());
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

fn print_info(document: &Document) {
    println!("Name: {}", document.name);
    println!("Size: {} x {} mm", document.width_mm, document.height_mm);
    println!("Layers: {}", document.layers.len());
    println!("Objects: {}", document.objects.len());
    println!("Format version: {}", document.format_version);
}
