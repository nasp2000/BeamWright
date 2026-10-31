pub mod dialect;
pub mod job;
pub mod playback;
pub mod toolpath;
pub mod writer;

pub use dialect::{GCodeOptions, LaserMode};
pub use job::{
    JobOptions, JobReport, bounds_warnings, build_toolpaths, build_toolpaths_cancellable,
    estimate_seconds, export_job, export_job_cancellable, order_by_operation, order_toolpaths,
    planned_toolpaths, planned_toolpaths_cancellable, render_job, render_job_cancellable,
    validate_bounds,
};
pub use playback::{Playback, PlaybackPosition, PlaybackSegment};
pub use toolpath::{MoveKind, Toolpath};
pub use writer::GCodeWriter;

/// Prefix every line of G-code with a right-aligned line number.
///
/// The preview shows the same text that would be written to the file, plus a
/// gutter, so a line can be discussed without changing the job.
pub fn number_lines(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let width = lines.len().to_string().len();
    lines
        .iter()
        .enumerate()
        .map(|(index, line)| format!("{:>width$}  {line}", index + 1, width = width))
        .collect::<Vec<String>>()
        .join("\n")
}

#[cfg(test)]
mod numbering_tests {
    use super::number_lines;

    #[test]
    fn numbers_are_aligned() {
        assert_eq!(number_lines("G21\nG90\nM5"), "1  G21\n2  G90\n3  M5");
        assert_eq!(number_lines(""), "");
        assert_eq!(number_lines("only"), "1  only");
    }

    #[test]
    fn wider_numbers_keep_the_gutter() {
        let text = (0..12)
            .map(|index| format!("L{index}"))
            .collect::<Vec<_>>()
            .join("\n");
        let numbered = number_lines(&text);
        assert!(numbered.starts_with(" 1  L0"));
        assert!(numbered.contains("12  L11"));
    }
}
