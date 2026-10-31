/// Simple shelf-based nesting of rectangles into a bin.
#[derive(Debug, Clone, Copy)]
pub struct RectItem {
    pub w: f64,
    pub h: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placement {
    pub x: f64,
    pub y: f64,
    pub item: usize,
    /// The part had to be turned 90 degrees.
    pub rotated: bool,
}

/// Spacing, margin and rotation rules for nesting.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NestOptions {
    /// Gap between two parts, in millimetres.
    pub spacing: f64,
    /// Free border kept around the available area.
    pub margin: f64,
    /// Allow parts to be turned 90 degrees.
    pub allow_rotation: bool,
}

impl Default for NestOptions {
    fn default() -> Self {
        Self {
            spacing: 1.0,
            margin: 0.0,
            allow_rotation: true,
        }
    }
}

/// Result of nesting: what was placed where, and what did not fit.
#[derive(Debug, Clone, PartialEq)]
pub struct NestResult {
    /// Placements relative to the bottom-left corner of the available area,
    /// already including the margin.
    pub placements: Vec<Placement>,
    /// Indices of the items that could not be placed.
    pub unplaced: Vec<usize>,
}

/// Place rectangles in shelves, largest first, honouring spacing and margins.
///
/// This is a deterministic area estimator, not an optimal solver: it stacks
/// rows of decreasing height and only turns a part when that makes it fit. It
/// never overlaps parts and reports everything that does not fit instead of
/// dropping it silently.
pub fn nest(items: &[RectItem], bin_w: f64, bin_h: f64, options: NestOptions) -> NestResult {
    let margin = options.margin.max(0.0);
    let spacing = options.spacing.max(0.0);
    let usable_w = bin_w - 2.0 * margin;
    let usable_h = bin_h - 2.0 * margin;
    let mut result = NestResult {
        placements: Vec::new(),
        unplaced: Vec::new(),
    };
    if items.is_empty() || usable_w <= 0.0 || usable_h <= 0.0 {
        result.unplaced.extend(0..items.len());
        return result;
    }
    let mut order: Vec<usize> = (0..items.len()).collect();
    order.sort_by(|left, right| {
        let longest = |item: &RectItem| item.w.max(item.h);
        longest(&items[*right])
            .partial_cmp(&longest(&items[*left]))
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(left.cmp(right))
    });

    let mut x = 0.0;
    let mut y = 0.0;
    let mut row_height = 0.0;
    for index in order {
        let item = items[index];
        let mut orientations = vec![(item.w, item.h, false)];
        if options.allow_rotation && (item.w - item.h).abs() > 1e-9 {
            orientations.push((item.h, item.w, true));
        }
        let select = |at_x: f64, at_y: f64| {
            orientations.iter().copied().find(|(width, height, _)| {
                *width <= usable_w - at_x + 1e-9 && *height <= usable_h - at_y + 1e-9
            })
        };
        let mut chosen = select(x, y);
        if chosen.is_none() && x > 0.0 {
            x = 0.0;
            y += row_height + spacing;
            row_height = 0.0;
            chosen = select(x, y);
        }
        let Some((width, height, rotated)) = chosen else {
            result.unplaced.push(index);
            continue;
        };
        result.placements.push(Placement {
            x: margin + x,
            y: margin + y,
            item: index,
            rotated,
        });
        x += width + spacing;
        row_height = row_height.max(height);
    }
    result
}

/// Nest with rotation: try original and rotated (90°) orientation, pick best fit.
pub fn nest_with_rotation(items: &[RectItem], bin_w: f64, bin_h: f64) -> Vec<Placement> {
    // try original
    let p0 = nest_shelf(items, bin_w, bin_h);
    // try rotated items
    let rotated: Vec<RectItem> = items
        .iter()
        .map(|it| RectItem { w: it.h, h: it.w })
        .collect();
    let p1 = nest_shelf(&rotated, bin_w, bin_h);
    if p1.len() > p0.len() { p1 } else { p0 }
}

