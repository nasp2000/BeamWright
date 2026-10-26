/// Simple shelf-based nesting of rectangles into a bin.
#[derive(Debug, Clone, Copy)]
pub struct RectItem {
    pub w: f64,
    pub h: f64,
}

#[derive(Debug, Clone, Copy)]
pub struct Placement {
    pub x: f64,
    pub y: f64,
    pub item: usize,
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
        placements.push(Placement { x, y, item: i });
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
        assert!(p.is_empty());
    }

    #[test]
    fn empty() {
        assert!(nest_shelf(&[], 10.0, 10.0).is_empty());
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
