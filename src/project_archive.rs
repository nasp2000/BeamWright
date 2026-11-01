//! Project archives: a `.bwpz` file with the project and its images inside.
//!
//! The archive is a ZIP written with the "store" method (no compression), so it
//! needs no third-party crate and stays readable by any unzip tool. Images are
//! copied into `images/` and the project paths are rewritten to point there;
//! opening an archive extracts those images next to it.

use std::path::{Path, PathBuf};

use crate::document::{Document, Shape};

/// Result of writing an archive.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ArchiveReport {
    pub images: usize,
    /// Images that were referenced but could not be read; they stay as paths.
    pub missing: Vec<String>,
}

const PROJECT_ENTRY: &str = "project.bwp";

/// Write `document` and its images into a `.bwpz` archive.
pub fn write_archive(document: &Document, path: &Path) -> Result<ArchiveReport, String> {
    let mut copy = document.clone();
    let mut entries: Vec<(String, Vec<u8>)> = Vec::new();
    let mut report = ArchiveReport::default();
    for object in &mut copy.objects {
        let Shape::Image(image) = &mut object.shape else {
            continue;
        };
        let source = Path::new(&image.path);
        let Ok(data) = std::fs::read(source) else {
            report.missing.push(image.path.clone());
            continue;
        };
        let name = format!(
            "images/{:03}-{}",
            entries.len(),
            sanitize(
                source
                    .file_name()
                    .and_then(|value| value.to_str())
                    .unwrap_or("image")
            )
        );
        entries.push((name.clone(), data));
        image.path = name;
        report.images += 1;
    }
    let json = crate::document::serialization::to_json(&copy).map_err(|error| error.to_string())?;
    entries.insert(0, (PROJECT_ENTRY.to_string(), json.into_bytes()));
    let zip = build_zip(&entries);
    std::fs::write(path, zip).map_err(|error| format!("cannot write the archive: {error}"))?;
    Ok(report)
}

/// Read a `.bwpz` archive, extracting its images into `destination`.
pub fn read_archive(path: &Path, destination: &Path) -> Result<Document, String> {
    let bytes = std::fs::read(path).map_err(|error| format!("cannot read the archive: {error}"))?;
    let entries = read_zip(&bytes)?;
    let project = entries
        .iter()
        .find(|(name, _)| name == PROJECT_ENTRY)
        .ok_or_else(|| format!("the archive has no {PROJECT_ENTRY}"))?;
    let text = std::str::from_utf8(&project.1)
        .map_err(|_| "the project inside the archive is not text".to_string())?;
    let mut document =
        crate::document::serialization::from_json(text).map_err(|error| error.to_string())?;
    if document.objects.iter().any(
        |object| matches!(&object.shape, Shape::Image(image) if image.path.starts_with("images/")),
    ) {
        std::fs::create_dir_all(destination)
            .map_err(|error| format!("cannot create {}: {error}", destination.display()))?;
    }
    for object in &mut document.objects {
        let Shape::Image(image) = &mut object.shape else {
            continue;
        };
        let Some((name, data)) = entries.iter().find(|(name, _)| *name == image.path) else {
            continue;
        };
        let file_name = Path::new(name)
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("image");
        let target = destination.join(file_name);
        if !target.exists() {
            std::fs::write(&target, data)
                .map_err(|error| format!("cannot extract {}: {error}", target.display()))?;
        }
        image.path = target.to_string_lossy().to_string();
    }
    Ok(document)
}

/// Where an archive's images are extracted: `<stem>_assets` beside the file.
pub fn image_directory(path: &Path) -> PathBuf {
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("project");
    path.with_file_name(format!("{stem}_assets"))
}

/// Keep a file name usable inside the archive.
fn sanitize(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .collect();
    if cleaned.trim_matches('.').is_empty() {
        "image".to_string()
    } else {
        cleaned
    }
}

// ---------------------------------------------------------------- ZIP store

fn crc32(data: &[u8]) -> u32 {
    let mut table = [0u32; 256];
    for (index, entry) in table.iter_mut().enumerate() {
        let mut value = u32::try_from(index).unwrap_or(0);
        for _ in 0..8 {
            value = if value & 1 == 0 {
                value >> 1
            } else {
                0xEDB8_8320 ^ (value >> 1)
            };
        }
        *entry = value;
    }
    let mut crc = 0xFFFF_FFFFu32;
    for byte in data {
        crc = (crc >> 8) ^ table[((crc ^ u32::from(*byte)) & 0xFF) as usize];
    }
    !crc
}

