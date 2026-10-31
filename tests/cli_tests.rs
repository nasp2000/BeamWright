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

#[test]
fn cli_export_gcode_command() {
    use beamwright::document::{Document, Object, Shape};

    let project = temp_dir().join("bw_cli_job.bwp");
    let output = temp_dir().join("bw_cli_job.nc");
    let mut document = Document::default();
    let layer_id = document.layers[0].id.clone();
    document.add_object(Object::new(
        layer_id,
        Shape::Rect {
            width: 10.0,
            height: 5.0,
            corner_radius: 0.0,
        },
    ));
    beamwright::document::serialization::save_to_file(&document, &project).unwrap();
    let _ = fs::remove_file(&output);
    let cli = Cli::try_parse_from([
        "BeamWright",
        "export-gcode",
        project.to_str().unwrap(),
        output.to_str().unwrap(),
    ])
    .unwrap();
    run(cli).unwrap();
    let text = fs::read_to_string(&output).unwrap();
    assert!(text.contains("G21"));
    assert!(text.contains("G1 X"));
    assert!(text.contains("M5"));
    let _ = fs::remove_file(&project);
    let _ = fs::remove_file(&output);
}

#[test]
fn cli_export_gcode_out_of_bounds_errors() {
    use beamwright::document::{Document, Object, Shape};

    let project = temp_dir().join("bw_cli_job_bad.bwp");
    let output = temp_dir().join("bw_cli_job_bad.nc");
    let mut document = Document {
        width_mm: 5.0,
        height_mm: 5.0,
        ..Document::default()
    };
    let layer_id = document.layers[0].id.clone();
    document.add_object(Object::new(
        layer_id,
        Shape::Rect {
            width: 10.0,
            height: 5.0,
            corner_radius: 0.0,
        },
    ));
    beamwright::document::serialization::save_to_file(&document, &project).unwrap();
    let cli = Cli::try_parse_from([
        "BeamWright",
        "export-gcode",
        project.to_str().unwrap(),
        output.to_str().unwrap(),
    ])
    .unwrap();
    assert!(run(cli).is_err());
    assert!(!output.exists());
    let _ = fs::remove_file(&project);
}

#[test]
fn cli_estimate_command() {
    let project = temp_dir().join("bw_cli_estimate.bwp");
    let document = beamwright::document::Document::default();
    beamwright::document::serialization::save_to_file(&document, &project).unwrap();
    let cli = Cli::try_parse_from(["BeamWright", "estimate", project.to_str().unwrap()]).unwrap();
    run(cli).unwrap();
    let _ = fs::remove_file(&project);
}

#[test]
fn cli_export_dxf_command() {
    let project = temp_dir().join("bw_cli_dxf.bwp");
    let output = temp_dir().join("bw_cli_dxf.dxf");
    let document = beamwright::document::Document::default();
    beamwright::document::serialization::save_to_file(&document, &project).unwrap();
    let _ = fs::remove_file(&output);
    let cli = Cli::try_parse_from([
        "BeamWright",
        "export-dxf",
        project.to_str().unwrap(),
        output.to_str().unwrap(),
    ])
    .unwrap();
    run(cli).unwrap();
    let text = fs::read_to_string(&output).unwrap();
    assert!(text.contains("ENTITIES"));
    let _ = fs::remove_file(&project);
    let _ = fs::remove_file(&output);
}

