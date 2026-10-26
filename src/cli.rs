use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "BeamWright",
    version,
    about = "Vector drawing and laser job preparation"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Print document info
    Info { path: PathBuf },
    /// Import a file and save as .bwp
    Import { input: PathBuf, output: PathBuf },
    /// Export G-code from a project
    ExportGcode { input: PathBuf, output: PathBuf },
    /// List layers in a project
    ListLayers { path: PathBuf },
}

pub fn run(cli: Cli) -> Result<(), Box<dyn std::error::Error>> {
    match cli.command {
        None => Ok(()), // GUI mode handled by caller
        Some(Commands::Info { path }) => {
            let doc = crate::document::serialization::load_from_file(&path)?;
            println!("Name: {}", doc.name);
            println!(
                "Size: {}x{} {}",
                doc.width_mm,
                doc.height_mm,
                doc.unit.label()
            );
            println!("Layers: {}", doc.layers.len());
            println!("Objects: {}", doc.objects.len());
            Ok(())
        }
        Some(Commands::Import { input, output }) => {
            let doc = crate::import::format::import_by_extension(&input)?;
            crate::document::serialization::save_to_file(&doc, &output)?;
            println!("Imported {} -> {}", input.display(), output.display());
            Ok(())
        }
        Some(Commands::ExportGcode { input, output }) => {
            let doc = crate::document::serialization::load_from_file(&input)?;
            // build a simple toolpath per object
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
            std::fs::write(&output, s)?;
            println!("Exported G-code -> {}", output.display());
            Ok(())
        }
        Some(Commands::ListLayers { path }) => {
            let doc = crate::document::serialization::load_from_file(&path)?;
            for l in &doc.layers {
                println!("{}: {} (visible={})", l.id, l.name, l.visible);
            }
            Ok(())
        }
    }
}
