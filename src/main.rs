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
        Box::new(|cc| {
            cc.egui_ctx.set_visuals(egui::Visuals::light());
            Ok(Box::new(BeamWrightApp::default()))
        }),
    )
}
