use super::*;
use ::yrs::updates::decoder::Decode;
use ::yrs::{
    Any, Doc, GetString, Map, MapPrelim, MapRef, ReadTxn, StateVector, Text, TextPrelim, Transact,
    Update,
};
#[derive(Debug)]
pub struct Yrs {
    doc: Doc,
    import_ms: f64,
    order: u64,
    index_ms: f64,
}
impl Backend for Yrs {
    fn new(peer: u64) -> Self {
        Self {
            doc: Doc::with_options(::yrs::Options {
                client_id: ::yrs::block::ClientID::new(peer),
                offset_kind: ::yrs::OffsetKind::Utf16,
                ..::yrs::Options::default()
            }),
            import_ms: 0.0,
            order: 0,
            index_ms: 0.0,
        }
    }
    fn apply(&mut self, edit: &Edit) {
        self.order += 1;
        let nodes = self.doc.get_or_insert_map("nodes");
        let mut txn = self.doc.transact_mut();
        match edit {
            Edit::Create {
                id,
                parent,
                kind,
                text,
            } => {
                let node: MapRef = nodes.insert(&mut txn, id.as_str(), MapPrelim::default());
                node.insert(&mut txn, "parent", parent.as_str());
                node.insert(&mut txn, "kind", *kind);
                node.insert(&mut txn, "alive", true);
                node.insert(&mut txn, "order", self.order as f64);
                node.insert(&mut txn, "text", TextPrelim::new(*text));
            }
            Edit::Delete(id) => {
                node(&nodes, &txn, id).insert(&mut txn, "alive", false);
            }
            Edit::Move { id, parent } => {
                let node = node(&nodes, &txn, id);
                node.insert(&mut txn, "parent", parent.as_str());
                node.insert(&mut txn, "order", self.order as f64);
            }
            Edit::Insert(id) => {
                let text: ::yrs::TextRef = node(&nodes, &txn, id)
                    .get(&txn, "text")
                    .expect("text")
                    .cast()
                    .expect("text");
                text.insert(&mut txn, 0, "é");
            }
            Edit::Remove(id) => {
                let text: ::yrs::TextRef = node(&nodes, &txn, id)
                    .get(&txn, "text")
                    .expect("text")
                    .cast()
                    .expect("text");
                text.remove_range(&mut txn, 0, 1);
            }
        }
    }
    fn snapshot(&self) -> Vec<u8> {
        self.updates()
    }
    fn updates(&self) -> Vec<u8> {
        self.doc
            .transact()
            .encode_state_as_update_v1(&StateVector::default())
    }
    fn import(&mut self, bytes: &[u8]) {
        let start = std::time::Instant::now();
        self.doc
            .transact_mut()
            .apply_update(Update::decode_v1(bytes).expect("decode"))
            .expect("import");
        self.import_ms = start.elapsed().as_secs_f64() * 1000.0;
        let start = std::time::Instant::now();
        let nodes = self.doc.get_or_insert_map("nodes");
        let txn = self.doc.transact();
        self.order = nodes
            .iter(&txn)
            .map(|(_, value)| {
                let node: MapRef = value.cast().expect("node");
                number(&node, &txn)
            })
            .max()
            .unwrap_or(0);
        self.index_ms = start.elapsed().as_secs_f64() * 1000.0;
    }
    fn last_import_ms(&self) -> (f64, f64) {
        (self.import_ms, self.index_ms)
    }
    fn state(&self) -> State {
        let nodes = self.doc.get_or_insert_map("nodes");
        let txn = self.doc.transact();
        let mut children: BTreeMap<String, Vec<(u64, String)>> = BTreeMap::new();
        for (id, value) in nodes.iter(&txn) {
            let node: MapRef = value.cast().expect("node");
            if node.get(&txn, "alive").expect("alive") == ::yrs::Out::Any(Any::Bool(true)) {
                let parent = node.get(&txn, "parent").expect("parent").to_string(&txn);
                children
                    .entry(parent)
                    .or_default()
                    .push((number(&node, &txn), id.to_owned()));
            }
        }
        for values in children.values_mut() {
            values.sort();
        }
        nodes
            .iter(&txn)
            .filter_map(|(id, value)| {
                let node: MapRef = value.cast().expect("node");
                if node.get(&txn, "alive").expect("alive") != ::yrs::Out::Any(Any::Bool(true)) {
                    return None;
                }
                let parent = node.get(&txn, "parent").expect("parent").to_string(&txn);
                let kind = node.get(&txn, "kind").expect("kind").to_string(&txn);
                let text: ::yrs::TextRef =
                    node.get(&txn, "text").expect("text").cast().expect("text");
                Some((
                    id.to_string(),
                    (
                        parent,
                        kind,
                        text.get_string(&txn),
                        children
                            .remove(id)
                            .unwrap_or_default()
                            .into_iter()
                            .map(|(_, id)| id)
                            .collect(),
                    ),
                ))
            })
            .collect()
    }
}
fn node(nodes: &MapRef, txn: &impl ReadTxn, id: &str) -> MapRef {
    nodes.get(txn, id).expect("node").cast().expect("map")
}

fn number(node: &MapRef, txn: &impl ReadTxn) -> u64 {
    match node.get(txn, "order").expect("order") {
        ::yrs::Out::Any(Any::Number(value)) => value as u64,
        _ => panic!("numeric order"),
    }
}
