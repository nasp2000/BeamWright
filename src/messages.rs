use std::collections::HashMap;

#[derive(Debug, Default)]
pub struct MessageCatalog {
    entries: HashMap<&'static str, &'static str>,
}

impl MessageCatalog {
    pub fn new() -> Self {
        let mut c = Self {
            entries: HashMap::new(),
        };
        c.entries.insert("app.title", "BeamWright");
        c.entries.insert("menu.file", "File");
        c.entries.insert("menu.edit", "Edit");
        c.entries.insert("menu.draw", "Draw");
        c.entries.insert("menu.view", "View");
        c.entries.insert("status.ready", "Ready");
        c.entries.insert("dialog.ok", "OK");
        c.entries.insert("dialog.cancel", "Cancel");
        c
    }
    pub fn get(&self, key: &'static str) -> &'static str {
        self.entries.get(key).copied().unwrap_or(key)
    }
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::MessageCatalog;

    #[test]
    fn title() {
        let c = MessageCatalog::new();
        assert_eq!(c.get("app.title"), "BeamWright");
    }

    #[test]
    fn missing_returns_key() {
        let c = MessageCatalog::new();
        assert_eq!(c.get("nope"), "nope");
    }

    #[test]
    fn non_empty() {
        let c = MessageCatalog::new();
        assert!(!c.is_empty());
        assert!(!c.is_empty());
    }
}