fn push_u16(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn push_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn build_zip(entries: &[(String, Vec<u8>)]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut central = Vec::new();
    for (name, data) in entries {
        let offset = u32::try_from(out.len()).unwrap_or(u32::MAX);
        let crc = crc32(data);
        let size = u32::try_from(data.len()).unwrap_or(u32::MAX);
        let name_len = u16::try_from(name.len()).unwrap_or(u16::MAX);
        push_u32(&mut out, 0x0403_4b50);
        push_u16(&mut out, 20);
        push_u16(&mut out, 0);
        push_u16(&mut out, 0);
        push_u16(&mut out, 0);
        push_u16(&mut out, 0);
        push_u32(&mut out, crc);
        push_u32(&mut out, size);
        push_u32(&mut out, size);
        push_u16(&mut out, name_len);
        push_u16(&mut out, 0);
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(data);

        push_u32(&mut central, 0x0201_4b50);
        push_u16(&mut central, 20);
        push_u16(&mut central, 20);
        push_u16(&mut central, 0);
        push_u16(&mut central, 0);
        push_u16(&mut central, 0);
        push_u16(&mut central, 0);
        push_u32(&mut central, crc);
        push_u32(&mut central, size);
        push_u32(&mut central, size);
        push_u16(&mut central, name_len);
        push_u16(&mut central, 0);
        push_u16(&mut central, 0);
        push_u16(&mut central, 0);
        push_u16(&mut central, 0);
        push_u32(&mut central, 0);
        push_u32(&mut central, offset);
        central.extend_from_slice(name.as_bytes());
    }
    let central_offset = u32::try_from(out.len()).unwrap_or(u32::MAX);
    out.extend_from_slice(&central);
    push_u32(&mut out, 0x0605_4b50);
    push_u16(&mut out, 0);
    push_u16(&mut out, 0);
    push_u16(&mut out, u16::try_from(entries.len()).unwrap_or(u16::MAX));
    push_u16(&mut out, u16::try_from(entries.len()).unwrap_or(u16::MAX));
    push_u32(&mut out, u32::try_from(central.len()).unwrap_or(u32::MAX));
    push_u32(&mut out, central_offset);
    push_u16(&mut out, 0);
    out
}

fn read_u16(data: &[u8], offset: usize) -> Option<u16> {
    Some(u16::from_le_bytes(
        data.get(offset..offset + 2)?.try_into().ok()?,
    ))
}

fn read_u32(data: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        data.get(offset..offset + 4)?.try_into().ok()?,
    ))
}

