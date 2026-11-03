use super::*;
use ::loro::{ExportMode, LoroDoc, LoroText, TreeID, ValueOrContainer};
use std::collections::HashMap;
#[derive(Debug)]
pub struct Loro {
    doc: LoroDoc,
    ids: HashMap<String, TreeID>,
    import_ms: f64,
    index_ms: f64,
}
impl Backend for Loro {
    fn new(peer: u64) -> Self {
        let doc = LoroDoc::new();
        doc.set_peer_id(peer).expect("peer");
        doc.get_tree("nodes").enable_fractional_index(0);
        Self {
            doc,
            ids: HashMap::new(),
            import_ms: 0.0,
            index_ms: 0.0,
        }
    }
    fn apply(&mut self, edit: &Edit) {
        let tree = self.doc.get_tree("nodes");
        match edit {
            Edit::Create {
                id,
                parent,
                kind,
                text,
            } => {
                let node = tree.create(self.ids.get(parent).copied()).expect("create");
                let meta = tree.get_meta(node).expect("meta");
                meta.insert("id", id.as_str()).expect("id");
                meta.insert("kind", *kind).expect("kind");
                let leaf = meta
                    .insert_container("text", LoroText::new())
                    .expect("text");
                leaf.insert(0, text).expect("initial text");
                self.ids.insert(id.clone(), node);
            }
            Edit::Delete(id) => tree.delete(self.ids[id]).expect("delete"),
            Edit::Move { id, parent } => tree.mov(self.ids[id], self.ids[parent]).expect("move"),
            Edit::Insert(id) => text(&self.doc, self.ids[id])
                .insert(0, "é")
                .expect("insert"),
            Edit::Remove(id) => text(&self.doc, self.ids[id])
                .delete(0, 1)
                .expect("delete char"),
        }
        self.doc.commit();
    }
    fn snapshot(&self) -> Vec<u8> {
        self.doc.export(ExportMode::Snapshot).expect("snapshot")
    }
    fn updates(&self) -> Vec<u8> {
        self.doc.export(ExportMode::all_updates()).expect("updates")
    }
    fn import(&mut self, bytes: &[u8]) {
        let start = std::time::Instant::now();
        self.doc.import(bytes).expect("import");
        self.import_ms = start.elapsed().as_secs_f64() * 1000.0;
        let start = std::time::Instant::now();
        let tree = self.doc.get_tree("nodes");
        for node in tree.nodes() {
            let id = value(&tree.get_meta(node).expect("meta"), "id");
            self.ids.insert(id, node);
        }
        self.index_ms = start.elapsed().as_secs_f64() * 1000.0;
    }
    fn last_import_ms(&self) -> (f64, f64) {
        (self.import_ms, self.index_ms)
    }
    fn state(&self) -> State {
        let tree = self.doc.get_tree("nodes");
        tree.nodes()
            .into_iter()
            .filter(|id| !tree.is_node_deleted(id).expect("deleted"))
            .map(|id| {
                let meta = tree.get_meta(id).expect("meta");
                let parent = tree
                    .parent(id)
                    .and_then(|p| p.tree_id())
                    .map(|p| value(&tree.get_meta(p).expect("parent"), "id"))
                    .unwrap_or_default();
                (
                    value(&meta, "id"),
                    (
                        parent,
                        value(&meta, "kind"),
                        text(&self.doc, id).to_string(),
                        tree.children(id)
                            .unwrap_or_default()
                            .into_iter()
                            .map(|child| value(&tree.get_meta(child).expect("child"), "id"))
                            .collect(),
                    ),
                )
            })
            .collect()
    }
}
fn value(map: &::loro::LoroMap, key: &str) -> String {
    match map.get(key).expect("field") {
        ValueOrContainer::Value(::loro::LoroValue::String(s)) => s.to_string(),
        _ => panic!("string"),
    }
}
fn text(doc: &LoroDoc, id: TreeID) -> LoroText {
    doc.get_tree("nodes")
        .get_meta(id)
        .expect("meta")
        .get("text")
        .expect("text")
        .into_container()
        .expect("container")
        .into_text()
        .expect("text")
}
