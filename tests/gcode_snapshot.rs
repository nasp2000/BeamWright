use beamwright::gcode::dialect::GCodeOptions;
use beamwright::gcode::toolpath::{MoveKind, Toolpath};
use beamwright::gcode::writer::GCodeWriter;

#[test]
fn gcode_snapshot_simple() {
    let mut tp = Toolpath::new(1000.0, 50.0, 1);
    tp.push(MoveKind::Travel, 0.0, 0.0);
    tp.push(MoveKind::Cut, 10.0, 0.0);
    tp.push(MoveKind::Cut, 10.0, 10.0);
    let w = GCodeWriter::new(GCodeOptions::default());
    let s = w.render(&[tp]);
    // Basic sanity: contains expected commands
    assert!(s.contains("G21"));
    assert!(s.contains("G90"));
    assert!(s.contains("M3") || s.contains("M4"));
    assert!(s.contains("G1"));
    // Deterministic: re-render identical
    let mut tp2 = Toolpath::new(1000.0, 50.0, 1);
    tp2.push(MoveKind::Travel, 0.0, 0.0);
    tp2.push(MoveKind::Cut, 10.0, 0.0);
    tp2.push(MoveKind::Cut, 10.0, 10.0);
    let s2 = w.render(&[tp2]);
    assert_eq!(s, s2);
}

#[test]
fn gcode_empty_toolpath() {
    let tp = Toolpath::new(1000.0, 50.0, 1);
    let w = GCodeWriter::new(GCodeOptions::default());
    let s = w.render(&[tp]);
    assert!(s.contains("G21"));
}
