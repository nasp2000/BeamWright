/// Raster engraving helpers (PLAN.md §8.3, §14.4).
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
    let mut buf: Vec<i32> = pixels.iter().map(|&v| i32::from(v)).collect();
    let mut out = vec![0u8; pixels.len()];
    for y in 0..height {
        for x in 0..width {
            let idx = y * width + x;
            let old = buf[idx].clamp(0, 255);
            let new = if old >= 128 { 255 } else { 0 };
            out[idx] = new;
            let err = old - i32::from(new);
            if x + 1 < width {
                buf[idx + 1] += err * 7 / 16;
            }
            if y + 1 < height {
                if x > 0 {
                    buf[idx + width - 1] += err * 3 / 16;
                }
                buf[idx + width] += err * 5 / 16;
                if x + 1 < width {
                    buf[idx + width + 1] += err / 16;
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn threshold_basic() {
        let p = [0u8, 100, 200, 255];
        let r = threshold(&p, 128);
        assert_eq!(r, vec![0, 0, 255, 255]);
    }

    #[test]
    fn threshold_boundary() {
        let p = [127u8, 128];
        let r = threshold(&p, 128);
        assert_eq!(r, vec![0, 255]);
    }

    #[test]
    fn invert_basic() {
        let p = [0u8, 255];
        assert_eq!(invert(&p), vec![255, 0]);
    }

    #[test]
    fn floyd_output_size() {
        let p = vec![128u8; 4 * 4];
        let r = floyd_steinberg(&p, 4, 4);
        assert_eq!(r.len(), 16);
    }

    #[test]
    fn floyd_values_binary() {
        let p = vec![128u8; 2 * 2];
        let r = floyd_steinberg(&p, 2, 2);
        for v in r {
            assert!(v == 0 || v == 255);
        }
    }
}
