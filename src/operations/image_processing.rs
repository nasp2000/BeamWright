pub fn grayscale(pixels: &[u8]) -> Vec<u8> {
    // assume input is already grayscale or take first channel; identity here
    pixels.to_vec()
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub fn brightness(pixels: &[u8], delta: i16) -> Vec<u8> {
    pixels
        .iter()
        .map(|&v| (i16::from(v) + delta).clamp(0, 255) as u8)
        .collect()
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub fn contrast(pixels: &[u8], factor: f64) -> Vec<u8> {
    pixels
        .iter()
        .map(|&v| {
            let v = (f64::from(v) - 128.0) * factor + 128.0;
            v.clamp(0.0, 255.0) as u8
        })
        .collect()
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub fn gamma(pixels: &[u8], g: f64) -> Vec<u8> {
    if g <= 0.0 {
        return pixels.to_vec();
    }
    pixels
        .iter()
        .map(|&v| {
            let n = (f64::from(v) / 255.0).powf(1.0 / g) * 255.0;
            n.clamp(0.0, 255.0) as u8
        })
        .collect()
}

pub fn threshold(pixels: &[u8], t: u8) -> Vec<u8> {
    pixels
        .iter()
        .map(|&v| if v >= t { 255 } else { 0 })
        .collect()
}

pub fn invert(pixels: &[u8]) -> Vec<u8> {
    pixels.iter().map(|&v| 255 - v).collect()
}

pub fn floyd_steinberg(pixels: &[u8], width: usize, height: usize) -> Vec<u8> {
    crate::gcode::raster::floyd_steinberg(pixels, width, height)
}

/// Ordered dithering (Bayer 2x2) for grayscale.
#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_possible_truncation
)]
pub fn ordered_dither(pixels: &[u8], width: usize, height: usize) -> Vec<u8> {
    let bayer: [[u8; 2]; 2] = [[0, 2], [3, 1]];
    let mut out = vec![0u8; pixels.len()];
    for y in 0..height {
        for x in 0..width {
            let idx = y * width + x;
            let v = pixels.get(idx).copied().unwrap_or(0);
            let threshold = (u16::from(bayer[y % 2][x % 2]) * 255 / 4) as u8;
            out[idx] = if v > threshold { 255 } else { 0 };
        }
    }
    out
}

/// Sobel edge detection (grayscale input).
#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_possible_truncation
)]
pub fn sobel(pixels: &[u8], width: usize, height: usize) -> Vec<u8> {
    let mut out = vec![0u8; pixels.len()];
    for y in 1..height.saturating_sub(1) {
        for x in 1..width.saturating_sub(1) {
            let idx = |dx: i32, dy: i32| -> u8 {
                let xx = (x as i32 + dx) as usize;
                let yy = (y as i32 + dy) as usize;
                pixels.get(yy * width + xx).copied().unwrap_or(0)
            };
            let gx = (i32::from(idx(-1, -1)) + 2 * i32::from(idx(-1, 0)) + i32::from(idx(-1, 1)))
                - (i32::from(idx(1, -1)) + 2 * i32::from(idx(1, 0)) + i32::from(idx(1, 1)));
            let gy = (i32::from(idx(-1, -1)) + 2 * i32::from(idx(0, -1)) + i32::from(idx(1, -1)))
                - (i32::from(idx(-1, 1)) + 2 * i32::from(idx(0, 1)) + i32::from(idx(1, 1)));
            let mag = f64::from(gx * gx + gy * gy).sqrt().min(255.0) as u8;
            out[y * width + x] = mag;
        }
    }
    out
}

/// Flip image horizontally.
pub fn flip_horizontal(pixels: &[u8], width: usize, height: usize) -> Vec<u8> {
    let mut out = vec![0u8; pixels.len()];
    for y in 0..height {
        for x in 0..width {
            let src = y * width + (width - 1 - x);
            let dst = y * width + x;
            if src < pixels.len() && dst < out.len() {
                out[dst] = pixels[src];
            }
        }
    }
    out
}

/// Flip image vertically.
pub fn flip_vertical(pixels: &[u8], width: usize, height: usize) -> Vec<u8> {
    let mut out = vec![0u8; pixels.len()];
    for y in 0..height {
        for x in 0..width {
            let src = (height - 1 - y) * width + x;
            let dst = y * width + x;
            if src < pixels.len() && dst < out.len() {
                out[dst] = pixels[src];
            }
        }
    }
    out
}

