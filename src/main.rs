#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]
#![allow(dead_code)]

/// Reattach a console when a Windows GUI build is launched from a terminal with
/// CLI arguments, so `info`, `import`, the exporters and `mcp` still print.
///
/// The portable release is a GUI-subsystem executable: without this, Windows
/// would show a second black console window behind the editor.
#[cfg(all(windows, not(debug_assertions)))]
fn attach_parent_console() {
    const ATTACH_PARENT_PROCESS: u32 = 0xFFFF_FFFF;
    unsafe extern "system" {
        fn AttachConsole(process_id: u32) -> i32;
    }
    if std::env::args_os().len() > 1 {
        // SAFETY: AttachConsole only associates the process with its parent's
        // console; it does not dereference pointers or own any memory.
        unsafe {
            AttachConsole(ATTACH_PARENT_PROCESS);
        }
    }
}

#[cfg(not(all(windows, not(debug_assertions))))]
fn attach_parent_console() {}

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
    attach_parent_console();
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
