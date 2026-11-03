use crate::{Error, codec::key, schema::Shape};
use loro::{LoroDoc, LoroMap, LoroText, TreeID, ValueOrContainer};
use scholium_model::NodeId;
use std::collections::HashMap;

const MAX_ENGINE_NODES: usize = 400_000;

#[derive(Debug)]
pub(crate) struct Entry {
    pub tree: TreeID,
    pub shape: Shape,
    pub text: String,
    pub slot: String,
    pub children: Vec<NodeId>,
    pub fallback: Option<NodeId>,
    pub active: bool,
    pub reserved: bool,
    pub live: bool,
}
#[derive(Debug)]
pub(crate) struct Graph {
    pub entries: HashMap<NodeId, Entry>,
    pub roots: Vec<NodeId>,
    pub fallback: NodeId,
}
impl Graph {
    pub fn read(doc: &LoroDoc) -> Result<Self, Error> {
        let tree = doc.get_tree("sdg");
        let all = tree.nodes();
        if all.len() > MAX_ENGINE_NODES {
            return Err(Error::Capacity);
        }
        let mut ids = HashMap::new();
        let mut inverse = HashMap::new();
        for id in all {
            let node: NodeId = serde_json::from_str(&string(&tree.get_meta(id)?, "id")?)?;
            if inverse.insert(node, id).is_some() {
                return Err(Error::Identity);
            }
            ids.insert(id, node);
        }
        let entries = ids
            .iter()
            .map(|(&id, &node)| Ok((node, read_entry(doc, id, &ids)?)))
            .collect::<Result<HashMap<_, _>, Error>>()?;
        let roots = tree
            .roots()
            .iter()
            .map(|id| ids.get(id).copied().ok_or(Error::Format))
            .collect::<Result<_, _>>()?;
        let fallback = serde_json::from_str(&string(&doc.get_map("schema"), "fallback")?)?;
        if !entries.contains_key(&fallback) {
            return Err(Error::Format);
        }
        Ok(Self {
            entries,
            roots,
            fallback,
        })
    }
    pub fn get(&self, id: NodeId) -> Result<&Entry, Error> {
        self.entries.get(&id).ok_or(Error::Identity)
    }
}
fn read_entry(doc: &LoroDoc, id: TreeID, ids: &HashMap<TreeID, NodeId>) -> Result<Entry, Error> {
    let tree = doc.get_tree("sdg");
    let meta = tree.get_meta(id)?;
    let children = tree
        .children(id)
        .unwrap_or_default()
        .iter()
        .map(|c| ids.get(c).copied().ok_or(Error::Format))
        .collect::<Result<_, _>>()?;
    let fallback = meta
        .get("fallback")
        .map(|_| string(&meta, "fallback").and_then(|v| Ok(serde_json::from_str(&v)?)))
        .transpose()?;
    Ok(Entry {
        tree: id,
        shape: serde_json::from_str(&string(&meta, "shape")?)?,
        text: text(&meta)?.to_string(),
        slot: string(&meta, "slot")?,
        children,
        fallback,
        active: flag(&meta, "active"),
        reserved: flag(&meta, "reserved"),
        live: !tree.is_node_deleted(&id)?,
    })
}
fn flag(meta: &LoroMap, name: &str) -> bool {
    matches!(
        meta.get(name),
        Some(ValueOrContainer::Value(loro::LoroValue::Bool(true)))
    )
}
pub(crate) fn string(map: &LoroMap, key: &str) -> Result<String, Error> {
    match map.get(key) {
        Some(ValueOrContainer::Value(loro::LoroValue::String(s))) => Ok(s.to_string()),
        _ => Err(Error::Format),
    }
}
pub(crate) fn text(meta: &LoroMap) -> Result<LoroText, Error> {
    meta.get("text")
        .and_then(|v| v.into_container().ok())
        .and_then(|v| v.into_text().ok())
        .ok_or(Error::Format)
}
pub(crate) fn create(
    doc: &LoroDoc,
    node: NodeId,
    parent: Option<TreeID>,
    shape: &Shape,
    slot: &str,
    active: bool,
) -> Result<TreeID, Error> {
    let tree = doc.get_tree("sdg");
    let id = tree.create(parent)?;
    let meta = tree.get_meta(id)?;
    meta.insert("id", key(&node))?;
    meta.insert("shape", serde_json::to_string(shape)?)?;
    meta.insert("slot", slot)?;
    meta.insert("active", active)?;
    meta.insert("reserved", slot == "repair_reserve")?;
    meta.insert_container("text", LoroText::new())?;
    if shape.needs_fallback() {
        let fallback = reserve(&node, "fallback");
        let shape = if shape.is_inline() {
            Shape::Hole
        } else {
            Shape::Text(scholium_model::structured::TextStyle::Plain)
        };
        create(doc, fallback, None, &shape, "repair_reserve", false)?;
        meta.insert("fallback", key(&fallback))?;
    }
    Ok(id)
}

pub(crate) fn reserve<T: serde::Serialize>(owner: &T, role: &str) -> NodeId {
    use sha2::Digest;
    let hash =
        sha2::Sha256::digest(format!("scholium-sdg-reserve-v1/{role}/{}", key(owner)).as_bytes());
    let mut bytes = [0; 16];
    bytes.copy_from_slice(&hash[..16]);
    NodeId::from_bytes(bytes)
}
