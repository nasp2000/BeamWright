use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileMode {
    OpenProject,
    SaveProject,
    ImportFile,
    ExportSvg,
    ExportDxf,
    ExportGcode,
}

impl FileMode {
    fn title(self) -> &'static str {
        match self {
            FileMode::OpenProject => "Open project or SVG",
            FileMode::SaveProject => "Save project",
            FileMode::ImportFile => "Import SVG or DXF",
            FileMode::ExportSvg => "Export SVG",
            FileMode::ExportDxf => "Export DXF",
            FileMode::ExportGcode => "Export G-code",
        }
    }

    fn action_label(self) -> &'static str {
        match self {
            FileMode::OpenProject | FileMode::ImportFile => "Open",
            FileMode::SaveProject => "Save",
            FileMode::ExportSvg | FileMode::ExportDxf | FileMode::ExportGcode => "Export",
        }
    }

    fn default_extension(self) -> &'static str {
        match self {
            FileMode::SaveProject => "bwp",
            FileMode::ExportGcode => "nc",
            FileMode::ExportDxf => "dxf",
            FileMode::OpenProject | FileMode::ImportFile | FileMode::ExportSvg => "svg",
        }
    }

    fn accepts(self, path: &Path) -> bool {
        let Some(extension) = path.extension().and_then(|value| value.to_str()) else {
            return false;
        };
        let extension = extension.to_ascii_lowercase();
        match self {
            FileMode::OpenProject => extension == "bwp" || extension == "svg",
            FileMode::SaveProject => extension == "bwp",
            FileMode::ImportFile => extension == "svg" || extension == "dxf",
            FileMode::ExportSvg => extension == "svg",
            FileMode::ExportDxf => extension == "dxf",
            FileMode::ExportGcode => extension == "nc" || extension == "gcode",
        }
    }
}

pub struct FileDialog {
    pub mode: FileMode,
    pub directory: PathBuf,
    pub file_name: String,
    pub error: Option<String>,
}

pub enum DialogResult {
    Pending,
    Cancelled,
    Accepted(PathBuf),
}