#[test]
fn cli_import_dxf_command() {
    let source = temp_dir().join("bw_cli_input.dxf");
    let output = temp_dir().join("bw_cli_input.bwp");
    fs::write(
        &source,
        "0\nSECTION\n2\nENTITIES\n0\nLINE\n10\n0\n20\n0\n11\n10\n21\n0\n0\nENDSEC\n0\nEOF\n",
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
fn cli_export_png_command() {
    use beamwright::document::{Document, Object, Shape};

    let project = temp_dir().join("bw_cli_png.bwp");
    let output = temp_dir().join("bw_cli_png.png");
    let mut document = Document::default();
    let layer_id = document.layers[0].id.clone();
    let mut object = Object::new(
        layer_id,
        Shape::Rect {
            width: 20.0,
            height: 10.0,
            corner_radius: 0.0,
        },
    );
    object.style.fill = Some("#ff0000".to_string());
    document.add_object(object);
    beamwright::document::serialization::save_to_file(&document, &project).unwrap();
    let _ = fs::remove_file(&output);
    let cli = Cli::try_parse_from([
        "BeamWright",
        "export-png",
        project.to_str().unwrap(),
        output.to_str().unwrap(),
        "--dpi",
        "48",
    ])
    .unwrap();
    run(cli).unwrap();
    let image = image::open(&output).unwrap();
    assert!(image.width() > 0);
    let _ = fs::remove_file(&project);
    let _ = fs::remove_file(&output);
}

#[test]
fn cli_export_png_rejects_bad_dpi() {
    let project = temp_dir().join("bw_cli_png_dpi.bwp");
    let output = temp_dir().join("bw_cli_png_dpi.png");
    let document = beamwright::document::Document::default();
    beamwright::document::serialization::save_to_file(&document, &project).unwrap();
    let cli = Cli::try_parse_from([
        "BeamWright",
        "export-png",
        project.to_str().unwrap(),
        output.to_str().unwrap(),
        "--dpi",
        "0",
    ])
    .unwrap();
    assert!(run(cli).is_err());
    assert!(!output.exists());
    let _ = fs::remove_file(&project);
}

#[test]
fn cli_recover_restores_the_backup() {
    let path = temp_dir().join("bw_cli_recover.bwp");
    let backup = beamwright::document::serialization::backup_path(&path);
    let _ = fs::remove_file(&path);
    let _ = fs::remove_file(&backup);
    let document = beamwright::document::Document {
        name: "Recovered".to_string(),
        ..beamwright::document::Document::default()
    };
    beamwright::document::serialization::save_to_file(&document, &path).unwrap();
    beamwright::document::serialization::save_to_file(&document, &path).unwrap();
    fs::write(&path, "{broken").unwrap();
    let cli = Cli::try_parse_from(["BeamWright", "recover", path.to_str().unwrap()]).unwrap();
    run(cli).unwrap();
    let reloaded = beamwright::document::serialization::load_from_file(&path).unwrap();
    assert_eq!(reloaded.name, "Recovered");
    let _ = fs::remove_file(&path);
    let _ = fs::remove_file(&backup);
}

#[test]
fn cli_recover_without_backup_fails() {
    let path = temp_dir().join("bw_cli_recover_bad.bwp");
    let backup = beamwright::document::serialization::backup_path(&path);
    let _ = fs::remove_file(&backup);
    fs::write(&path, "{broken").unwrap();
    let cli = Cli::try_parse_from(["BeamWright", "recover", path.to_str().unwrap()]).unwrap();
    assert!(run(cli).is_err());
    let _ = fs::remove_file(&path);
}

#[test]
fn cli_import_raster_png() {
    let source = temp_dir().join("bw_cli_raster.png");
    let project = temp_dir().join("bw_cli_raster.bwp");
    let image = image::RgbaImage::from_pixel(8, 4, image::Rgba([0, 0, 0, 255]));
    image.save(&source).unwrap();
    let _ = fs::remove_file(&project);
    let cli = Cli::try_parse_from([
        "BeamWright",
        "import",
        source.to_str().unwrap(),
        project.to_str().unwrap(),
    ])
    .unwrap();
    run(cli).unwrap();
    let document = beamwright::document::serialization::load_from_file(&project).unwrap();
    assert_eq!(document.objects.len(), 1);
    assert!(matches!(
        document.objects[0].shape,
        beamwright::document::Shape::Image(_)
    ));
    let _ = fs::remove_file(&source);
    let _ = fs::remove_file(&project);
}

fn two_rectangle_project(path: &std::path::Path) {
    use beamwright::geometry::Point2;

    let document = beamwright::document::Document::new("Boolean", 100.0, 100.0);
    let mut editor = beamwright::editor::EditorState::new(document);
    editor.add_rectangle(Point2::new(0.0, 0.0), Point2::new(4.0, 4.0));
    editor.add_rectangle(Point2::new(2.0, 2.0), Point2::new(6.0, 6.0));
    beamwright::document::serialization::save_to_file(&editor.document, path).unwrap();
}

#[test]
fn cli_boolean_union_writes_one_contour() {
    let source = temp_dir().join("bw_cli_bool.bwp");
    let output = temp_dir().join("bw_cli_bool_out.bwp");
    let _ = fs::remove_file(&source);
    let _ = fs::remove_file(&output);
    two_rectangle_project(&source);
    let cli = Cli::try_parse_from([
        "BeamWright",
        "boolean",
        source.to_str().unwrap(),
        output.to_str().unwrap(),
        "union",
    ])
    .unwrap();
    run(cli).unwrap();
    let document = beamwright::document::serialization::load_from_file(&output).unwrap();
    assert_eq!(document.objects.len(), 1);
    assert!(document.objects[0].name.starts_with("Union"));
    let (min, max) = document.objects[0].bounds().unwrap();
    assert!((min.x - 0.0).abs() < 1e-6);
    assert!((max.x - 6.0).abs() < 1e-6);
    let _ = fs::remove_file(&source);
    let _ = fs::remove_file(&output);
}

#[test]
fn cli_offset_grows_a_shape() {
    use beamwright::geometry::Point2;

    let source = temp_dir().join("bw_cli_offset.bwp");
    let output = temp_dir().join("bw_cli_offset_out.bwp");
    let _ = fs::remove_file(&source);
    let _ = fs::remove_file(&output);
    let document = beamwright::document::Document::new("Offset", 100.0, 100.0);
    let mut editor = beamwright::editor::EditorState::new(document);
    editor.add_rectangle(Point2::new(10.0, 10.0), Point2::new(20.0, 20.0));
    beamwright::document::serialization::save_to_file(&editor.document, &source).unwrap();
    let cli = Cli::try_parse_from([
        "BeamWright",
        "offset",
        source.to_str().unwrap(),
        output.to_str().unwrap(),
        "2.0",
    ])
    .unwrap();
    run(cli).unwrap();
    let document = beamwright::document::serialization::load_from_file(&output).unwrap();
    let (min, max) = document.objects[0].bounds().unwrap();
    assert!((min.x - 8.0).abs() < 1e-6);
    assert!((max.y - 22.0).abs() < 1e-6);
    let _ = fs::remove_file(&source);
    let _ = fs::remove_file(&output);
}

#[test]
fn cli_boolean_rejects_unknown_ids() {
    let source = temp_dir().join("bw_cli_bad_bool.bwp");
    let output = temp_dir().join("bw_cli_bad_bool_out.bwp");
    let _ = fs::remove_file(&source);
    let _ = fs::remove_file(&output);
    two_rectangle_project(&source);
    let cli = Cli::try_parse_from([
        "BeamWright",
        "boolean",
        source.to_str().unwrap(),
        output.to_str().unwrap(),
        "xor",
        "--ids",
        "missing",
    ])
    .unwrap();
    assert!(run(cli).is_err());
    assert!(!output.exists());
    let _ = fs::remove_file(&source);
}
