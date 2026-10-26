use crate::geometry::boolean::clip_polygon as clip_poly;

/// Clip (intersection) of subject polygon by clip polygon (convex clip).
pub fn clip(subject: &[(f64, f64)], clip: &[(f64, f64)]) -> Vec<(f64, f64)> {
    clip_poly(subject, clip)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clip_basic() {
        let subject = [(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)];
        let clipper = [(2.0, 2.0), (8.0, 2.0), (8.0, 8.0), (2.0, 8.0)];
        let r = clip(&subject, &clipper);
        assert_ne!(r, [] as [(f64, f64); 0]);
    }

    #[test]
    fn clip_no_overlap() {
        let subject = [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)];
        let clipper = [(5.0, 5.0), (6.0, 5.0), (6.0, 6.0), (5.0, 6.0)];
        let r = clip(&subject, &clipper);
        assert_eq!(r, [] as [(f64, f64); 0]);
    }

    #[test]
    fn clip_empty_subject() {
        let clipper = [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0)];
        let r = clip(&[], &clipper);
        assert_eq!(r, [] as [(f64, f64); 0]);
    }
}
