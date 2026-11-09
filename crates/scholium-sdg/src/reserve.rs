//! Promote a used repair reserve to an ordinary stable node and replenish its slot.
use crate::{
    Error,
    codec::key,
    graph::{self, Graph},
    schema::{Record, Shape},
};
use loro::LoroDoc;
use scholium_model::{RequestId, structured::TextStyle};

pub(crate) fn promote(
    doc: &LoroDoc,
    graph: &Graph,
    records: &[Record],
    action: RequestId,
) -> Result<(), Error> {
    let mut owners: Vec<_> = graph.entries.iter().collect();
    owners.sort_by_key(|(id, _)| id.as_bytes());
    for record in records {
        let Some(entry) = graph.entries.get(&record.node).filter(|e| e.reserved) else {
            continue;
        };
        let tree = doc.get_tree("sdg");
        // Births and reserve replenishment are excluded from undo. The user's
        // activation/move/text remains one undoable transaction with the same ID.
        tree.get_meta(entry.tree)?.insert("reserved", false)?;
        let role = format!("fallback/{}", key(&action));
        if record.node == graph.fallback {
            let id = graph::reserve(&record.node, &format!("document/{role}"));
            graph::create(
                doc,
                id,
                None,
                &Shape::Block(scholium_model::BlockKind::Paragraph),
                "repair_reserve",
                false,
            )?;
            doc.get_map("schema").insert("fallback", key(&id))?;
        }
        for &(owner, e) in &owners {
            if e.fallback != Some(record.node) {
                continue;
            }
            let id = graph::reserve(&owner, &role);
            let shape = if e.shape.is_inline() {
                Shape::Hole
            } else {
                Shape::Text(TextStyle::Plain)
            };
            graph::create(doc, id, None, &shape, "repair_reserve", false)?;
            tree.get_meta(e.tree)?.insert("fallback", key(&id))?;
        }
    }
    Ok(())
}
