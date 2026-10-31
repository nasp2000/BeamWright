/// Summary of one import or export, shown in the operation report dialog.
///
/// The report is deliberately plain data so it can be built and tested without
/// egui, and so the same values can be printed by the CLI.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FileReport {
    /// Verb describing the operation, for example `Import` or `Export G-code`.
    pub action: String,
    /// File that was read or written.
    pub path: String,
    /// Ordered key/value lines describing what was produced.
    pub summary: Vec<(String, String)>,
    /// Elements that were skipped or approximated.
    pub warnings: Vec<String>,
}

impl FileReport {
    pub fn new(action: impl Into<String>, path: impl Into<String>) -> Self {
        Self {
            action: action.into(),
            path: path.into(),
            summary: Vec::new(),
            warnings: Vec::new(),
        }
    }

    pub fn with_summary(mut self, label: impl Into<String>, value: impl Into<String>) -> Self {
        self.summary.push((label.into(), value.into()));
        self
    }

    pub fn with_warnings(mut self, warnings: Vec<String>) -> Self {
        self.warnings = warnings;
        self
    }

    pub fn warning_count(&self) -> usize {
        self.warnings.len()
    }

    pub fn is_clean(&self) -> bool {
        self.warnings.is_empty()
    }

    /// One line used by the status bar and the CLI.
    pub fn headline(&self) -> String {
        if self.is_clean() {
            format!("{} {} complete", self.action, self.path)
        } else {
            format!(
                "{} {} complete with {} warning(s)",
                self.action,
                self.path,
                self.warning_count()
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::FileReport;

    #[test]
    fn clean_report_has_no_warnings() {
        let report = FileReport::new("Export SVG", "drawing.svg")
            .with_summary("Objects", "3")
            .with_summary("Layers", "1");
        assert!(report.is_clean());
        assert_eq!(report.warning_count(), 0);
        assert_eq!(report.summary.len(), 2);
        assert_eq!(report.headline(), "Export SVG drawing.svg complete");
    }

    #[test]
    fn warnings_change_the_headline() {
        let report = FileReport::new("Import", "drawing.svg")
            .with_warnings(vec!["<use> is not expanded".to_string()]);
        assert!(!report.is_clean());
        assert_eq!(report.warning_count(), 1);
        assert_eq!(
            report.headline(),
            "Import drawing.svg complete with 1 warning(s)"
        );
    }

    #[test]
    fn summary_order_is_preserved() {
        let report = FileReport::new("Export G-code", "job.nc")
            .with_summary("Paths", "4")
            .with_summary("Lines", "120");
        let labels: Vec<&str> = report
            .summary
            .iter()
            .map(|(label, _)| label.as_str())
            .collect();
        assert_eq!(labels, vec!["Paths", "Lines"]);
    }
}
