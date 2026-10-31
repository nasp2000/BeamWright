#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]

//! Material test grid: a matrix of speed and power cells (PLAN.md §18.6).
//!
//! This is preparation only. The grid is ordinary geometry with one outline
//! operation per cell, so the generated job differs only in feed rate and
//! power; nothing here knows about a controller.

use crate::document::{Document, Object, Shape};
use crate::geometry::Transform2;
use crate::operations::{Operation, OperationParams, OperationType};

/// Parameters of a material test grid.
#[derive(Debug, Clone, PartialEq)]
pub struct Options {
    pub columns: usize,
    pub rows: usize,
    pub speed_min: f64,
    pub speed_max: f64,
    pub power_min: f64,
    pub power_max: f64,
    pub cell_width_mm: f64,
    pub cell_height_mm: f64,
    pub gap_mm: f64,
    pub margin_mm: f64,
    pub passes: u32,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            columns: 5,
            rows: 4,
            speed_min: 600.0,
            speed_max: 3000.0,
            power_min: 10.0,
            power_max: 80.0,
            cell_width_mm: 20.0,
            cell_height_mm: 12.0,
            gap_mm: 3.0,
            margin_mm: 5.0,
            passes: 1,
        }
    }
}

impl Options {
    pub fn validate(&self) -> Result<(), String> {
        if !(1..=20).contains(&self.columns) || !(1..=20).contains(&self.rows) {
            return Err("columns and rows must be between 1 and 20".to_string());
        }
        if self.speed_min <= 0.0 || self.speed_max < self.speed_min {
            return Err("speed range must be positive and increasing".to_string());
        }
        if !(0.0..=100.0).contains(&self.power_min)
            || !(0.0..=100.0).contains(&self.power_max)
            || self.power_max < self.power_min
        {
            return Err("power range must be within 0 and 100 %".to_string());
        }
        if self.cell_width_mm <= 0.0
            || self.cell_height_mm <= 0.0
            || self.gap_mm < 0.0
            || self.margin_mm < 0.0
        {
            return Err("cell sizes, gap and margin must be positive".to_string());
        }
        if self.passes == 0 {
            return Err("passes must be at least one".to_string());
        }
        Ok(())
    }

    pub fn width_mm(&self) -> f64 {
        self.margin_mm * 2.0
            + self.columns as f64 * self.cell_width_mm
            + (self.columns.saturating_sub(1)) as f64 * self.gap_mm
    }

    pub fn height_mm(&self) -> f64 {
        self.margin_mm * 2.0
            + self.rows as f64 * self.cell_height_mm
            + (self.rows.saturating_sub(1)) as f64 * self.gap_mm
    }
}

fn interpolate(min: f64, max: f64, index: usize, count: usize) -> f64 {
    if count <= 1 {
        return min;
    }
    min + (max - min) * index as f64 / (count - 1) as f64
}

/// Add the grid to `document` and return the number of cells.
pub fn generate(document: &mut Document, options: &Options) -> Result<usize, String> {
    options.validate()?;
    let layer_id = document.add_layer("Material test").id.clone();
    let mut cells = 0;
    for row in 0..options.rows {
        let power = interpolate(options.power_min, options.power_max, row, options.rows);
        for column in 0..options.columns {
            let speed = interpolate(
                options.speed_min,
                options.speed_max,
                column,
                options.columns,
            );
            let mut operation = Operation::new(
                format!("{speed:.0} mm/min / {power:.0} %"),
                OperationType::OutlineCutting,
            );
            operation.params = OperationParams {
                speed,
                power,
                passes: options.passes,
                ..OperationParams::default()
            };
            let operation_id = operation.id.clone();
            document.operations.push(operation);

            let x = options.margin_mm + column as f64 * (options.cell_width_mm + options.gap_mm);
            let y = options.margin_mm + row as f64 * (options.cell_height_mm + options.gap_mm);
            let mut object = Object::new(
                layer_id.clone(),
                Shape::Rect {
                    width: options.cell_width_mm,
                    height: options.cell_height_mm,
                    corner_radius: 0.0,
                },
            );
            object.name = format!("S{speed:.0} P{power:.0}");
            object.transform = Transform2::translate(x, y);
            object.operation_id = Some(operation_id);
            document.add_object(object);
            cells += 1;
        }
    }
    Ok(cells)
}

#[cfg(test)]
mod tests {
    use super::{Options, generate};
    use crate::document::Document;

    #[test]
    fn a_grid_has_one_operation_per_cell() {
        let options = Options {
            columns: 3,
            rows: 2,
            ..Options::default()
        };
        let mut document = Document::default();
        let cells = generate(&mut document, &options).unwrap();
        assert_eq!(cells, 6);
        assert_eq!(document.operations.len(), 6);
        assert_eq!(document.objects.len(), 6);
        assert_eq!(document.layers.len(), 2);
        for object in &document.objects {
            assert!(object.operation_id.is_some());
        }
    }

    #[test]
    fn the_corners_use_the_endpoints_of_the_ranges() {
        let options = Options {
            columns: 3,
            rows: 2,
            speed_min: 500.0,
            speed_max: 1500.0,
            power_min: 10.0,
            power_max: 90.0,
            ..Options::default()
        };
        let mut document = Document::default();
        generate(&mut document, &options).unwrap();
        let speeds: Vec<f64> = document
            .operations
            .iter()
            .map(|operation| operation.params.speed)
            .collect();
        let powers: Vec<f64> = document
            .operations
            .iter()
            .map(|operation| operation.params.power)
            .collect();
        assert!(speeds.contains(&500.0));
        assert!(speeds.contains(&1500.0));
        assert!(speeds.contains(&1000.0));
        assert!(powers.contains(&10.0));
        assert!(powers.contains(&90.0));
    }

    #[test]
    fn invalid_ranges_are_rejected() {
        assert!(Options {
            speed_min: 0.0,
            ..Options::default()
        }
        .validate()
        .is_err());
        assert!(Options {
            power_max: 120.0,
            ..Options::default()
        }
        .validate()
        .is_err());
        assert!(Options {
            columns: 0,
            ..Options::default()
        }
        .validate()
        .is_err());
    }

    #[test]
    fn the_grid_size_includes_margins_and_gaps() {
        let options = Options {
            columns: 2,
            rows: 2,
            cell_width_mm: 10.0,
            cell_height_mm: 5.0,
            gap_mm: 2.0,
            margin_mm: 1.0,
            ..Options::default()
        };
        assert!((options.width_mm() - 24.0).abs() < 1e-9);
        assert!((options.height_mm() - 14.0).abs() < 1e-9);
    }
}
