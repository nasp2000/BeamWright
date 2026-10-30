use beamwright::document::shape::Shape;
use beamwright::document::{Document, Object, serialization};
use beamwright::editor::EditorState;
use beamwright::export::svg::export_svg;
use beamwright::geometry::{Point2, Transform2};
use beamwright::import::svg::import_svg;
use std::env::temp_dir;

#[test]
fn document_round_trip() {
    let document = Document::new("Round trip", 120.0, 80.0);
    let path = temp_dir().join("beamwright_integration_roundtrip.bwp");
    serialization::save_to_file(&document, &path).unwrap();
    let loaded = serialization::load_from_file(&path).unwrap();
    assert_eq!(loaded.name, document.name);
    assert_eq!(loaded.width_mm, 120.0);
    assert_eq!(loaded.height_mm, 80.0);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn svg_export_contains_real_geometry() {
    let mut document = Document::default();
    let layer_id = document.layers[0].id.clone();
    document.add_object(
        Object::new(
            layer_id,
            Shape::Rect {
                width: 10.0,
                height: 5.0,
                corner_radius: 0.0,
            },
        )
        .with_transform(Transform2::translate(2.0, 3.0)),
    );
    let svg = export_svg(&document).unwrap();
    assert!(svg.contains("<svg"));
    assert!(svg.contains("M2 197"));
    assert!(svg.contains("L12 197"));
}

#[test]
fn svg_round_trip_preserves_object_count_and_bounds() {
    let mut document = Document::default();
    let layer_id = document.layers[0].id.clone();
    document.add_object(
        Object::new(
            layer_id.clone(),
            Shape::Rect {
                width: 10.0,
                height: 5.0,
                corner_radius: 0.0,
            },
        )
        .with_transform(Transform2::translate(2.0, 3.0)),
    );
    document.add_object(Object::new(
        layer_id.clone(),
        Shape::Ellipse { rx: 4.0, ry: 2.0 },
    ));
    let svg = export_svg(&document).unwrap();
    let imported = import_svg(&svg).unwrap();
    assert_eq!(imported.objects.len(), 2);
    let (rect_min, rect_max) = imported.objects[0].bounds().unwrap();
    assert!((rect_min.x - 2.0).abs() < 1e-6);
    assert!((rect_min.y - 3.0).abs() < 1e-6);
    assert!((rect_max.x - 12.0).abs() < 1e-6);
    assert!((rect_max.y - 8.0).abs() < 1e-6);
}

#[test]
fn drawing_select_move_save_open_acceptance() {
    let mut editor = EditorState::new(Document::new("Acceptance", 200.0, 120.0));
    editor.add_rectangle(Point2::new(10.0, 10.0), Point2::new(40.0, 30.0));
    editor.add_line(Point2::new(50.0, 10.0), Point2::new(90.0, 40.0));
    editor.add_ellipse(Point2::new(100.0, 20.0), Point2::new(140.0, 60.0));
    assert_eq!(editor.document.objects.len(), 3);

    editor.selection.clear();
    let hit = editor.object_at(Point2::new(10.0, 20.0), 0.5);
    assert!(hit.is_some());
    editor.select_at(Point2::new(10.0, 20.0), false, 0.5);
    editor.record();
    editor.move_selection(5.0, 5.0);

    let path = temp_dir().join("beamwright_acceptance.bwp");
    editor.save(&path).unwrap();
    let mut reopened = EditorState::new(Document::default());
    reopened.load(&path).unwrap();
    assert_eq!(reopened.document.objects.len(), 3);
    let (min, _) = reopened.document.objects[0].bounds().unwrap();
    assert!((min.x - 15.0).abs() < 1e-6);
    assert!((min.y - 15.0).abs() < 1e-6);
    let _ = std::fs::remove_file(&path);
}
