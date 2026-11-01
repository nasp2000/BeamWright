//! Import and export the material library as CSV.
//!
//! The columns are deliberately flat so a spreadsheet can read them:
//! `name,material,thickness_mm,category,notes,builtin`. Quotes are written only
//! where needed and doubled inside a field, so notes may contain commas.

use std::fmt::Write as _;

use crate::profiles::model::{MaterialProfile, ProfileLibrary, ProfileMeta};

/// Column header written by the export and required by the import.
pub const MATERIAL_HEADER: &str = "name,material,thickness_mm,category,notes,builtin";

/// Write every material as CSV, built-ins included.
pub fn export_materials_csv(library: &ProfileLibrary) -> String {
    let mut out = String::from(MATERIAL_HEADER);
    out.push('\n');
    for material in &library.materials {
        out.push_str(&escape(&material.name));
        out.push(',');
        out.push_str(&escape(&material.material));
        out.push(',');
        let _ = write!(out, "{:.3}", material.thickness_mm);
        out.push(',');
        out.push_str(&escape(&material.meta.category));
        out.push(',');
        out.push_str(&escape(&material.notes));
        out.push(',');
        out.push_str(if material.builtin { "true" } else { "false" });
        out.push('\n');
    }
    out
}

/// Merge user materials from CSV into `library`.
///
/// Built-in rows are ignored (they ship with the application), rows with a
/// matching user name are replaced and new names are appended. Returns the
/// number of materials written.
pub fn import_materials_csv(library: &mut ProfileLibrary, text: &str) -> Result<usize, String> {
    let mut lines = text.lines();
    let header = lines
        .next()
        .ok_or_else(|| "the CSV file is empty".to_string())?;
    let header = header.trim_start_matches('\u{feff}');
    if header.trim() != MATERIAL_HEADER {
        return Err(format!(
            "the first line must be the header '{MATERIAL_HEADER}'"
        ));
    }
    let mut imported = 0;
    for (index, line) in lines.enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let fields =
            parse_line(line).ok_or_else(|| format!("line {} has unbalanced quotes", index + 2))?;
        if fields.len() != 6 {
            return Err(format!(
                "line {} has {} fields; expected 6",
                index + 2,
                fields.len()
            ));
        }
        let mut fields = fields.into_iter();
        let name = fields.next().unwrap_or_default().trim().to_string();
        let material = fields.next().unwrap_or_default().trim().to_string();
        let thickness = fields.next().unwrap_or_default().trim().to_string();
        let category = fields.next().unwrap_or_default().trim().to_string();
        let notes = fields.next().unwrap_or_default();
        let builtin = fields.next().unwrap_or_default();
        let builtin = builtin.trim();
        if name.is_empty() {
            return Err(format!("line {} has no name", index + 2));
        }
        if builtin.eq_ignore_ascii_case("true") {
            // Shipped with the application; never overwritten by an import.
            continue;
        }
        let thickness_mm = thickness
            .parse::<f64>()
            .map_err(|_| format!("line {} has an invalid thickness", index + 2))?;
        if !thickness_mm.is_finite() || !(0.1..=100.0).contains(&thickness_mm) {
            return Err(format!(
                "line {} thickness must be between 0.1 and 100 mm",
                index + 2
            ));
        }
        let material = if material.is_empty() {
            name.clone()
        } else {
            material
        };
        let mut profile = MaterialProfile::new(name.clone(), material, thickness_mm);
        profile.notes = notes;
        if !category.is_empty() {
            profile.meta = ProfileMeta::with_category(category);
        }
        profile.validate()?;
        if let Some(existing) = library
            .materials
            .iter_mut()
            .find(|candidate| !candidate.builtin && candidate.name == name)
        {
            *existing = profile;
        } else {
            library.materials.push(profile);
        }
        imported += 1;
    }
    library.validate()?;
    Ok(imported)
}

