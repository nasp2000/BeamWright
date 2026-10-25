#[derive(Debug, Clone)]
pub struct PreviewSegment {
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
    pub color: String,
}

#[derive(Debug, Default)]
pub struct PreviewData {
    pub segments: Vec<PreviewSegment>,
}

impl PreviewData {
    pub fn add_segment(&mut self, x1: f64, y1: f64, x2: f64, y2: f64, color: String) {
        self.segments.push(PreviewSegment {
            x1,
            y1,
            x2,
            y2,
            color,
        });
    }
    pub fn segment_count(&self) -> usize {
        self.segments.len()
    }
    pub fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }
}

/// Build a simple preview from a document's objects (one segment per object).
#[allow(clippy::cast_precision_loss)]
pub fn build_preview(object_count: usize) -> PreviewData {
    let mut p = PreviewData::default();
    for i in 0..object_count {
        p.add_segment(
            i as f64 * 10.0,
            0.0,
            i as f64 * 10.0 + 5.0,
            0.0,
            "#00ff00".to_string(),
        );
    }
    p
}

#[cfg(test)]
mod tests {
    use super::{PreviewData, build_preview};

    #[test]
    fn empty() {
        let p = PreviewData::default();
        assert!(p.is_empty());
        assert_eq!(p.segment_count(), 0);
    }

    #[test]
    fn add() {
        let mut p = PreviewData::default();
        p.add_segment(0.0, 0.0, 1.0, 1.0, "#ff0000".to_string());
        assert_eq!(p.segment_count(), 1);
        assert!(!p.is_empty());
    }

    #[test]
    fn multiple() {
        let mut p = PreviewData::default();
        for i in 0..5 {
            p.add_segment(f64::from(i), 0.0, f64::from(i + 1), 0.0, "#000".to_string());
        }
        assert_eq!(p.segment_count(), 5);
    }

    #[test]
    fn build_preview_empty() {
        let p = build_preview(0);
        assert!(p.is_empty());
    }

    #[test]
    fn build_preview_count() {
        let p = build_preview(3);
        assert_eq!(p.segment_count(), 3);
    }
}