impl FileDialog {
    pub fn new(mode: FileMode, start: Option<&Path>, suggested_name: &str) -> Self {
        let (directory, file_name) = match start {
            Some(path) if path.is_dir() => (path.to_path_buf(), String::new()),
            Some(path) => (
                path.parent()
                    .map_or_else(|| PathBuf::from("."), Path::to_path_buf),
                path.file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or(suggested_name)
                    .to_string(),
            ),
            None => (
                std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
                suggested_name.to_string(),
            ),
        };
        Self {
            mode,
            directory,
            file_name,
            error: None,
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) -> DialogResult {
        let mut result = DialogResult::Pending;
        let mut open = true;
        egui::Window::new(self.mode.title())
            .collapsible(false)
            .resizable(true)
            .default_width(540.0)
            .open(&mut open)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if ui.button("Up").clicked() {
                        if let Some(parent) = self.directory.parent() {
                            self.directory = parent.to_path_buf();
                        }
                    }
                    if ui.button("Home").clicked() {
                        if let Some(home) = home_directory() {
                            self.directory = home;
                        }
                    }
                    ui.label(self.directory.display().to_string());
                });
                ui.separator();
                egui::ScrollArea::vertical()
                    .max_height(260.0)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        self.listing(ui);
                    });
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label("File name:");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.file_name)
                            .desired_width(f32::INFINITY),
                    );
                });
                if let Some(error) = &self.error {
                    ui.colored_label(egui::Color32::from_rgb(200, 60, 60), error);
                }
                ui.horizontal(|ui| {
                    if ui.button(self.mode.action_label()).clicked() {
                        match self.resolve() {
                            Ok(path) => result = DialogResult::Accepted(path),
                            Err(error) => self.error = Some(error),
                        }
                    }
                    if ui.button("Cancel").clicked() {
                        result = DialogResult::Cancelled;
                    }
                    ui.label(format!("Files must use .{}", self.mode.default_extension()));
                });
            });
        if !open && matches!(result, DialogResult::Pending) {
            result = DialogResult::Cancelled;
        }
        result
    }

    fn listing(&mut self, ui: &mut egui::Ui) {
        let entries = match std::fs::read_dir(&self.directory) {
            Ok(entries) => entries,
            Err(error) => {
                ui.colored_label(
                    egui::Color32::from_rgb(200, 60, 60),
                    format!("Cannot read directory: {error}"),
                );
                return;
            }
        };
        let mut directories = Vec::new();
        let mut files = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                directories.push(path);
            } else if self.mode.accepts(&path) {
                files.push(path);
            }
        }
        directories.sort();
        files.sort();
        for directory in directories {
            let name = directory
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("?");
            if ui
                .selectable_label(false, format!("[dir] {name}"))
                .clicked()
            {
                self.directory = directory;
                self.error = None;
            }
        }
        for file in files {
            let name = file
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("?");
            if ui.selectable_label(false, name).clicked() {
                self.file_name = name.to_string();
                self.error = None;
            }
        }
    }

    fn resolve(&self) -> Result<PathBuf, String> {
        let name = self.file_name.trim();
        if name.is_empty() {
            return Err("Enter a file name".to_string());
        }
        let mut path = self.directory.join(name);
        if path.extension().is_none() {
            path.set_extension(self.mode.default_extension());
        }
        if !self.mode.accepts(&path) {
            if matches!(
                self.mode,
                FileMode::SaveProject
                    | FileMode::ExportSvg
                    | FileMode::ExportDxf
                    | FileMode::ExportGcode
            ) {
                path.set_extension(self.mode.default_extension());
            } else {
                return Err(format!("Use a .{} file", self.mode.default_extension()));
            }
        }
        match self.mode {
            FileMode::OpenProject | FileMode::ImportFile => {
                if !path.is_file() {
                    return Err(format!("File not found: {}", path.display()));
                }
            }
            FileMode::SaveProject
            | FileMode::ExportSvg
            | FileMode::ExportDxf
            | FileMode::ExportGcode => {
                if let Some(parent) = path.parent() {
                    if !parent.is_dir() {
                        return Err(format!("Directory not found: {}", parent.display()));
                    }
                }
            }
        }
        Ok(path)
    }
}

fn home_directory() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::{FileDialog, FileMode};

    #[test]
    fn open_accepts_projects_and_svg() {
        assert!(FileMode::OpenProject.accepts(std::path::Path::new("a.bwp")));
        assert!(FileMode::OpenProject.accepts(std::path::Path::new("a.SVG")));
        assert!(!FileMode::OpenProject.accepts(std::path::Path::new("a.dxf")));
    }

    #[test]
    fn save_adds_default_extension() {
        let mut dialog = FileDialog::new(
            FileMode::SaveProject,
            Some(std::path::Path::new("/tmp/beamwright-dialog-test")),
            "untitled",
        );
        dialog.directory = std::env::temp_dir();
        dialog.file_name = "beamwright-dialog-test".to_string();
        let path = dialog.resolve().unwrap();
        assert_eq!(
            path.extension().and_then(|value| value.to_str()),
            Some("bwp")
        );
    }

    #[test]
    fn gcode_export_uses_nc_extension() {
        let mut dialog = FileDialog::new(
            FileMode::ExportGcode,
            Some(std::path::Path::new("/tmp/beamwright-job")),
            "job",
        );
        dialog.directory = std::env::temp_dir();
        dialog.file_name = "beamwright-job".to_string();
        let path = dialog.resolve().unwrap();
        assert_eq!(
            path.extension().and_then(|value| value.to_str()),
            Some("nc")
        );
        assert!(FileMode::ExportGcode.accepts(std::path::Path::new("a.gcode")));
        assert!(!FileMode::ExportGcode.accepts(std::path::Path::new("a.svg")));
    }

    #[test]
    fn import_rejects_missing_file() {
        let mut dialog = FileDialog::new(
            FileMode::ImportFile,
            Some(std::path::Path::new("/tmp")),
            "missing.svg",
        );
        dialog.file_name = "definitely-not-here-12345.svg".to_string();
        assert!(dialog.resolve().is_err());
    }
}
