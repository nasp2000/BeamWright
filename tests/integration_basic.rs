use beamwright::document::{Document, serialization};
use beamwright::export::svg::export_svg;
use beamwright::gcode::dialect::GCodeOptions;
use beamwright::gcode::toolpath::{MoveKind, Toolpath};
use beamwright::gcode::writer::GCodeWriter;
use std::env::temp_dir;

#[test]
fn document_round_trip() {
    let d = Document::default();
    let path = temp_dir().join("bw_integ.bwp");
    serialization::save_to_file(&d, &path).unwrap();
    let loaded = serialization::load_from_file(&path).unwrap();
    assert_eq!(loaded.name, d.name);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn svg_export_nonempty() {
    let d = Document::default();
    let s = export_svg(&d).unwrap();
    assert!(s.contains("<svg"));
}

#[test]
fn gcode_write() {
    let mut tp = Toolpath::new(1000.0, 50.0, 1);
    tp.push(MoveKind::Cut, 10.0, 0.0);
    let w = GCodeWriter::new(GCodeOptions::default());
    let s = w.render(&[tp]);
    assert!(s.contains("G1"));
}
