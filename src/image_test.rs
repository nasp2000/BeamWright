#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]

//! Image test grid: a speed × power matrix of copies of one prepared image.
//!
//! This is preparation only. Every copy keeps the source path, preparation
//! settings and crop, and receives its own outline operation whose feed rate and
//! power identify the cell. Nothing here knows about a controller or a machine.

use crate::document::{Document, Object, Shape};
use crate::geometry::Transform2;
use crate::operations::{Operation, OperationParams, OperationType};

/// Parameters of an image test grid.
#[derive(Debug, Clone, PartialEq)]
pub struct Options {
    /// Speed varies across the columns.
    pub columns: usize,
    /// Power varies down the rows.
    pub rows: usize,
    pub speed_min: f64,
    pub speed_max: f64,
    pub power_min: f64,
    pub power_max: f64,
    /// Space between neighbouring copies, in millimetres.
    pub gap_mm: f64,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            columns: 4,
            rows: 4,
            speed_min: 600.0,
            speed_max: 3000.0,
            power_min: 10.0,
            power_max: 80.0,
            gap_mm: 3.0,
        }
    }
}

impl Options {
    pub fn validate(&self) -> Result<(), String> {
        if !(1..=20).contains(&self.columns) || !(1..=20).contains(&self.rows) {
            return Err("columns and rows must be between 1 and 20".to_string());
        }
        if !self.speed_min.is_finite()
            || !self.speed_max.is_finite()
            || self.speed_min <= 0.0
            || self.speed_max < self.speed_min
        {
            return Err("speed range must be positive and increasing".to_string());
        }
        if !self.power_min.is_finite()
            || !self.power_max.is_finite()
            || !(0.0..=100.0).contains(&self.power_min)
            || !(0.0..=100.0).contains(&self.power_max)
            || self.power_max < self.power_min
        {
            return Err("power range must be within 0 and 100 %".to_string());
        }
        if !self.gap_mm.is_finite() || self.gap_mm < 0.0 {
            return Err("gap must be finite and non-negative".to_string());
        }
        Ok(())
    }

    pub fn cell_count(&self) -> usize {
        self.columns * self.rows
    }
}

/// Where one copy goes and which reference values it carries.
#[derive(Debug, Clone, PartialEq)]
pub struct Cell {
    pub column: usize,
    pub row: usize,
    pub speed: f64,
    pub power: f64,
    /// World position of the copy's lower-left bounds corner, in millimetres.
    pub x_mm: f64,
    pub y_mm: f64,
}

fn interpolate(min: f64, max: f64, index: usize, count: usize) -> f64 {
    if count <= 1 {
        return min;
    }
    min + (max - min) * index as f64 / (count - 1) as f64
}

/// Small tolerance for comparing page and grid extents.
const EPSILON: f64 = 1e-9;

