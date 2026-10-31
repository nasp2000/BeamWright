//! Cross-module workflows from PLAN.md §27.2: file round-trips, profiles,
//! recovery, raster jobs and preview data.

use std::fs;
use std::path::PathBuf;

use beamwright::document::{Document, Object, Shape};
use beamwright::editor::EditorState;
use beamwright::gcode::{JobOptions, Playback, planned_toolpaths, render_job};
use beamwright::geometry::Point2;

fn temporary(name: &str) -> PathBuf {
    std::env::temp_dir().join(name)
}

fn sample_document() -> Document {
    let mut document = Document::new("Workflow", 200.0, 100.0);
    let layer = document.layers[0].id.clone();
    document.add_object(
        Object::new(
            layer.clone(),
            Shape::Rect {
                width: 20.0,
                height: 10.0,
                corner_radius: 0.0,
            },
        )
        .with_transform(beamwright::geometry::Transform2::translate(10.0, 10.0)),
    );
    document.add_object(
        Object::new(layer.clone(), Shape::Ellipse { rx: 5.0, ry: 3.0 })
            .with_transform(beamwright::geometry::Transform2::translate(50.0, 20.0)),
    );
    document.add_object(Object::new(
        layer,
        Shape::Path(beamwright::geometry::Path::from_points(
            &[(80.0, 10.0), (90.0, 10.0)],
            false,
        )),
    ));
    document
}

#[test]
fn dxf_export_and_import_round_trip() {
    let document = sample_document();
    let dxf = beamwright::export::export_dxf(&document).unwrap();
    let imported = beamwright::import::dxf::import_dxf_report(&dxf).unwrap();
    assert_eq!(imported.document.objects.len(), document.objects.len());
    assert_eq!(imported.warnings.len(), 0);
    let (min, max) = imported.document.objects[0].bounds().unwrap();
    assert!((min.x - 10.0).abs() < 1e-6);
    assert!((max.x - 30.0).abs() < 1e-6);
}

#[test]
fn svg_export_and_import_round_trip() {
    let document = sample_document();
    let svg = beamwright::export::export_svg(&document).unwrap();
    let report = beamwright::import::svg::import_svg_report(&svg).unwrap();
    assert_eq!(report.document.objects.len(), document.objects.len());
    assert_eq!(report.warnings.len(), 0);
}

#[test]
fn applied_profile_values_survive_profile_deletion() {
    use beamwright::operations::OperationType;
    use beamwright::profiles::{OperationProfile, ProfileLibrary};

    let mut library = ProfileLibrary::with_builtins();
    let mut profile = OperationProfile::new("Workflow cut", OperationType::OutlineCutting);
    profile.params.speed = 1234.0;
    profile.params.power = 61.0;
    let id = profile.id.clone();
    library.add_operation(profile.clone());

    let mut editor = EditorState::new(sample_document());
    editor.select_all();
    let assigned = editor.apply_operation_profile(&profile);
    assert_eq!(assigned, 3);
    assert_eq!(editor.document.operations.len(), 1);
    assert_eq!(editor.document.operations[0].params.speed, 1234.0);

    library.remove_operation(&id).unwrap();
    assert!(library.operation(&id).is_none());
    assert_eq!(editor.document.operations[0].params.power, 61.0);
}

#[test]
fn a_corrupt_project_recovers_its_previous_version() {
    let path = temporary("bw_workflow_recovery.bwp");
    let backup = beamwright::document::serialization::backup_path(&path);
    let _ = fs::remove_file(&path);
    let _ = fs::remove_file(&backup);
    let document = sample_document();
    beamwright::document::serialization::save_to_file(&document, &path).unwrap();
    beamwright::document::serialization::save_to_file(&document, &path).unwrap();
    fs::write(&path, "{not json").unwrap();
    let (recovered, note) = beamwright::document::serialization::load_with_recovery(&path).unwrap();
    assert_eq!(recovered.objects.len(), document.objects.len());
    assert!(note.is_some());
    let _ = fs::remove_file(&path);
    let _ = fs::remove_file(&backup);
}

#[test]
fn raster_import_can_be_exported_as_a_job() {
    let path = temporary("bw_workflow_raster.png");
    let image = image::GrayImage::from_pixel(4, 2, image::Luma([0]));
    image.save(&path).unwrap();
    let document = beamwright::import::raster::import_raster(&path).unwrap();
    assert_eq!(document.objects.len(), 1);
    let (_, report) = render_job(&document, &JobOptions::default()).unwrap();
    assert_eq!(report.paths, 1);
    assert!(report.seconds > 0.0);
    let _ = fs::remove_file(&path);
}

#[test]
fn preview_playback_follows_the_planned_job() {
    let document = sample_document();
    let options = JobOptions::default();
    let (toolpaths, warnings) = planned_toolpaths(&document, &options);
    assert_eq!(warnings.len(), 0);
    let playback = Playback::from_toolpaths(&toolpaths, options.travel_speed_mm_min);
    assert!(!playback.is_empty());
    assert!(playback.total_length > 0.0);
    assert!(playback.duration_seconds > 0.0);
    let start = playback.position_at(0.0).unwrap();
    let end = playback.position_at(playback.total_length).unwrap();
    let last = toolpaths.last().unwrap().end().unwrap();
    assert!(end.point.distance(last) < 1e-6);
    assert!(start.point.distance(Point2::new(10.0, 10.0)) < 1e-6);
    assert!(playback.length_at_seconds(playback.duration_seconds / 2.0) > 0.0);
}

#[test]
fn saving_a_project_keeps_the_open_document_unchanged() {
    let path = temporary("bw_workflow_save.bwp");
    let _ = fs::remove_file(&path);
    let editor = EditorState::new(sample_document());
    let before = editor.document.clone();
    editor.save(&path).unwrap();
    assert_eq!(editor.document, before);
    let loaded = beamwright::document::serialization::load_from_file(&path).unwrap();
    assert_eq!(loaded, before);
    let _ = fs::remove_file(&path);
}
