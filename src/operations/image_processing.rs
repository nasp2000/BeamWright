pub fn grayscale(pixels: &[u8]) -> Vec<u8> {
    // assume input is already grayscale or take first channel; identity here
    pixels.to_vec()
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