/// Plan the grid for `base` on a page of the given size.
///
/// The preferred origin is immediately to the right of the source image, at the
/// same bottom edge, separated by the configured gap. If that grid would leave
/// the page or overlap the source image, the grid is tried below the source
/// instead (same left edge). When neither position fits, the whole grid starts
/// at the page origin `(0, 0)`. Speeds rise left to right, powers rise bottom
/// to top, and the extremes of both ranges are always used.
pub fn plan(
    base: &Object,
    page_width_mm: f64,
    page_height_mm: f64,
    options: &Options,
) -> Result<Vec<Cell>, String> {
    options.validate()?;
    if !page_width_mm.is_finite() || page_width_mm <= 0.0 {
        return Err("the page width must be positive".to_string());
    }
    if !page_height_mm.is_finite() || page_height_mm <= 0.0 {
        return Err("the page height must be positive".to_string());
    }
    base.validate()
        .map_err(|error| format!("invalid image object: {error}"))?;
    if !matches!(base.shape, Shape::Image(_)) {
        return Err("the base object is not an image".to_string());
    }
    let (min, max) = base
        .bounds()
        .ok_or_else(|| "the image has no bounds".to_string())?;
    let cell_width = max.x - min.x;
    let cell_height = max.y - min.y;
    if !cell_width.is_finite()
        || cell_width <= 0.0
        || !cell_height.is_finite()
        || cell_height <= 0.0
    {
        return Err("the image has no positive size".to_string());
    }

    let columns = options.columns as f64;
    let rows = options.rows as f64;
    let grid_width = columns * cell_width + (columns - 1.0) * options.gap_mm;
    let grid_height = rows * cell_height + (rows - 1.0) * options.gap_mm;

    let preferred_x = max.x + options.gap_mm;
    let preferred_y = min.y;
    let below_x = min.x;
    let below_y = max.y + options.gap_mm;
    let fits = |origin_x: f64, origin_y: f64| {
        origin_x + grid_width <= page_width_mm + EPSILON
            && origin_y + grid_height <= page_height_mm + EPSILON
            && !(origin_x < max.x - EPSILON
                && origin_x + grid_width > min.x + EPSILON
                && origin_y < max.y - EPSILON
                && origin_y + grid_height > min.y + EPSILON)
    };
    let (origin_x, origin_y) = if fits(preferred_x, preferred_y) {
        (preferred_x, preferred_y)
    } else if fits(below_x, below_y) {
        (below_x, below_y)
    } else {
        (0.0, 0.0)
    };

    let mut cells = Vec::with_capacity(options.cell_count());
    for row in 0..options.rows {
        let power = interpolate(options.power_min, options.power_max, row, options.rows);
        for column in 0..options.columns {
            let speed = interpolate(
                options.speed_min,
                options.speed_max,
                column,
                options.columns,
            );
            cells.push(Cell {
                column,
                row,
                speed,
                power,
                x_mm: origin_x + column as f64 * (cell_width + options.gap_mm),
                y_mm: origin_y + row as f64 * (cell_height + options.gap_mm),
            });
        }
    }
    Ok(cells)
}

/// Clone the image once per cell, each with its own outline operation.
///
/// Returns the number of copies. The copies go on a new "Image test" layer and
/// keep the source path, preparation settings and crop. The caller records the
/// undo step once, so the whole grid is a single action.
pub fn generate(
    document: &mut Document,
    base_id: &str,
    options: &Options,
) -> Result<usize, String> {
    options.validate()?;
    let base = document
        .object(base_id)
        .cloned()
        .ok_or_else(|| format!("unknown image object: {base_id}"))?;
    let cells = plan(&base, document.width_mm, document.height_mm, options)?;
    let (base_min, _) = base
        .bounds()
        .ok_or_else(|| "the image has no bounds".to_string())?;

    let layer_id = document.add_layer("Image test").id.clone();
    let mut copies = 0;
    for cell in cells {
        let name = format!("S{:.0} P{:.0}", cell.speed, cell.power);
        let mut operation = Operation::new(&name, OperationType::OutlineCutting);
        operation.params = OperationParams {
            speed: cell.speed,
            power: cell.power,
            ..OperationParams::default()
        };
        let operation_id = operation.id.clone();
        document.operations.push(operation);

        let mut object = base.clone();
        object.id = uuid::Uuid::new_v4().to_string();
        object.name = name;
        object.layer_id.clone_from(&layer_id);
        object.group_id = None;
        object.visible = true;
        object.locked = false;
        object.transform = Transform2::translate(cell.x_mm - base_min.x, cell.y_mm - base_min.y)
            .compose(&base.transform);
        object.operation_id = Some(operation_id);
        document.add_object(object);
        copies += 1;
    }
    Ok(copies)
}

#[cfg(test)]
mod tests {
    use super::{Options, generate, plan};
    use crate::document::{Document, Object, Shape};
    use crate::geometry::Transform2;
    use crate::objects::{ImageCrop, ImageObject};
    use crate::operations::OperationType;

    fn base_at(x: f64, y: f64) -> Object {
        // 100 px at 254 dpi is exactly 10 mm wide and 5 mm high.
        let image = ImageObject::new("photo.png", 100, 50, 254.0);
        let mut object = Object::new("layer", Shape::Image(image));
        object.transform = Transform2::translate(x, y);
        object
    }

