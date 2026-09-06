use scholium_doc::{Document, NodeKind};

use crate::SourceMap;

/// Serialize a `Document` to Typst source with a `SourceMap`.
///
/// The `SourceMap` maps every node's byte range in the output to its `NodeId`,
/// enabling reverse lookup from Typst's `Span → Source::range()` → `NodeId`.
pub fn serialize(doc: &Document) -> (String, SourceMap) {
    let mut ser = Serializer::new(doc);
    ser.run();
    (ser.output, ser.source_map)
}

struct Serializer<'a> {
    doc: &'a Document,
    output: String,
    source_map: SourceMap,
}

impl<'a> Serializer<'a> {
    fn new(doc: &'a Document) -> Self {
        Self {
            doc,
            output: String::new(),
            source_map: SourceMap::new(),
        }
    }

    fn run(&mut self) {
        let root = self.doc.root();
        self.serialize_node(root);
        self.source_map.sort();
    }

    fn serialize_node(&mut self, node_id: scholium_doc::NodeId) {
        let start = self.output.len();
        let node = self.doc.node(node_id);

        match node.kind {
            NodeKind::Document => {
                for child in &node.children {
                    self.serialize_node(*child);
                }
            }

            NodeKind::Paragraph => {
                for child in &node.children {
                    self.serialize_node(*child);
                }
                self.output.push_str("\n\n");
            }

            NodeKind::Heading => {
                let level = node.heading_level.unwrap_or(1).clamp(1, 6);
                for _ in 0..level {
                    self.output.push('=');
                }
                self.output.push(' ');
                for child in &node.children {
                    self.serialize_node(*child);
                }
                self.output.push_str("\n\n");
            }

            NodeKind::Text => {
                if let Some(ref text) = node.text {
                    self.serialize_text(node_id, text);
                }
            }
        }

        let end = self.output.len();
        if start < end {
            self.source_map.push(start..end, node_id);
        }
    }

    fn serialize_text(&mut self, node_id: scholium_doc::NodeId, text: &str) {
        for (text_start, ch) in text.char_indices() {
            let source_start = self.output.len();
            if is_typst_markup(ch) {
                self.output.push('\\');
            }
            self.output.push(ch);
            let source_end = self.output.len();
            let text_end = text_start + ch.len_utf8();
            self.source_map
                .push_text(source_start..source_end, node_id, text_start..text_end);
        }
    }
}

fn is_typst_markup(ch: char) -> bool {
    matches!(
        ch,
        '\\' | '#' | '$' | '*' | '_' | '@' | '<' | '>' | '[' | ']'
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use scholium_doc::{Document, NodeKind};

    fn make_doc() -> Document {
        let mut doc = Document::new();
        let root = doc.root();
        let p1 = doc.append_child(root, NodeKind::Paragraph, None);
        doc.append_child(p1, NodeKind::Text, Some("Hello, ".to_string()));
        doc.append_child(p1, NodeKind::Text, Some("世界！".to_string()));
        let p2 = doc.append_child(root, NodeKind::Paragraph, None);
        doc.append_child(p2, NodeKind::Text, Some("Second paragraph.".to_string()));
        let h1 = doc.append_child(root, NodeKind::Heading, None);
        {
            let n = doc.node_mut(h1);
            n.heading_level = Some(2);
        }
        doc.append_child(h1, NodeKind::Text, Some("Section Title".to_string()));
        doc
    }

    #[test]
    fn serializes_paragraphs() {
        let (src, _sm) = serialize(&make_doc());
        assert!(src.contains("Hello,"));
        assert!(src.contains("世界！"));
        assert!(src.contains("Second paragraph."));
    }

    #[test]
    fn serializes_headings() {
        let (src, _sm) = serialize(&make_doc());
        assert!(src.contains("== Section Title"));
    }

    #[test]
    fn adds_paragraph_separators() {
        let (src, _sm) = serialize(&make_doc());
        assert!(src.contains("\n\n"));
        // Two paragraphs = one separator between them
        let count = src.matches("\n\n").count();
        assert!(count >= 2, "expected at least 2 blank lines, got {count}");
    }

    #[test]
    fn source_map_has_all_nodes() {
        let doc = make_doc();
        let (_src, sm) = serialize(&doc);
        // Document + 3 structural (P1, P2, H1) + 4 text = 8
        assert_eq!(sm.len(), 8);
    }

    #[test]
    fn innermost_returns_deepest_node() {
        let doc = make_doc();
        let (src, sm) = serialize(&doc);
        let hello_start = src.find("Hello,").expect("Hello, found in source");
        let hello_end = hello_start + "Hello,".len();
        let id = sm
            .innermost(hello_start..hello_end)
            .expect("contains range");
        let node = doc.node(id);
        assert_eq!(node.kind, NodeKind::Text);
        assert_eq!(node.text.as_deref(), Some("Hello, "));
    }

    #[test]
    fn innermost_returns_none_for_out_of_range() {
        let (_src, sm) = serialize(&make_doc());
        assert!(sm.innermost(9999..10000).is_none());
    }

    #[test]
    fn empty_document_produces_empty_source_map() {
        let doc = Document::new();
        let (src, sm) = serialize(&doc);
        assert!(src.is_empty());
        assert!(sm.is_empty());
    }

    #[test]
    fn heading_level_uses_correct_prefix() {
        let mut doc = Document::new();
        let root = doc.root();
        let h = doc.append_child(root, NodeKind::Heading, None);
        doc.node_mut(h).heading_level = Some(3);
        doc.append_child(h, NodeKind::Text, Some("Level 3".to_string()));
        let (src, _sm) = serialize(&doc);
        assert!(src.starts_with("=== Level 3"));
    }

    #[test]
    fn text_without_heading_level_defaults_to_1() {
        let mut doc = Document::new();
        let root = doc.root();
        let h = doc.append_child(root, NodeKind::Heading, None);
        doc.append_child(h, NodeKind::Text, Some("Default level".to_string()));
        let (src, _sm) = serialize(&doc);
        assert!(src.starts_with("= Default level"));
    }

    #[test]
    fn escapes_typst_markup_and_preserves_text_offsets() {
        let mut doc = Document::new();
        let root = doc.root();
        let text = doc.append_child(root, NodeKind::Text, Some("a#中*".to_string()));

        let (src, map) = serialize(&doc);

        assert_eq!(src, "a\\#中\\*");
        assert_eq!(map.text_position(2), Some((text, 1..2)));
        assert_eq!(map.text_position(3), Some((text, 2..5)));
        assert_eq!(map.text_range(0, 2), Some((text, 0..2)));
    }
}
