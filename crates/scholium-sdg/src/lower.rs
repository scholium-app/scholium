use crate::{
    Error,
    codec::key,
    graph::{self, Graph},
    schema::{Record, Shape, flatten},
};
use loro::{LoroDoc, TreeID};
use scholium_model::{NodeId, structured::StructuredDocument};
use std::collections::{HashMap, HashSet};

pub(crate) fn bootstrap(doc: &LoroDoc, initial: &StructuredDocument) -> Result<(), Error> {
    doc.get_tree("sdg").enable_fractional_index(0);
    let fallback = graph::reserve(&initial.document, "document");
    graph::create(
        doc,
        fallback,
        None,
        &Shape::Block(scholium_model::BlockKind::Paragraph),
        "repair_reserve",
        false,
    )?;
    let schema = doc.get_map("schema");
    schema.insert("document", key(&initial.document))?;
    schema.insert("fallback", key(&fallback))?;
    schema.insert("version", "1")?;
    let records = flatten(initial);
    let mut ids = HashMap::new();
    for r in &records {
        let parent = r.parent.and_then(|p| ids.get(&p).copied());
        let id = graph::create(doc, r.node, parent, &r.shape, r.slot, true)?;
        graph::text(&doc.get_tree("sdg").get_meta(id)?)?.insert(0, &r.text)?;
        ids.insert(r.node, id);
    }
    doc.commit();
    Ok(())
}
pub(crate) fn apply(
    doc: &LoroDoc,
    before: &StructuredDocument,
    after: &StructuredDocument,
    origin: &str,
    action: scholium_model::RequestId,
) -> Result<(), Error> {
    let graph = Graph::read(doc)?;
    let mut ids: HashMap<_, _> = graph
        .entries
        .iter()
        .map(|(&node, e)| (node, e.tree))
        .collect();
    let records = flatten(after);
    let before_ids: HashSet<_> = flatten(before).iter().map(|r| r.node).collect();
    // Births are retained, undo-excluded identities; activation/moves/text are the
    // user transaction. Redo must not ask Loro to recreate metadata containers.
    doc.set_next_commit_origin("system/identity");
    for r in &records {
        if let Some(&id) = ids.get(&r.node) {
            if !before_ids.contains(&r.node) {
                return Err(Error::Identity);
            }
            ensure_fallback(doc, id, r, action)?;
        } else {
            let id = graph::create(doc, r.node, None, &r.shape, "reserve", false)?;
            ids.insert(r.node, id);
        }
    }
    crate::reserve::promote(doc, &graph, &records, action)?;
    doc.commit();
    doc.set_next_commit_origin(origin);
    for r in &records {
        let id = ids[&r.node];
        update(doc, id, r)?;
        // Parent-first traversal breaks old ancestry before relocating descendants.
        doc.get_tree("sdg")
            .mov_to(id, parent(&ids, r.parent)?, r.index)?;
    }
    let survivors: HashSet<_> = records.iter().map(|r| r.node).collect();
    for old in flatten(before).into_iter().rev() {
        if !survivors.contains(&old.node) {
            if graph.get(old.node)?.reserved {
                doc.get_tree("sdg")
                    .get_meta(ids[&old.node])?
                    .insert("active", false)?;
                doc.get_tree("sdg").mov(ids[&old.node], None)?;
            } else {
                doc.get_tree("sdg").delete(ids[&old.node])?;
            }
        }
    }
    doc.commit();
    Ok(())
}
fn parent(ids: &HashMap<NodeId, TreeID>, parent: Option<NodeId>) -> Result<Option<TreeID>, Error> {
    parent
        .map(|p| ids.get(&p).copied().ok_or(Error::Identity))
        .transpose()
}
fn update(doc: &LoroDoc, id: TreeID, r: &Record) -> Result<(), Error> {
    let meta = doc.get_tree("sdg").get_meta(id)?;
    meta.insert("shape", serde_json::to_string(&r.shape)?)?;
    meta.insert("slot", r.slot)?;
    meta.insert("active", true)?;
    let text = graph::text(&meta)?;
    // Preserve unchanged character identities; update uses Unicode scalar indexing.
    text.update(&r.text, Default::default())
        .map_err(|_| Error::Engine)?;
    Ok(())
}
fn ensure_fallback(
    doc: &LoroDoc,
    id: TreeID,
    r: &Record,
    action: scholium_model::RequestId,
) -> Result<(), Error> {
    let meta = doc.get_tree("sdg").get_meta(id)?;
    if r.shape.needs_fallback() && meta.get("fallback").is_none() {
        let fallback = graph::reserve(&r.node, &format!("fallback/{}", key(&action)));
        let shape = if r.shape.is_inline() {
            Shape::Hole
        } else {
            Shape::Text(scholium_model::structured::TextStyle::Plain)
        };
        graph::create(doc, fallback, None, &shape, "repair_reserve", false)?;
        meta.insert("fallback", key(&fallback))?;
    }
    Ok(())
}