    #[test]
    fn a_grid_has_one_cell_per_combination() {
        let options = Options {
            columns: 3,
            rows: 2,
            ..Options::default()
        };
        let cells = plan(&base_at(0.0, 0.0), 300.0, 200.0, &options).unwrap();
        assert_eq!(cells.len(), 6);
        assert_eq!(options.cell_count(), 6);
        for (index, cell) in cells.iter().enumerate() {
            assert_eq!(cell.column, index % 3);
            assert_eq!(cell.row, index / 3);
        }
    }

    #[test]
    fn the_ranges_use_their_endpoints() {
        let options = Options {
            columns: 3,
            rows: 2,
            speed_min: 500.0,
            speed_max: 1500.0,
            power_min: 10.0,
            power_max: 90.0,
            ..Options::default()
        };
        let cells = plan(&base_at(0.0, 0.0), 300.0, 200.0, &options).unwrap();
        let speeds: Vec<f64> = cells.iter().map(|cell| cell.speed).collect();
        let powers: Vec<f64> = cells.iter().map(|cell| cell.power).collect();
        assert!(speeds.contains(&500.0));
        assert!(speeds.contains(&1000.0));
        assert!(speeds.contains(&1500.0));
        assert!(powers.contains(&10.0));
        assert!(powers.contains(&90.0));
        // Speed varies along a row, power down a column.
        assert_eq!(cells[0].speed, 500.0);
        assert_eq!(cells[2].speed, 1500.0);
        assert_eq!(cells[0].power, cells[1].power);
        assert_eq!(cells[0].power, 10.0);
        assert_eq!(cells[3].power, 90.0);
    }

    #[test]
    fn the_grid_is_placed_to_the_right_of_the_source() {
        let options = Options {
            columns: 3,
            rows: 2,
            gap_mm: 2.0,
            ..Options::default()
        };
        let cells = plan(&base_at(10.0, 20.0), 300.0, 200.0, &options).unwrap();
        assert!((cells[0].x_mm - 22.0).abs() < 1e-9);
        assert!((cells[0].y_mm - 20.0).abs() < 1e-9);
        assert!((cells[1].x_mm - 34.0).abs() < 1e-9);
        assert!((cells[2].x_mm - 46.0).abs() < 1e-9);
        assert!((cells[3].y_mm - 27.0).abs() < 1e-9);
        // The first copy starts one gap to the right of the source's right edge.
        assert!((cells[0].x_mm - (10.0 + 10.0 + 2.0)).abs() < 1e-9);
    }

    #[test]
    fn a_grid_that_does_not_fit_on_the_right_is_placed_below() {
        let options = Options {
            columns: 1,
            rows: 2,
            gap_mm: 2.0,
            ..Options::default()
        };
        // Source at x = 285..295: the 10 mm grid would end at 307 mm on the
        // right, but it fits below the source.
        let cells = plan(&base_at(285.0, 20.0), 300.0, 200.0, &options).unwrap();
        assert!((cells[0].x_mm - 285.0).abs() < 1e-9);
        assert!((cells[0].y_mm - 27.0).abs() < 1e-9);
        assert!((cells[1].y_mm - 34.0).abs() < 1e-9);
    }

    #[test]
    fn a_grid_that_does_not_fit_starts_at_the_page_origin() {
        let options = Options {
            columns: 3,
            rows: 2,
            gap_mm: 2.0,
            ..Options::default()
        };
        // Preferred x is 282 mm, but the 34 mm grid would end at 316 mm.
        let cells = plan(&base_at(270.0, 20.0), 300.0, 200.0, &options).unwrap();
        assert!(cells[0].x_mm.abs() < 1e-9);
        assert!(cells[0].y_mm.abs() < 1e-9);

        // A grid taller than the remaining page also falls back to the origin.
        let tall = Options {
            columns: 1,
            rows: 2,
            gap_mm: 2.0,
            ..Options::default()
        };
        let cells = plan(&base_at(10.0, 195.0), 300.0, 200.0, &tall).unwrap();
        assert!(cells[0].x_mm.abs() < 1e-9);
        assert!(cells[0].y_mm.abs() < 1e-9);
    }

