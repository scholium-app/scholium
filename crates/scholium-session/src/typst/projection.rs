//! Stable semantic content and endpoints, independent of random document/node identities.
use super::*;
use serde_json::{Value, json};
use std::collections::HashMap;

/// Strip only runtime identities and revision counters; retain every content/style field.
fn content(value: &Value) -> Value {
    match value {
        Value::Object(fields) => fields
            .iter()
            .filter(|(key, _)| !matches!(key.as_str(), "node" | "document" | "revision"))
            .map(|(key, value)| (key.clone(), content(value)))
            .collect(),
        Value::Array(values) => values.iter().map(content).collect(),
        value => value.clone(),
    }
}

fn paths(value: &Value, path: &str, result: &mut HashMap<String, String>) {
    match value {
        Value::Object(fields) => {
            if let Some(node) = fields.get("node").and_then(Value::as_str) {
                result.insert(node.to_owned(), path.to_owned());
            }
            for (key, child) in fields {
                let child_path = if path.is_empty() {
                    key.clone()
                } else {
                    format!("{path}/{key}")
                };
                paths(child, &child_path, result);
            }
        }
        Value::Array(values) => {
            for (index, child) in values.iter().enumerate() {
                paths(child, &format!("{path}/{index}"), result);
            }
        }
        _ => {}
    }
}

impl EditorCore {
    /// Stable content summary and semantic endpoints for UI-free/window consumers.
    ///
    /// `snapshot` preserves the entire semantic tree, except random `node`/`document`
    /// identities and the revision counter. Endpoints use the node's tree path,
    /// leaf-local UTF-8 byte offset and affinity; the anchor stays null when absent.
    /// A missing endpoint remains observable as a null path instead of being guessed.
    pub fn semantic_projection(&self) -> Value {
        // StructuredDocument consists solely of JSON-compatible data.
        let snapshot = json!(self.snapshot.as_ref());
        let mut locations = HashMap::new();
        paths(&snapshot, "", &mut locations);
        let position = |at: Position| {
            let id = json!(at.leaf);
            json!({
                "path": id.as_str().and_then(|node| locations.get(node)),
                "byte": at.byte,
                "affinity": format!("{:?}", at.affinity),
            })
        };
        json!({
            "snapshot": content(&snapshot),
            "cursor": position(self.cursor),
            "anchor": self.anchor.map(position),
            "undo_depth": self.undo.len(),
        })
    }
}
