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
}