    #[test]
    fn invalid_options_and_non_image_bases_are_rejected() {
        let base = base_at(0.0, 0.0);
        assert!(
            plan(
                &base,
                300.0,
                200.0,
                &Options {
                    columns: 0,
                    ..Options::default()
                }
            )
            .is_err()
        );
        assert!(
            plan(
                &base,
                300.0,
                200.0,
                &Options {
                    speed_min: 0.0,
                    ..Options::default()
                }
            )
            .is_err()
        );
        assert!(
            plan(
                &base,
                300.0,
                200.0,
                &Options {
                    power_max: 120.0,
                    ..Options::default()
                }
            )
            .is_err()
        );
        assert!(
            plan(
                &base,
                300.0,
                200.0,
                &Options {
                    gap_mm: -1.0,
                    ..Options::default()
                }
            )
            .is_err()
        );
        assert!(plan(&base, 0.0, 200.0, &Options::default()).is_err());
        let rectangle = Object::new(
            "layer",
            Shape::Rect {
                width: 10.0,
                height: 5.0,
                corner_radius: 0.0,
            },
        );
        assert!(plan(&rectangle, 300.0, 200.0, &Options::default()).is_err());
    }

    #[test]
    fn generate_creates_one_image_and_operation_per_cell() {
        let mut document = Document::default();
        let layer = document.layers[0].id.clone();
        let mut base = base_at(10.0, 20.0);
        base.layer_id = layer;
        base.name = "Source photo".to_string();
        let Shape::Image(image) = &mut base.shape else {
            unreachable!();
        };
        image.crop = Some(ImageCrop {
            x: 0,
            y: 0,
            width: 50,
            height: 25,
        });
        image.settings.shadow_lift = 0.25;
        let base_id = base.id.clone();
        document.add_object(base);

        let options = Options {
            columns: 3,
            rows: 2,
            gap_mm: 2.0,
            ..Options::default()
        };
        let cells = plan(
            document.object(&base_id).unwrap(),
            document.width_mm,
            document.height_mm,
            &options,
        )
        .unwrap();
        let copies = generate(&mut document, &base_id, &options).unwrap();
        assert_eq!(copies, 6);
        assert_eq!(document.objects.len(), 7);
        assert_eq!(document.operations.len(), 6);
        assert_eq!(document.layers.len(), 2);
        assert_eq!(document.layers[1].name, "Image test");

        let source = document.object(&base_id).unwrap();
        let Shape::Image(source_image) = &source.shape else {
            unreachable!();
        };
        let mut operation_ids = Vec::new();
        for (index, object) in document.objects.iter().skip(1).enumerate() {
            assert_eq!(object.layer_id, document.layers[1].id);
            assert!(object.visible);
            assert!(!object.locked);
            assert!(object.group_id.is_none());
            assert_eq!(
                object.name,
                format!("S{:.0} P{:.0}", cells[index].speed, cells[index].power)
            );
            assert!(object.operation_id.is_some());
            let Shape::Image(copy) = &object.shape else {
                panic!("the copy is not an image");
            };
            assert_eq!(copy.path, source_image.path);
            assert_eq!(copy.settings, source_image.settings);
            assert_eq!(copy.crop, source_image.crop);
            let bounds = object.bounds().unwrap();
            assert!((bounds.0.x - cells[index].x_mm).abs() < 1e-9);
            assert!((bounds.0.y - cells[index].y_mm).abs() < 1e-9);
            let operation_id = object.operation_id.clone().unwrap();
            assert!(!operation_ids.contains(&operation_id));
            operation_ids.push(operation_id);
        }
        for operation in &document.operations {
            assert_eq!(operation.op_type, OperationType::OutlineCutting);
            assert!(operation.params.speed > 0.0);
            assert!((0.0..=100.0).contains(&operation.params.power));
        }
        assert!(document.validate().is_ok());
    }

    #[test]
    fn generate_rejects_an_unknown_source() {
        let mut document = Document::default();
        assert!(generate(&mut document, "missing", &Options::default()).is_err());
        assert_eq!(document.objects.len(), 0);
        assert_eq!(document.layers.len(), 1);
    }
}
