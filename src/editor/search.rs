use crate::document::Document;
/// What a global search match refers to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchKind {
    Object,
    Layer,
    Operation,
    /// A material, operation, image or project profile.
    Profile,
}

/// One hit from a document-wide search.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchHit {
    pub kind: SearchKind,
    pub id: String,
    pub label: String,
}

/// Search the profile library by name, category and tags.
pub fn search_profiles(library: &crate::profiles::ProfileLibrary, query: &str) -> Vec<SearchHit> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return Vec::new();
    }
    let mut hits = Vec::new();
    let mut push = |kind: &str, id: &str, name: &str, meta: &crate::profiles::ProfileMeta| {
        let text = format!("{} {} {}", kind, name, meta.category).to_lowercase();
        if text.contains(&query)
            || meta
                .tags
                .iter()
                .any(|tag| tag.to_lowercase().contains(&query))
        {
            hits.push(SearchHit {
                kind: SearchKind::Profile,
                id: id.to_string(),
                label: if meta.category.is_empty() {
                    format!("{kind}: {name}")
                } else {
                    format!("{kind}: {name} ({})", meta.category)
                },
            });
        }
    };
    for profile in &library.materials {
        push("Material", &profile.id, &profile.name, &profile.meta);
    }
    for profile in &library.operations {
        push(
            "Operation profile",
            &profile.id,
            &profile.name,
            &profile.meta,
        );
    }
    for profile in &library.images {
        push("Image profile", &profile.id, &profile.name, &profile.meta);
    }
    for profile in &library.projects {
        push("Project profile", &profile.id, &profile.name, &profile.meta);
    }
    hits
}

/// Search objects, layers and operations by name and identifier.
pub fn search_document(doc: &Document, query: &str) -> Vec<SearchHit> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return Vec::new();
    }
    let mut hits = Vec::new();
    for object in &doc.objects {
        let label = format!("{} {}", object.shape.kind_name(), object.name);
        if label.to_lowercase().contains(&query) || object.id.to_lowercase().contains(&query) {
            hits.push(SearchHit {
                kind: SearchKind::Object,
                id: object.id.clone(),
                label,
            });
        }
    }
    for layer in &doc.layers {
        if layer.name.to_lowercase().contains(&query) || layer.id.to_lowercase().contains(&query) {
            hits.push(SearchHit {
                kind: SearchKind::Layer,
                id: layer.id.clone(),
                label: format!("Layer: {}", layer.name),
            });
        }
    }
    for operation in &doc.operations {
        if operation.name.to_lowercase().contains(&query)
            || operation.id.to_lowercase().contains(&query)
            || operation.op_type.label().to_lowercase().contains(&query)
        {
            hits.push(SearchHit {
                kind: SearchKind::Operation,
                id: operation.id.clone(),
                label: format!("Operation: {}", operation.name),
            });
        }
    }
    hits
}

#[cfg(test)]
mod document_tests {
    use super::*;
    use crate::document::model::{Object, Shape};
    use crate::operations::{Operation, OperationType};

    fn document() -> Document {
        let mut doc = Document::default();
        doc.layers[0].name = "Cut layer".to_string();
        let layer_id = doc.layers[0].id.clone();
        let mut object = Object::new(
            layer_id,
            Shape::Rect {
                width: 10.0,
                height: 5.0,
                corner_radius: 0.0,
            },
        );
        object.name = "Panel".to_string();
        doc.add_object(object);
        doc.operations
            .push(Operation::new("Engrave text", OperationType::LineEngraving));
        doc
    }

    #[test]
    fn finds_all_document_kinds() {
        let doc = document();
        assert_eq!(search_document(&doc, "panel")[0].kind, SearchKind::Object);
        assert_eq!(
            search_document(&doc, "cut layer")[0].kind,
            SearchKind::Layer
        );
        assert_eq!(
            search_document(&doc, "engrave")[0].kind,
            SearchKind::Operation
        );
        assert_eq!(search_document(&doc, "panel").len(), 1);
    }

    #[test]
    fn empty_query_and_misses_return_nothing() {
        let doc = document();
        assert_eq!(search_document(&doc, "  ").len(), 0);
        assert_eq!(search_document(&doc, "zzz").len(), 0);
    }

    #[test]
    fn profiles_are_searchable_by_name_category_and_tag() {
        use crate::profiles::{MaterialProfile, ProfileLibrary};

        let mut library = ProfileLibrary::with_builtins();
        let mut profile = MaterialProfile::new("Birch board", "Birch", 4.0);
        profile.meta.category = "Signage".to_string();
        profile.meta.tags = vec!["outdoor".to_string()];
        library.add_material(profile);
        for query in ["birch board", "signage", "outdoor"] {
            let hits = super::search_profiles(&library, query);
            assert!(!hits.is_empty(), "{query}");
            assert_eq!(hits[0].kind, SearchKind::Profile);
        }
        assert_eq!(super::search_profiles(&library, "zzz").len(), 0);
        assert_eq!(super::search_profiles(&library, "  ").len(), 0);
        // Built-ins are searchable too.
        assert_ne!(super::search_profiles(&library, "plywood").len(), 0);
    }

    #[test]
    fn operation_type_is_searchable() {
        let doc = document();
        let hits = search_document(&doc, "line engraving");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].kind, SearchKind::Operation);
    }
}
