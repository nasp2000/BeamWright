use beamwright::cli::{Cli, Commands, run};
use clap::Parser;
use std::env::temp_dir;
use std::fs;

#[test]
fn cli_help_parses() {
    let cli = Cli::try_parse_from(["BeamWright", "--help"]).err();
    // --help exits with error code 0 in clap, so parse returns Err
    assert!(cli.is_some());
}

#[test]
fn cli_no_subcommand() {
    let cli = Cli::try_parse_from(["BeamWright"]).unwrap();
    assert!(cli.command.is_none());
}

#[test]
fn cli_info_command() {
    let dir = temp_dir();
    let proj = dir.join("bw_cli_info.bwp");
    let doc = beamwright::document::Document::default();
    beamwright::document::serialization::save_to_file(&doc, &proj).unwrap();
    let cli = Cli::try_parse_from(["BeamWright", "info", proj.to_str().unwrap()]).unwrap();
    assert!(matches!(cli.command, Some(Commands::Info { .. })));
    run(cli).unwrap();
    let _ = fs::remove_file(&proj);
}

#[test]
fn cli_list_layers_command() {
    let dir = temp_dir();
    let proj = dir.join("bw_cli_layers.bwp");
    let doc = beamwright::document::Document::default();
    beamwright::document::serialization::save_to_file(&doc, &proj).unwrap();
    let cli = Cli::try_parse_from(["BeamWright", "list-layers", proj.to_str().unwrap()]).unwrap();
    run(cli).unwrap();
    let _ = fs::remove_file(&proj);
}

#[test]
fn cli_import_command() {
    let dir = temp_dir();
    let src = dir.join("bw_cli_import.svg");
    let out = dir.join("bw_cli_import.bwp");
    fs::write(
        &src,
        "<svg width=\"10\" height=\"10\"><rect x=\"0\" y=\"0\" width=\"1\" height=\"1\"/></svg>",
    )
    .unwrap();
    let _ = fs::remove_file(&out);
    let cli = Cli::try_parse_from([
        "BeamWright",
        "import",
        src.to_str().unwrap(),
        out.to_str().unwrap(),
    ])
    .unwrap();
    run(cli).unwrap();
    assert!(out.exists());
    let _ = fs::remove_file(&src);
    let _ = fs::remove_file(&out);
}

#[test]
fn cli_export_gcode_command() {
    let dir = temp_dir();
    let proj = dir.join("bw_cli_gcode.bwp");
    let out = dir.join("bw_cli_gcode.nc");
    let doc = beamwright::document::Document::default();
    beamwright::document::serialization::save_to_file(&doc, &proj).unwrap();
    let _ = fs::remove_file(&out);
    let cli = Cli::try_parse_from([
        "BeamWright",
        "export-gcode",
        proj.to_str().unwrap(),
        out.to_str().unwrap(),
    ])
    .unwrap();
    run(cli).unwrap();
    assert!(out.exists());
    let _ = fs::remove_file(&proj);
    let _ = fs::remove_file(&out);
}

#[test]
fn cli_info_missing_file_errors() {
    let cli = Cli::try_parse_from(["BeamWright", "info", "/tmp/opencode/nope.bwp"]).unwrap();
    assert!(run(cli).is_err());
}
