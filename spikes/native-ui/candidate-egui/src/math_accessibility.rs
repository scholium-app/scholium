//! Spoken structural projection beside the editable text projection.
use super::*;
use egui::accesskit;

fn slot_name(kind: NodeKind, slot: usize, index: usize) -> Option<String> {
    let name = match kind {
        NodeKind::Fraction => ["分数分子", "分数分母"].get(slot).copied()?,
        NodeKind::Script => ["底数", "下标", "上标"].get(slot).copied()?,
        NodeKind::Sqrt => "根式被开方项",
        NodeKind::Delimited => "定界结构内容",
        NodeKind::Matrix => return Some(format!("矩阵单元格 {}", index + 1)),
        _ => return None,
    };
    Some(name.to_owned())
}

fn cursor_description(doc: &scholium_spike_core::Document, cursor: Cursor) -> String {
    let mut current = cursor.focus();
    let mut path = Vec::new();
    if let Cursor::Slot { node, slot, index } = cursor
        && let Ok(node) = doc.node(node)
        && let Some(name) = slot_name(node.kind, slot, index)
    {
        path.push(if node.kind == NodeKind::Matrix {
            format!("矩阵插入位置 {}", index + 1)
        } else {
            name
        });
    }
    while let Some((parent, slot, index)) = doc.locate_in_parent(current) {
        let Ok(node) = doc.node(parent) else {
            return String::new();
        };
        if let Some(name) = slot_name(node.kind, slot, index) {
            path.push(name);
        }
        current = parent;
    }
    if path.is_empty() {
        return String::new();
    }
    path.reverse();
    let content = match cursor {
        Cursor::Text { node, .. } => describe(doc, node),
        Cursor::Slot { node, slot, .. } => doc.slot(node, slot).map_or(String::new(), |children| {
            children
                .iter()
                .map(|id| describe(doc, *id))
                .collect::<Vec<_>>()
                .join("，")
        }),
    };
    format!(
        "{}：{}",
        path.join("，"),
        if content.is_empty() { "空" } else { &content }
    )
}

fn describe(doc: &scholium_spike_core::Document, id: NodeId) -> String {
    let Ok(node) = doc.node(id) else {
        return String::new();
    };
    if node.kind.is_text() {
        return node.text.as_string();
    }
    let slots: Vec<_> = node
        .slots
        .iter()
        .map(|children| {
            children
                .iter()
                .map(|id| describe(doc, *id))
                .collect::<Vec<_>>()
                .join("，")
        })
        .collect();
    match node.kind {
        NodeKind::Fraction => format!("分数（分子：{}；分母：{}）", slots[0], slots[1]),
        NodeKind::Sqrt => format!("根式（被开方项：{}）", slots[0]),
        NodeKind::Script => format!(
            "上下标（底数：{}；下标：{}；上标：{}）",
            slots[0], slots[1], slots[2]
        ),
        NodeKind::Matrix => format!(
            "矩阵（{}）",
            node.slots[0]
                .iter()
                .enumerate()
                .map(|(i, id)| { format!("单元格 {}：{}", i + 1, describe(doc, *id)) })
                .collect::<Vec<_>>()
                .join("；")
        ),
        NodeKind::Delimited => format!("定界结构（{}）", slots[0]),
        _ => slots.join("；"),
    }
}

impl SpikeApp {
    pub(super) fn accessible_math(&self, ui: &mut egui::Ui, body: egui::Id, focused: bool) {
        // Keep one live node across frames. Only semantic context changes announce;
        // ordinary character motion remains the editable Text interface's job.
        let status = body.with("math-cursor");
        drop(ui.new_child(egui::UiBuilder::new().id(status)));
        ui.ctx().accesskit_node_builder(status, |out| {
            out.set_role(accesskit::Role::Status);
            out.set_live(accesskit::Live::Polite);
            out.set_label(if focused {
                cursor_description(self.core.document(), self.focus)
            } else {
                String::new()
            });
        });
        let mut stack = vec![self.core.document().root()];
        while let Some(id) = stack.pop() {
            let Ok(node) = self.core.document().node(id) else {
                continue;
            };
            if node.kind != NodeKind::Math {
                stack.extend(node.slots.iter().flatten().rev().copied());
                continue;
            }
            // Keep the editable TextRun order intact. Math objects are sibling projections.
            let accessible_id = body.with("math").with(id.index());
            drop(ui.new_child(egui::UiBuilder::new().id(accessible_id)));
            ui.ctx().accesskit_node_builder(accessible_id, |out| {
                out.set_role(accesskit::Role::Math);
                out.set_label(format!("公式：{}", describe(self.core.document(), id)));
                out.set_description("正文语义图的数学结构；文本位置由正文结构编辑器提供");
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cursor_context_preserves_slot_ancestry_and_empty_content() {
        let ctx = egui::Context::default();
        let mut app = SpikeApp::new_layout_probe(&ctx, None);
        let root = app.core.document().root();
        let paragraph = app.core.document().slot(root, 0).expect("root")[0];
        let math = app.core.document().slot(paragraph, 0).expect("paragraph")[1];
        let fraction = app.core.document().slot(math, 0).expect("math")[0];
        let numerator = app.core.document().slot(fraction, 0).expect("numerator")[0];
        let cursor = Cursor::Text {
            node: numerator,
            byte: 0,
        };
        assert_eq!(
            cursor_description(app.core.document(), cursor),
            "分数分子：a"
        );
        app.core
            .apply(
                LOCAL,
                Intent::Structural,
                SemanticEdit::Wrap {
                    node: numerator,
                    kind: NodeKind::Sqrt,
                },
            )
            .expect("wrap numerator");
        assert_eq!(
            cursor_description(app.core.document(), cursor),
            "分数分子，根式被开方项：a"
        );
        app.core
            .apply(
                LOCAL,
                Intent::Typing,
                SemanticEdit::DeleteRange {
                    node: numerator,
                    start: 0,
                    end: 1,
                },
            )
            .expect("clear numerator");
        assert_eq!(
            cursor_description(app.core.document(), cursor),
            "分数分子，根式被开方项：空"
        );
        let outside = Cursor::Slot {
            node: paragraph,
            slot: 0,
            index: 0,
        };
        assert_eq!(cursor_description(app.core.document(), outside), "");
    }

    #[test]
    fn description_preserves_nested_slots_and_matrix_cell_order() {
        let ctx = egui::Context::default();
        let mut app = SpikeApp::new_layout_probe(&ctx, None);
        let math = app
            .core
            .document()
            .slot(app.core.document().root(), 0)
            .expect("root")[0];
        let math = app.core.document().slot(math, 0).expect("paragraph")[1];
        let fraction = app.core.document().slot(math, 0).expect("math")[0];
        let numerator = app.core.document().slot(fraction, 0).expect("numerator")[0];
        app.core
            .apply(
                LOCAL,
                Intent::Structural,
                SemanticEdit::Wrap {
                    node: numerator,
                    kind: NodeKind::Sqrt,
                },
            )
            .expect("wrap");
        let speech = describe(app.core.document(), math);
        assert!(speech.contains("分子：根式（被开方项：a）；分母：b"));
        assert!(speech.contains("底数：x；下标：1；上标：2"));
        assert!(speech.contains("单元格 1：1；单元格 2：2；单元格 3：3"));
    }
}
