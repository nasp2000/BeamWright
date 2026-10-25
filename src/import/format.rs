use crate::document::Document;
use crate::errors::Result;
use std::path::Path;

pub fn detect_format(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_lowercase)
        .as_deref()
    {
        Some("svg") => "svg",
        Some("dxf") => "dxf",
        Some("nc" | "gcode") => "gcode",
        Some("png" | "jpg" | "jpeg" | "bmp" | "tiff") => "raster",
        Some("hp" | "hpgl" | "plt") => "hpgl",
        Some("pdf") => "pdf",
        _ => "unknown",
    }
}

pub fn import_by_extension(path: &Path) -> Result<Document> {
    let fmt = detect_format(path);
    match fmt {
        "svg" => {
            let src = std::fs::read_to_string(path)?;
            crate::import::svg::import_svg(&src)
        }
        "dxf" => {
            let src = std::fs::read_to_string(path)?;
            crate::import::dxf::import_dxf(&src)
        }
        "gcode" => {
            let src = std::fs::read_to_string(path)?;
            crate::import::gcode::import_gcode(&src)
        }
        "hpgl" => {
            let src = std::fs::read_to_string(path)?;
            crate::import::hpgl::import_hpgl(&src)
        }
        "raster" => crate::import::raster::import_raster(path),
        "pdf" => crate::import::pdf::import_pdf(path),
        _ => Err(crate::errors::BeamError::InvalidInput(
            "unsupported format".into(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env::temp_dir;

    #[test]
    fn detect_svg() {
        assert_eq!(detect_format(Path::new("a.svg")), "svg");
    }

    #[test]
    fn detect_dxf() {
        assert_eq!(detect_format(Path::new("a.DXF")), "dxf");
    }

    #[test]
    fn detect_unknown() {
        assert_eq!(detect_format(Path::new("a.xyz")), "unknown");
    }

    #[test]
    fn import_svg_by_ext() {
        let dir = temp_dir();
        let p = dir.join("bw_fmt.svg");
        std::fs::write(&p, "<svg width=\"10\" height=\"10\"><rect x=\"0\" y=\"0\" width=\"1\" height=\"1\"/></svg>").unwrap();
        let d = import_by_extension(&p).unwrap();
        assert_eq!(d.objects.len(), 1);
        let _ = std::fs::remove_file(&p);
    }
}
