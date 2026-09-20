//! Disposable spike interchange: semantic tree and unapplied source, never rendered output.
use super::super::*;
use scholium_spike_reconcile::{Session, generate::Dialect};
use serde::{Deserialize, Serialize};

const SCHEMA: &str = "scholium-ui-session-spike-v1";
const MAX_NODES: usize = 100_000;
const MAX_DEPTH: usize = 128;

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub(super) struct SnapshotError(String);

impl From<&str> for SnapshotError {
    fn from(value: &str) -> Self {
        Self(value.into())
    }
}

fn invalid(error: impl ToString) -> SnapshotError {
    SnapshotError(error.to_string())
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub(super) struct Snapshot {
    schema: String,
    nodes: Vec<SavedNode>,
    dialect: String,
    source: String,
    base: String,
    stale: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct SavedNode {
    parent: Option<usize>,
    slot: usize,
    kind: String,
    text: String,
    variant: u8,
}

impl Snapshot {
    pub fn capture(app: &SpikeApp) -> Result<Self, SnapshotError> {
        let mut nodes = Vec::new();
        capture_node(
            app.core.document(),
            app.core.document().root(),
            None,
            0,
            0,
            &mut nodes,
        )?;
        Ok(Self {
            schema: SCHEMA.into(),
            nodes,
            dialect: format!("{:?}", app.source.dialect),
            source: app.source_buffer.clone(),
            base: app.source.generated.text.clone(),
            stale: app.source.revision != app.core.revision(),
        })
    }

    pub fn restore(&self) -> Result<(Editor, Session, String), SnapshotError> {
        if self.schema != SCHEMA || self.nodes.is_empty() || self.nodes.len() > MAX_NODES {
            return Err("不是受支持的实验会话文件".into());
        }
        let dialect = match self.dialect.as_str() {
            "Latex" => Dialect::Latex,
            "Typst" => Dialect::Typst,
            _ => return Err("未知源码语言".into()),
        };
        let core = restore_tree(&self.nodes)?;
        let mut source = Session::new(&core, dialect);
        if self.stale || self.base != source.generated.text {
            // Old source maps cannot be trusted after node IDs are rebuilt.
            // Preserve the exact base/draft, but require explicit re-generation to apply.
            source.generated.text = self.base.clone();
            source.revision = 0;
        }
        Ok((core, source, self.source.clone()))
    }
}

fn capture_node(
    doc: &scholium_spike_core::Document,
    node: NodeId,
    parent: Option<usize>,
    slot: usize,
    depth: usize,
    nodes: &mut Vec<SavedNode>,
) -> Result<(), SnapshotError> {
    if nodes.len() >= MAX_NODES || depth > MAX_DEPTH {
        return Err("会话超过实验规模限制".into());
    }
    let n = doc.node(node).map_err(invalid)?;
    let index = nodes.len();
    nodes.push(SavedNode {
        parent,
        slot,
        kind: format!("{:?}", n.kind),
        text: n.text.as_string(),
        variant: n.variant,
    });
    for (slot, children) in n.slots.iter().enumerate() {
        for child in children {
            capture_node(doc, *child, Some(index), slot, depth + 1, nodes)?;
        }
    }
    Ok(())
}

fn kind(name: &str) -> Result<NodeKind, SnapshotError> {
    [
        NodeKind::Document,
        NodeKind::Paragraph,
        NodeKind::Heading,
        NodeKind::Text,
        NodeKind::Math,
        NodeKind::Fraction,
        NodeKind::Sqrt,
        NodeKind::Script,
        NodeKind::Delimited,
        NodeKind::Matrix,
        NodeKind::Raw,
    ]
    .into_iter()
    .find(|k| format!("{k:?}") == name)
    .ok_or_else(|| "未知节点类型".into())
}

fn clear_children(core: &mut Editor, node: NodeId) -> Result<(), SnapshotError> {
    let children: Vec<_> = core
        .document()
        .node(node)
        .map_err(invalid)?
        .slots
        .iter()
        .flatten()
        .copied()
        .collect();
    for node in children {
        apply(core, SemanticEdit::DetachNode { node })?;
    }
    Ok(())
}

fn apply(core: &mut Editor, edit: SemanticEdit) -> Result<(), SnapshotError> {
    core.apply(ActorId(0), Intent::External, edit)
        .map(|_| ())
        .map_err(invalid)
}

fn restore_tree(nodes: &[SavedNode]) -> Result<Editor, SnapshotError> {
    let mut core = Editor::new();
    let root = core.document().root();
    clear_children(&mut core, root)?;
    let mut ids = Vec::new();
    let mut depths = Vec::new();
    for (index, saved) in nodes.iter().enumerate() {
        let kind = kind(&saved.kind)?;
        let (id, depth) = if index == 0 {
            if kind != NodeKind::Document || saved.parent.is_some() {
                return Err("根节点无效".into());
            }
            (root, 0)
        } else {
            let p = saved.parent.filter(|p| *p < index).ok_or("父节点无效")?;
            if depths[p] >= MAX_DEPTH || kind == NodeKind::Document {
                return Err("节点深度/类型无效".into());
            }
            let id = insert_saved(&mut core, ids[p], saved, kind)?;
            (id, depths[p] + 1)
        };
        if saved.variant >= kind.variant_count().max(1)
            || (!kind.is_text() && !saved.text.is_empty())
        {
            return Err("节点内容无效".into());
        }
        for _ in 0..saved.variant {
            apply(&mut core, SemanticEdit::CycleVariant { node: id })?;
        }
        if !saved.text.is_empty() {
            apply(
                &mut core,
                SemanticEdit::InsertText {
                    node: id,
                    at: 0,
                    text: saved.text.clone(),
                },
            )?;
        }
        ids.push(id);
        depths.push(depth);
    }
    if core.document().first_text_descendant(root).is_none() {
        return Err("会话没有可编辑文本".into());
    }
    Ok(core)
}

fn insert_saved(
    core: &mut Editor,
    parent: NodeId,
    saved: &SavedNode,
    kind: NodeKind,
) -> Result<NodeId, SnapshotError> {
    let at = core
        .document()
        .slot(parent, saved.slot)
        .map_err(invalid)?
        .len();
    apply(
        core,
        SemanticEdit::InsertNode {
            parent,
            slot: saved.slot,
            index: at,
            kind,
        },
    )?;
    let id = core.document().slot(parent, saved.slot).map_err(invalid)?[at];
    clear_children(core, id)?;
    Ok(id)
}