/// Nearest-neighbor resize.
pub fn resize_nearest(
    pixels: &[u8],
    src_w: usize,
    src_h: usize,
    dst_w: usize,
    dst_h: usize,
) -> Vec<u8> {
    if src_w == 0 || src_h == 0 || dst_w == 0 || dst_h == 0 {
        return Vec::new();
    }
    let mut out = vec![0u8; dst_w * dst_h];
    for y in 0..dst_h {
        for x in 0..dst_w {
            let sx = x * src_w / dst_w;
            let sy = y * src_h / dst_h;
            out[y * dst_w + x] = pixels.get(sy * src_w + sx).copied().unwrap_or(0);
        }
    }
    out
}

/// Compute histogram (256 bins).
pub fn histogram(pixels: &[u8]) -> [u32; 256] {
    let mut h = [0u32; 256];
    for &v in pixels {
        h[v as usize] += 1;
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grayscale_identity() {
        let p = [1u8, 2, 3];
        assert_eq!(grayscale(&p), vec![1, 2, 3]);
    }

    #[test]
    fn brightness_up() {
        let p = [100u8, 200];
        assert_eq!(brightness(&p, 50), vec![150, 250]);
    }

    #[test]
    fn brightness_clamp() {
        let p = [250u8];
        assert_eq!(brightness(&p, 50), vec![255]);
    }

    #[test]
    fn contrast_identity() {
        let p = [0u8, 128, 255];
        let r = contrast(&p, 1.0);
        for (a, b) in p.iter().zip(r.iter()) {
            assert_eq!(a, b);
        }
    }

    #[test]
    fn gamma_one() {
        let p = [0u8, 128, 255];
        let r = gamma(&p, 1.0);
        for (a, b) in p.iter().zip(r.iter()) {
            assert_eq!(a, b);
        }
    }

    #[test]
    fn gamma_invalid() {
        let p = [10u8];
        assert_eq!(gamma(&p, 0.0), vec![10]);
    }

    #[test]
    fn threshold_basic() {
        let p = [0u8, 100, 200, 255];
        assert_eq!(threshold(&p, 128), vec![0, 0, 255, 255]);
    }

    #[test]
    fn invert_basic() {
        let p = [0u8, 255];
        assert_eq!(invert(&p), vec![255, 0]);
    }

    #[test]
    fn floyd_size() {
        let p = vec![128u8; 4 * 4];
        let r = floyd_steinberg(&p, 4, 4);
        assert_eq!(r.len(), 16);
    }

    #[test]
    fn floyd_binary() {
        let p = vec![128u8; 2 * 2];
        let r = floyd_steinberg(&p, 2, 2);
        for v in r {
            assert!(v == 0 || v == 255);
        }
    }

    #[test]
    fn ordered_dither_size() {
        let p = vec![128u8; 4 * 4];
        let r = ordered_dither(&p, 4, 4);
        assert_eq!(r.len(), 16);
    }

    #[test]
    fn sobel_size() {
        let p = vec![128u8; 4 * 4];
        let r = sobel(&p, 4, 4);
        assert_eq!(r.len(), 16);
    }

    #[test]
    fn sobel_empty() {
        let p = vec![];
        let r = sobel(&p, 0, 0);
        assert_eq!(r, [] as [u8; 0]);
    }

    #[test]
    fn flip_horizontal_size() {
        let p = vec![1u8, 2, 3, 4];
        let r = flip_horizontal(&p, 2, 2);
        assert_eq!(r.len(), 4);
        assert_eq!(r[0], 2);
        assert_eq!(r[1], 1);
    }

    #[test]
    fn flip_vertical_size() {
        let p = vec![1u8, 2, 3, 4];
        let r = flip_vertical(&p, 2, 2);
        assert_eq!(r.len(), 4);
        assert_eq!(r[0], 3);
    }

    #[test]
    fn resize_nearest_up() {
        let p = vec![1u8, 2, 3, 4];
        let r = resize_nearest(&p, 2, 2, 4, 4);
        assert_eq!(r.len(), 16);
    }

    #[test]
    fn resize_zero() {
        let p = vec![1u8; 4];
        assert_eq!(resize_nearest(&p, 2, 2, 0, 2), [] as [u8; 0]);
    }

    #[test]
    fn histogram_basic() {
        let p = vec![0u8, 0, 255];
        let h = histogram(&p);
        assert_eq!(h[0], 2);
        assert_eq!(h[255], 1);
    }
}