fn escape(value: &str) -> String {
    if value.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

/// Split one CSV line, honouring quotes and doubled quotes.
fn parse_line(line: &str) -> Option<Vec<String>> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    let mut characters = line.chars().peekable();
    while let Some(character) = characters.next() {
        match character {
            '"' if quoted && characters.peek() == Some(&'"') => {
                current.push('"');
                characters.next();
            }
            '"' => quoted = !quoted,
            ',' if !quoted => {
                fields.push(std::mem::take(&mut current));
            }
            other => current.push(other),
        }
    }
    if quoted {
        return None;
    }
    fields.push(current);
    Some(fields)
}

#[cfg(test)]
mod tests {
    use super::{MATERIAL_HEADER, export_materials_csv, import_materials_csv, parse_line};
    use crate::profiles::model::{MaterialProfile, ProfileLibrary};

    #[test]
    fn the_export_contains_every_material_and_the_header() {
        let library = ProfileLibrary::with_builtins();
        let csv = export_materials_csv(&library);
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines[0], MATERIAL_HEADER);
        assert_eq!(lines.len(), 1 + library.materials.len());
        assert!(lines.iter().any(|line| line.starts_with("Plywood 3 mm,")));
    }

    #[test]
    fn notes_with_commas_and_quotes_survive_the_round_trip() {
        let mut library = ProfileLibrary::empty();
        let mut material = MaterialProfile::new("Odd", "Paper", 1.5);
        material.notes = "cuts, \"fast\"".to_string();
        material.meta = crate::profiles::model::ProfileMeta::with_category("Test");
        library.materials.push(material);
        let csv = export_materials_csv(&library);

        let mut imported = ProfileLibrary::empty();
        let count = import_materials_csv(&mut imported, &csv).unwrap();
        assert_eq!(count, 1);
        assert_eq!(imported.materials[0].name, "Odd");
        assert_eq!(imported.materials[0].notes, "cuts, \"fast\"");
        assert_eq!(imported.materials[0].meta.category, "Test");
    }

    #[test]
    fn builtin_rows_are_ignored_on_import() {
        let library = ProfileLibrary::with_builtins();
        let csv = export_materials_csv(&library);
        let mut target = ProfileLibrary::empty();
        let count = import_materials_csv(&mut target, &csv).unwrap();
        assert_eq!(count, 0);
        assert_eq!(target.materials.len(), 0);
    }

    #[test]
    fn a_matching_user_name_is_replaced() {
        let mut library = ProfileLibrary::empty();
        library
            .materials
            .push(MaterialProfile::new("Custom", "Wood", 3.0));
        let csv = format!("{MATERIAL_HEADER}\nCustom,Acrylic,5,Plastic,new note,false\n");
        let count = import_materials_csv(&mut library, &csv).unwrap();
        assert_eq!(count, 1);
        assert_eq!(library.materials.len(), 1);
        assert_eq!(library.materials[0].material, "Acrylic");
        assert_eq!(library.materials[0].thickness_mm, 5.0);
        assert_eq!(library.materials[0].notes, "new note");
    }

    #[test]
    fn bad_input_is_rejected_with_the_line_number() {
        let mut library = ProfileLibrary::empty();
        assert!(import_materials_csv(&mut library, "").is_err());
        assert!(
            import_materials_csv(&mut library, "name,other\nA,B\n")
                .unwrap_err()
                .contains("header")
        );
        let bad_thickness = format!("{MATERIAL_HEADER}\nA,Wood,zero,,,false\n");
        assert!(
            import_materials_csv(&mut library, &bad_thickness)
                .unwrap_err()
                .contains("line 2")
        );
        let missing = format!("{MATERIAL_HEADER}\n,Wood,3,,,false\n");
        assert!(import_materials_csv(&mut library, &missing).is_err());
    }

    #[test]
    fn quoted_fields_are_parsed() {
        assert_eq!(
            parse_line("a,\"b,c\",\"d\"\"e\"").unwrap(),
            vec!["a".to_string(), "b,c".to_string(), "d\"e".to_string()]
        );
        assert!(parse_line("\"unbalanced").is_none());
    }
}
