use crate::document::Document;
use crate::errors::Result;

pub fn to_json(doc: &Document) -> Result<String> {
    Ok(serde_json::to_string_pretty(doc)?)
}

pub fn from_json(s: &str) -> Result<Document> {
    Ok(serde_json::from_str(s)?)
}

#[allow(dead_code)]
fn _placeholder() {}
