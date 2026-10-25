/// Simple bitmap tracing helpers (approximates potrace-style contour extraction).
#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_possible_truncation
)]
pub fn boundary_points(grid: &[u8], width: usize, height: usize) -> Vec<(f64, f64)> {
    let mut out = Vec::new();
    if width == 0 || height == 0 {
        return out;
    }
    for y in 0..height {
        for x in 0..width {
            let idx = y * width + x;
            if grid.get(idx).copied().unwrap_or(0) == 0 {
                continue;
            }
            // boundary if any 4-neighbor is background/out of bounds
            let mut is_boundary = false;
            for (dx, dy) in [(-1i32, 0i32), (1, 0), (0, -1), (0, 1)] {
                let nx = x as i32 + dx;
                let ny = y as i32 + dy;
                if nx < 0 || ny < 0 || nx >= width as i32 || ny >= height as i32 {
                    is_boundary = true;
                    break;
                }
                if grid
                    .get((ny as usize) * width + (nx as usize))
                    .copied()
                    .unwrap_or(0)
                    == 0
                {
                    is_boundary = true;
                    break;
                }
            }
            if is_boundary {
                out.push((x as f64, y as f64));
            }
        }
    }
    out
}

/// Trace a single contour as the convex hull of boundary points.
pub fn trace_contour(grid: &[u8], width: usize, height: usize) -> Vec<(f64, f64)> {
    let pts = boundary_points(grid, width, height);
    if pts.is_empty() {
        return Vec::new();
    }
    crate::geometry::boolean::convex_hull(&pts)
}

/// Count foreground pixels.
pub fn foreground_count(grid: &[u8]) -> usize {
    grid.iter().filter(|&&v| v != 0).count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_grid() {
        let g = vec![0u8; 4 * 4];
        assert_eq!(boundary_points(&g, 4, 4), [] as [(f64, f64); 0]);
        assert_eq!(trace_contour(&g, 4, 4), [] as [(f64, f64); 0]);
        assert_eq!(foreground_count(&g), 0);
    }

    #[test]
    fn solid_square() {
        let mut g = vec![0u8; 6 * 6];
        for y in 1..5 {
            for x in 1..5 {
                g[y * 6 + x] = 255;
            }
        }
        let bp = boundary_points(&g, 6, 6);
        assert_ne!(bp, [] as [(f64, f64); 0]);
        let c = trace_contour(&g, 6, 6);
        assert!(c.len() >= 3);
        assert!(foreground_count(&g) > 0);
    }

    #[test]
    fn single_pixel() {
        let mut g = vec![0u8; 3 * 3];
        g[4] = 255;
        let bp = boundary_points(&g, 3, 3);
        assert_eq!(bp.len(), 1);
        let c = trace_contour(&g, 3, 3);
        assert_ne!(c, [] as [(f64, f64); 0]);
    }
}
