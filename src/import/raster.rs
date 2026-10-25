use crate::document::Document;
use crate::document::model::{DocumentObject, ObjectKind};
use crate::errors::Result;
use std::path::Path;

pub fn import_raster(path: &Path) -> Result<Document> {
    let img = image::open(path)
        .map_err(|e| crate::errors::BeamError::Io(std::io::Error::other(e.to_string())))?;
    let (w, h) = (img.width(), img.height());
    let mut doc = Document {
        width_mm: f64::from(w),
        height_mm: f64::from(h),
        ..Document::default()
    };
    let layer_id = doc.layers[0].id.clone();
    let lid = if layer_id.is_empty() {
        "layer-1".to_string()
    } else {
        layer_id
    };
    if doc.layers[0].id.is_empty() {
        doc.layers[0].id.clone_from(&lid);
    }
    doc.objects.push(DocumentObject {
        id: "img-0".to_string(),
        kind: ObjectKind::Image,
        layer_id: lid,
    });
    Ok(doc)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env::temp_dir;

    #[test]
    fn missing_file_errors() {
        let p = temp_dir().join("bw_no_img.png");
        let _ = std::fs::remove_file(&p);
        assert!(import_raster(&p).is_err());
    }

    #[test]
    fn imports_png() {
        let p = temp_dir().join("bw_test_img.png");
        let img = image::RgbaImage::from_pixel(4, 3, image::Rgba([255, 0, 0, 255]));
        img.save(&p).unwrap();
        let doc = import_raster(&p).unwrap();
        assert_eq!(doc.width_mm, 4.0);
        assert_eq!(doc.height_mm, 3.0);
        assert_eq!(doc.objects.len(), 1);
        let _ = std::fs::remove_file(&p);
    }
}
