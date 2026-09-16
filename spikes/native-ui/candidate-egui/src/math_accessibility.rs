//! Spoken structural projection beside the editable text projection.
use super::*;
use egui::accesskit;

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
    pub(super) fn accessible_math(&self, ui: &mut egui::Ui, body: egui::Id) {
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
    fn description_preserves_nested_slots_and_matrix_cell_order() {
        let ctx = egui::Context::default();
        let mut app = SpikeApp::new(&ctx, None);
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
