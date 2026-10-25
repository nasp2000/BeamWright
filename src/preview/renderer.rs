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

#[cfg(test)]
mod tests {
    use super::PreviewData;

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
}
