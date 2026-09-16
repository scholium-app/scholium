//! Existing spike application, staged on a private transaction candidate.
use crate::reconcile::Reconciled;
use scholium_spike_core::{ActorId, Editor, Intent, NodeId, NodeKind, SemanticEdit};

pub(crate) fn apply(editor: &mut Editor, outcome: &Reconciled) -> Result<String, String> {
    match outcome {
        Reconciled::Unchanged => Ok("无改动".into()),
        Reconciled::ReplaceText {
            node,
            start,
            end,
            text,
        } => {
            editor
                .apply_batch(
                    ActorId(1),
                    Intent::Typing,
                    &[
                        SemanticEdit::DeleteRange {
                            node: *node,
                            start: *start,
                            end: *end,
                        },
                        SemanticEdit::InsertText {
                            node: *node,
                            at: *start,
                            text: text.clone(),
                        },
                    ],
                )
                .map_err(|error| error.to_string())?;
            Ok(format!("替换为 {text:?}"))
        }
        Reconciled::Wrap { node, structure } => {
            let (name, _) = structure.split_once('|').ok_or("结构描述损坏")?;
            let kind = match name {
                "sqrt" => NodeKind::Sqrt,
                "frac" => NodeKind::Fraction,
                other => return Err(format!("不支持的结构 {other}")),
            };
            editor
                .apply(
                    ActorId(1),
                    Intent::Typing,
                    SemanticEdit::Wrap { node: *node, kind },
                )
                .map_err(|error| format!("包裹失败：{error}"))?;
            Ok(format!("包裹为 {name}"))
        }
        Reconciled::ReplaceWithRaw {
            node,
            start,
            end,
            text,
            parent,
            slot,
            index,
        } => {
            let leaf = editor
                .document()
                .text_of(*node)
                .map_err(|error| error.to_string())?
                .to_string();
            let suffix = leaf.get(*end..).unwrap_or("").to_string();
            let after = index + 1;

            // 1) 后缀另起一个文本节点（先插，后面 Raw 插在它前面）。
            if !suffix.is_empty() {
                editor
                    .apply(
                        ActorId(1),
                        Intent::Typing,
                        SemanticEdit::InsertNode {
                            parent: *parent,
                            slot: *slot,
                            index: after,
                            kind: NodeKind::Text,
                        },
                    )
                    .map_err(|error| format!("建后缀节点失败：{error}"))?;
                let tail = child_at(editor, *parent, *slot, after);
                editor
                    .apply(
                        ActorId(1),
                        Intent::Typing,
                        SemanticEdit::InsertText {
                            node: tail,
                            at: 0,
                            text: suffix,
                        },
                    )
                    .map_err(|error| format!("写后缀失败：{error}"))?;
            }

            // 2) Raw 节点插在叶子之后。
            editor
                .apply(
                    ActorId(1),
                    Intent::Typing,
                    SemanticEdit::InsertNode {
                        parent: *parent,
                        slot: *slot,
                        index: after,
                        kind: NodeKind::Raw,
                    },
                )
                .map_err(|error| format!("建 Raw 失败：{error}"))?;
            let raw = child_at(editor, *parent, *slot, after);
            editor
                .apply(
                    ActorId(1),
                    Intent::Typing,
                    SemanticEdit::InsertText {
                        node: raw,
                        at: 0,
                        text: text.clone(),
                    },
                )
                .map_err(|error| format!("写 Raw 失败：{error}"))?;

            // 3) 叶子只保留前缀。
            editor
                .apply(
                    ActorId(1),
                    Intent::Typing,
                    SemanticEdit::DeleteRange {
                        node: *node,
                        start: *start,
                        end: leaf.len(),
                    },
                )
                .map_err(|error| format!("裁剪前缀失败：{error}"))?;
            Ok(format!("拆分为前缀 + Raw({text:?}) + 后缀"))
        }
        Reconciled::Conflict { reason } => Err(reason.clone()),
    }
}

fn child_at(editor: &Editor, parent: NodeId, slot: usize, index: usize) -> NodeId {
    editor.document().slot(parent, slot).expect("槽位存在")[index]
}
