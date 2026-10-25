#![allow(dead_code)]

mod app;
mod config;
mod document;
mod editor;
mod errors;
mod estimates;
mod export;
mod gcode;
mod geometry;
mod history;
mod import;
mod layers;
mod materials;
mod messages;
mod objects;
mod operations;
mod preview;
mod profiles;
mod storage;
mod ui;

use app::BeamWrightApp;

fn main() -> eframe::Result<()> {
    tracing::info!("starting BeamWright");
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("BeamWright")
            .with_inner_size([1280.0, 800.0]),
        ..Default::default()
    };
    eframe::run_native(
        "BeamWright",
        options,
        Box::new(|_cc| Ok(Box::new(BeamWrightApp::default()))),
    )
}