fn read_zip(data: &[u8]) -> Result<Vec<(String, Vec<u8>)>, String> {
    if data.len() < 22 {
        return Err("the archive is too small to be a ZIP file".to_string());
    }
    let start = data.len().saturating_sub(65_557);
    let eocd = (start..data.len() - 4)
        .rev()
        .find(|offset| read_u32(data, *offset) == Some(0x0605_4b50))
        .ok_or_else(|| "the archive has no ZIP directory".to_string())?;
    let count = read_u16(data, eocd + 10).ok_or("truncated archive")?;
    let central_offset = read_u32(data, eocd + 16).ok_or("truncated archive")? as usize;
    let mut cursor = central_offset;
    let mut entries = Vec::with_capacity(usize::from(count));
    for _ in 0..count {
        if read_u32(data, cursor) != Some(0x0201_4b50) {
            return Err("the archive directory is damaged".to_string());
        }
        let method = read_u16(data, cursor + 10).ok_or("truncated archive")?;
        let crc = read_u32(data, cursor + 16).ok_or("truncated archive")?;
        let size = read_u32(data, cursor + 20).ok_or("truncated archive")? as usize;
        let name_len = read_u16(data, cursor + 28).ok_or("truncated archive")? as usize;
        let extra_len = read_u16(data, cursor + 30).ok_or("truncated archive")? as usize;
        let comment_len = read_u16(data, cursor + 32).ok_or("truncated archive")? as usize;
        let local_offset = read_u32(data, cursor + 42).ok_or("truncated archive")? as usize;
        let name = data
            .get(cursor + 46..cursor + 46 + name_len)
            .and_then(|bytes| std::str::from_utf8(bytes).ok())
            .ok_or("the archive has an invalid entry name")?
            .to_string();
        if method != 0 {
            return Err(format!(
                "the entry '{name}' is compressed; only stored archives are supported"
            ));
        }
        if read_u32(data, local_offset) != Some(0x0403_4b50) {
            return Err(format!("the entry '{name}' is damaged"));
        }
        let local_name = read_u16(data, local_offset + 26).ok_or("truncated archive")? as usize;
        let local_extra = read_u16(data, local_offset + 28).ok_or("truncated archive")? as usize;
        let start = local_offset + 30 + local_name + local_extra;
        let bytes = data
            .get(start..start + size)
            .ok_or_else(|| format!("the entry '{name}' is truncated"))?;
        if crc32(bytes) != crc {
            return Err(format!("the entry '{name}' failed its checksum"));
        }
        entries.push((name, bytes.to_vec()));
        cursor += 46 + name_len + extra_len + comment_len;
    }
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::{build_zip, crc32, read_zip, sanitize};
    use crate::document::Document;
    use crate::document::model::{Object, Shape};

    fn entries() -> Vec<(String, Vec<u8>)> {
        vec![
            ("project.bwp".to_string(), b"{}".to_vec()),
            ("images/000-photo.png".to_string(), vec![0_u8, 1, 2, 3, 250]),
        ]
    }

    #[test]
    fn crc32_matches_the_standard_value() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        assert_eq!(crc32(b""), 0);
    }

    #[test]
    fn a_stored_archive_round_trips() {
        let zip = build_zip(&entries());
        let read = read_zip(&zip).unwrap();
        assert_eq!(read, entries());
    }

    #[test]
    fn a_truncated_archive_is_rejected() {
        let zip = build_zip(&entries());
        assert!(read_zip(&zip[..zip.len() - 10]).is_err());
        assert!(read_zip(b"not a zip").is_err());
    }

    #[test]
    fn a_corrupted_entry_fails_its_checksum() {
        let mut zip = build_zip(&entries());
        // Flip a byte inside the first entry's data.
        let index = zip.len() - 100;
        zip[index] ^= 0xFF;
        assert!(read_zip(&zip).is_err());
    }

    #[test]
    fn names_are_sanitised() {
        assert_eq!(sanitize("photo.png"), "photo.png");
        assert_eq!(sanitize("a b/c.png"), "a_b_c.png");
        assert_eq!(sanitize("..."), "image");
    }

    #[test]
    fn an_archive_without_a_project_is_rejected() {
        let zip = build_zip(&[("images/a.png".to_string(), vec![1])]);
        let directory = std::env::temp_dir().join("bw_archive_test_no_project.bwpz");
        std::fs::write(&directory, zip).unwrap();
        assert!(super::read_archive(&directory, &std::env::temp_dir()).is_err());
        let _ = std::fs::remove_file(&directory);
    }

    #[test]
    fn a_written_archive_can_be_read_back() {
        let directory = std::env::temp_dir().join("bw_archive_roundtrip");
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).unwrap();
        let image_path = directory.join("source.png");
        let image = image::GrayImage::from_pixel(2, 2, image::Luma([0]));
        image.save(&image_path).unwrap();

        let mut document = Document::default();
        let layer_id = document.layers[0].id.clone();
        let mut picture = crate::objects::ImageObject::new(
            image_path.to_string_lossy(),
            2,
            2,
            crate::import::raster::DEFAULT_DPI,
        );
        picture.settings.mode = crate::objects::ImageMode::Threshold;
        let mut object = Object::new(layer_id, Shape::Image(picture));
        object.name = "Photo".to_string();
        document.add_object(object);

        let archive = directory.join("job.bwpz");
        let report = super::write_archive(&document, &archive).unwrap();
        assert_eq!(report.images, 1);
        assert_eq!(report.missing.len(), 0);

        let extracted = directory.join("extracted");
        let loaded = super::read_archive(&archive, &extracted).unwrap();
        assert_eq!(loaded.objects.len(), 1);
        let Shape::Image(image) = &loaded.objects[0].shape else {
            panic!("expected an image");
        };
        assert!(image.path.starts_with(extracted.to_string_lossy().as_ref()));
        assert!(std::path::Path::new(&image.path).is_file());
        let _ = std::fs::remove_dir_all(&directory);
    }

    #[test]
    fn missing_images_are_reported_but_do_not_block_the_archive() {
        let directory = std::env::temp_dir().join("bw_archive_missing");
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).unwrap();
        let mut document = Document::default();
        let layer_id = document.layers[0].id.clone();
        let mut object = Object::new(
            layer_id,
            Shape::Image(crate::objects::ImageObject::new(
                "/tmp/beamwright_no_such_image.png",
                2,
                2,
                96.0,
            )),
        );
        object.name = "Missing".to_string();
        document.add_object(object);
        let archive = directory.join("job.bwpz");
        let report = super::write_archive(&document, &archive).unwrap();
        assert_eq!(report.images, 0);
        assert_eq!(report.missing.len(), 1);
        assert!(archive.is_file());
        let _ = std::fs::remove_dir_all(&directory);
    }
}
