//! Native window fixture, injected through the existing semantic core.

use scholium_spike_core::action::{ActorId, Editor, RemoteEdit};
use scholium_spike_core::doc::NodeKind;
use scholium_spike_core::edit::SemanticEdit;
use scholium_spike_core::ids::NodeId;

pub(super) fn build() -> (Editor, NodeId) {
    let mut core = Editor::new();
    let paragraph = core.document().slot(core.document().root(), 0).unwrap()[0];
    let label = core.document().slot(paragraph, 0).unwrap()[0];
    insert(&mut core, label, "中文 English：");
    let math = create(&mut core, paragraph, NodeKind::Math, 1);
    let empty = core.document().slot(math, 0).unwrap()[0];
    core.apply_remote(RemoteEdit {
        actor: ActorId(99),
        edit: SemanticEdit::Wrap {
            node: empty,
            kind: NodeKind::Fraction,
        },
    })
    .unwrap();
    let fraction = core.document().node(empty).unwrap().parent.unwrap();
    let denominator = core.document().slot(fraction, 1).unwrap()[0];
    insert(&mut core, empty, "x");
    let suffix = create(&mut core, paragraph, NodeKind::Text, 2);
    insert(&mut core, suffix, "，直接编辑空分母。");
    (core, denominator)
}

fn create(core: &mut Editor, parent: NodeId, kind: NodeKind, index: usize) -> NodeId {
    core.apply_remote(RemoteEdit {
        actor: ActorId(99),
        edit: SemanticEdit::InsertNode {
            parent,
            slot: 0,
            index,
            kind,
        },
    })
    .unwrap()
    .created
    .unwrap()
}

fn insert(core: &mut Editor, node: NodeId, text: &str) {
    core.apply_remote(RemoteEdit {
        actor: ActorId(99),
        edit: SemanticEdit::InsertText {
            node,
            at: 0,
            text: text.into(),
        },
    })
    .unwrap();
}
