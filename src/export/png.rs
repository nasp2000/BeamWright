use crate::errors::Result;
use std::path::Path;

/// Write a grayscale/RGBA pixel buffer to a PNG file.
pub fn write_png_rgba(path: &Path, width: u32, height: u32, rgba: &[u8]) -> Result<()> {
    let img = image::RgbaImage::from_raw(width, height, rgba.to_vec()).ok_or_else(|| {
        crate::errors::BeamError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "invalid buffer size",
        ))
    })?;
    img.save(path)
        .map_err(|e| crate::errors::BeamError::Io(std::io::Error::other(e.to_string())))?;
    Ok(())
}

/// Write a grayscale buffer to PNG.
pub fn write_png_gray(path: &Path, width: u32, height: u32, gray: &[u8]) -> Result<()> {
    let img = image::GrayImage::from_raw(width, height, gray.to_vec()).ok_or_else(|| {
        crate::errors::BeamError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "invalid buffer size",
        ))
    })?;
    img.save(path)
        .map_err(|e| crate::errors::BeamError::Io(std::io::Error::other(e.to_string())))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env::temp_dir;

    #[test]
    fn write_rgba() {
        let path = temp_dir().join("bw_test.png");
        let buf = vec![255u8; 4 * 4 * 4];
        write_png_rgba(&path, 4, 4, &buf).unwrap();
        assert!(path.exists());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn write_gray() {
        let path = temp_dir().join("bw_testg.png");
        let buf = vec![128u8; 4 * 4];
        write_png_gray(&path, 4, 4, &buf).unwrap();
        assert!(path.exists());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn invalid_size() {
        let path = temp_dir().join("bw_bad.png");
        let buf = vec![0u8; 10];
        assert!(write_png_rgba(&path, 4, 4, &buf).is_err());
    }
}
