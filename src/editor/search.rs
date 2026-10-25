use crate::document::Document;

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub object_index: usize,
    pub label: String,
}

pub fn search_objects(doc: &Document, query: &str) -> Vec<SearchResult> {
    if query.trim().is_empty() {
        return Vec::new();
    }
    let q = query.to_lowercase();
    let mut out = Vec::new();
    for (i, obj) in doc.objects.iter().enumerate() {
        let label = format!("{:?} {}", obj.kind, obj.id);
        if label.to_lowercase().contains(&q) || obj.id.to_lowercase().contains(&q) {
            out.push(SearchResult {
                object_index: i,
                label,
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::Document;

    #[test]
    fn empty_query() {
        let d = Document::default();
        assert!(search_objects(&d, "").is_empty());
        assert!(search_objects(&d, "   ").is_empty());
    }

    #[test]
    fn finds_by_id() {
        let mut d = Document::default();
        d.objects.push(crate::document::model::DocumentObject {
            id: "myline".to_string(),
            kind: crate::document::model::ObjectKind::Line,
            layer_id: String::new(),
            operation_id: None,
        });
        let r = search_objects(&d, "myline");
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].object_index, 0);
    }

    #[test]
    fn no_match() {
        let d = Document::default();
        assert!(search_objects(&d, "zzz").is_empty());
    }
}
