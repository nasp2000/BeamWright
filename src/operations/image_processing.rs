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
}