pub fn nest_shelf(items: &[RectItem], bin_w: f64, bin_h: f64) -> Vec<Placement> {
    let mut placements = Vec::new();
    let mut x = 0.0;
    let mut y = 0.0;
    let mut row_h = 0.0;
    for (i, it) in items.iter().enumerate() {
        if it.w > bin_w || it.h > bin_h {
            continue;
        }
        if x + it.w > bin_w {
            // new row
            x = 0.0;
            y += row_h;
            row_h = 0.0;
        }
        if y + it.h > bin_h {
            break;
        }
        placements.push(Placement {
            x,
            y,
            item: i,
            rotated: false,
        });
        x += it.w;
        row_h = row_h.max(it.h);
    }
    placements
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single() {
        let items = [RectItem { w: 5.0, h: 5.0 }];
        let p = nest_shelf(&items, 10.0, 10.0);
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].x, 0.0);
        assert_eq!(p[0].y, 0.0);
    }

    #[test]
    fn wrap_row() {
        let items = [RectItem { w: 6.0, h: 4.0 }, RectItem { w: 6.0, h: 4.0 }];
        let p = nest_shelf(&items, 10.0, 10.0);
        assert_eq!(p.len(), 2);
        assert_eq!(p[1].y, 4.0);
    }

    #[test]
    fn overflow_bin() {
        let items = [RectItem { w: 20.0, h: 5.0 }];
        let p = nest_shelf(&items, 10.0, 10.0);
        assert_eq!(p.len(), 0);
    }

    #[test]
    fn empty() {
        assert_eq!(nest_shelf(&[], 10.0, 10.0).len(), 0);
    }
}

#[cfg(test)]
mod nest_tests {
    use super::{NestOptions, RectItem, nest};

    fn item(w: f64, h: f64) -> RectItem {
        RectItem { w, h }
    }

    #[test]
    fn spacing_and_margin_are_applied() {
        let items = [item(6.0, 4.0), item(6.0, 4.0)];
        let result = nest(
            &items,
            20.0,
            10.0,
            NestOptions {
                spacing: 1.0,
                margin: 2.0,
                allow_rotation: false,
            },
        );
        assert_eq!(result.placements.len(), 2);
        assert_eq!((result.placements[0].x, result.placements[0].y), (2.0, 2.0));
        assert_eq!((result.placements[1].x, result.placements[1].y), (9.0, 2.0));
        assert_eq!(result.unplaced.len(), 0);
    }

    #[test]
    fn rotation_is_used_when_it_fits() {
        let items = [item(3.0, 9.0)];
        let fixed = nest(
            &items,
            10.0,
            4.0,
            NestOptions {
                spacing: 0.0,
                margin: 0.0,
                allow_rotation: false,
            },
        );
        assert_eq!(fixed.placements.len(), 0);
        assert_eq!(fixed.unplaced, vec![0]);
        let turned = nest(
            &items,
            10.0,
            4.0,
            NestOptions {
                spacing: 0.0,
                margin: 0.0,
                allow_rotation: true,
            },
        );
        assert_eq!(turned.placements.len(), 1);
        assert!(turned.placements[0].rotated);
    }

    #[test]
    fn oversized_parts_are_reported_not_dropped() {
        let items = [item(20.0, 2.0), item(4.0, 4.0)];
        let result = nest(&items, 10.0, 10.0, NestOptions::default());
        assert_eq!(result.unplaced, vec![0]);
        assert_eq!(result.placements.len(), 1);
        assert_eq!(result.placements[0].item, 1);
    }

    #[test]
    fn largest_parts_are_placed_first() {
        let items = [item(2.0, 2.0), item(8.0, 8.0)];
        let result = nest(
            &items,
            10.0,
            10.0,
            NestOptions {
                spacing: 0.0,
                margin: 0.0,
                allow_rotation: false,
            },
        );
        assert_eq!(result.placements[0].item, 1);
        assert_eq!(result.placements[1].item, 0);
        assert_eq!((result.placements[0].x, result.placements[0].y), (0.0, 0.0));
        assert_eq!((result.placements[1].x, result.placements[1].y), (8.0, 0.0));
    }

    #[test]
    fn an_impossible_area_reports_everything() {
        let items = [item(1.0, 1.0)];
        let result = nest(
            &items,
            10.0,
            10.0,
            NestOptions {
                spacing: 0.0,
                margin: 6.0,
                allow_rotation: false,
            },
        );
        assert_eq!(result.unplaced, vec![0]);
        assert_eq!(result.placements.len(), 0);
    }
}

#[cfg(test)]
mod rot_tests {
    use super::*;

    #[test]
    fn rotation_helps() {
        let items = [RectItem { w: 9.0, h: 1.0 }];
        let p = nest_with_rotation(&items, 2.0, 10.0);
        // rotated (1x9) fits in 2-wide bin
        assert_eq!(p.len(), 1);
    }

    #[test]
    fn empty() {
        assert_eq!(nest_with_rotation(&[], 10.0, 10.0).len(), 0);
    }
}
