use beamwright::cli::{Cli, Commands, run};
use clap::Parser;
use std::env::temp_dir;
use std::fs;

#[test]
fn cli_help_parses() {
    let cli = Cli::try_parse_from(["BeamWright", "--help"]).err();
    // --help exits successfully, which clap reports as an error value.
    assert!(cli.is_some());
}

#[test]
fn cli_no_subcommand() {
    let cli = Cli::try_parse_from(["BeamWright"]).unwrap();
    assert!(cli.command.is_none());
    assert!(cli.file.is_none());
}

#[test]
fn cli_positional_file() {
    let cli = Cli::try_parse_from(["BeamWright", "drawing.bwp"]).unwrap();
    assert!(cli.command.is_none());
    assert_eq!(
        cli.file.as_deref().and_then(|path| path.to_str()),
        Some("drawing.bwp")
    );
}

#[test]
fn cli_info_command() {
    let path = temp_dir().join("bw_cli_info.bwp");
    let document = beamwright::document::Document::default();
    beamwright::document::serialization::save_to_file(&document, &path).unwrap();
    let cli = Cli::try_parse_from(["BeamWright", "info", path.to_str().unwrap()]).unwrap();
    assert!(matches!(cli.command, Some(Commands::Info { .. })));
    run(cli).unwrap();
    let _ = fs::remove_file(&path);
}

#[test]
fn cli_list_layers_command() {
    let path = temp_dir().join("bw_cli_layers.bwp");
    let document = beamwright::document::Document::default();
    beamwright::document::serialization::save_to_file(&document, &path).unwrap();
    let cli = Cli::try_parse_from(["BeamWright", "list-layers", path.to_str().unwrap()]).unwrap();
    run(cli).unwrap();
    let _ = fs::remove_file(&path);
}

#[test]
fn cli_import_command() {
    let source = temp_dir().join("bw_cli_import.svg");
    let output = temp_dir().join("bw_cli_import.bwp");
    fs::write(
        &source,
        "<svg width=\"10\" height=\"10\"><rect x=\"0\" y=\"0\" width=\"1\" height=\"1\"/></svg>",
    )
    .unwrap();
    let _ = fs::remove_file(&output);
    let cli = Cli::try_parse_from([
        "BeamWright",
        "import",
        source.to_str().unwrap(),
        output.to_str().unwrap(),
    ])
    .unwrap();
    run(cli).unwrap();
    assert!(output.exists());
    let _ = fs::remove_file(&source);
    let _ = fs::remove_file(&output);
}

#[test]
fn cli_export_svg_command() {
    let project = temp_dir().join("bw_cli_export.bwp");
    let output = temp_dir().join("bw_cli_export.svg");
    let document = beamwright::document::Document::default();
    beamwright::document::serialization::save_to_file(&document, &project).unwrap();
    let _ = fs::remove_file(&output);
    let cli = Cli::try_parse_from([
        "BeamWright",
        "export-svg",
        project.to_str().unwrap(),
        output.to_str().unwrap(),
    ])
    .unwrap();
    run(cli).unwrap();
    assert!(output.exists());
    let _ = fs::remove_file(&project);
    let _ = fs::remove_file(&output);
}

#[test]
fn cli_info_missing_file_errors() {
    let cli = Cli::try_parse_from(["BeamWright", "info", "/tmp/opencode/nope.bwp"]).unwrap();
    assert!(run(cli).is_err());
}
