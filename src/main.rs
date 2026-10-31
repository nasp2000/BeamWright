//! `BeamWright` entry point.
//!
//! The portable Windows release is built as a GUI-subsystem executable so no
//! console window appears behind the editor. CLI output needs a console build:
//! the `console` feature selects the console subsystem, and every non-Windows
//! or debug build already has one.
#![cfg_attr(
    all(windows, not(debug_assertions), not(feature = "console")),
    windows_subsystem = "windows"
)]
#![allow(dead_code)]

fn load_icon() -> egui::IconData {
    let bytes = include_bytes!("../assets/icons/beamwright.png");
    if let Ok(img) = image::load_from_memory(bytes) {
        let rgba = img.to_rgba8();
        let (w, h) = rgba.dimensions();
        return egui::IconData {
            rgba: rgba.into_raw(),
            width: w,
            height: h,
        };
    }
    egui::IconData {
        rgba: vec![],
        width: 0,
        height: 0,
    }
}

use beamwright::app::BeamWrightApp;
use clap::Parser;

fn main() -> eframe::Result<()> {
    tracing::info!("starting BeamWright");
    let cli = beamwright::cli::Cli::parse();
    if cli.command.is_some() {
        if let Err(e) = beamwright::cli::run(cli) {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
        return Ok(());
    }
    let icon_data = load_icon();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("BeamWright")
            .with_inner_size([1280.0, 800.0])
            .with_icon(icon_data),
        ..Default::default()
    };
    eframe::run_native(
        "BeamWright",
        options,
        Box::new(move |cc| {
            cc.egui_ctx.set_visuals(egui::Visuals::light());
            Ok(Box::new(BeamWrightApp::new(cli.file)))
        }),
    )
}
