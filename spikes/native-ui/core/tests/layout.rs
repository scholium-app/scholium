//! 结构渲染验收测试。
//!
//! 这些测试针对一个此前被漏掉的问题：正文只投影成纯文本时，"数学结构"这一项验收
//! 退化成模型层测试，候选之间也无法比较。这里断言**布局真的画出了结构**。

use scholium_spike_core::layout::{Item, Metrics, layout_document, layout_node};
use scholium_spike_core::{ActorId, Editor, Intent, NodeId, NodeKind, RemoteEdit, SemanticEdit};

const FIXTURE: ActorId = ActorId(99);

#[test]
fn math_slots_render_every_child() {
    for kind in [
        NodeKind::Fraction,
        NodeKind::Sqrt,
        NodeKind::Script,
        NodeKind::Delimited,
    ] {
        let mut editor = build_editor();
        let paragraph = slot_child(&editor, editor.document().root(), 0, 0);
        let math = slot_child(&editor, paragraph, 0, 1);
        let structure = create(&mut editor, math, 0, 0, kind);
        let slots = editor.document().node(structure).expect("node").slots.len();
        for slot in 0..slots {
            let first = slot_child(&editor, structure, slot, 0);
            type_text(&mut editor, first, "a");
            let second = create(&mut editor, structure, slot, 1, NodeKind::Text);
            type_text(&mut editor, second, "WIDE");
        }
        let layout = layout_node(editor.document(), structure, Metrics::default());
        assert_eq!(
            texts(&layout)
                .iter()
                .filter(|item| item.3 == "WIDE")
                .count(),
            slots
        );
    }
}

#[test]
fn nested_sqrt_aligns_sign_with_fraction_baseline() {
    let mut editor = build_editor();
    let paragraph = slot_child(&editor, editor.document().root(), 0, 0);
    let math = slot_child(&editor, paragraph, 0, 1);
    let sqrt = create(&mut editor, math, 0, 0, NodeKind::Sqrt);
    let fraction = create(&mut editor, sqrt, 0, 1, NodeKind::Fraction);
    let numerator = slot_child(&editor, fraction, 0, 0);
    let denominator = slot_child(&editor, fraction, 1, 0);
    type_text(&mut editor, numerator, "a");
    type_text(&mut editor, denominator, "b");
    let layout = layout_node(editor.document(), sqrt, Metrics::default());
    let sign = texts(&layout)
        .into_iter()
        .find(|item| item.3 == "\u{221a}")
        .expect("radical");
    assert!((sign.1 - layout.baseline).abs() < 0.01);
    assert!(
        layout.caret(denominator, 1).is_some(),
        "second slot child must be visible"
    );
}

fn build_editor() -> Editor {
    let mut editor = Editor::new();
    let paragraph = {
        let doc = editor.document();
        doc.slot(doc.root(), 0)
            .expect("根槽位")
            .first()
            .copied()
            .expect("段落")
    };
    let math = create(&mut editor, paragraph, 0, 1, NodeKind::Math);
    let _ = math;
    editor
}

fn create(
    editor: &mut Editor,
    parent: NodeId,
    slot: usize,
    index: usize,
    kind: NodeKind,
) -> NodeId {
    editor
        .apply_remote(RemoteEdit {
            actor: FIXTURE,
            edit: SemanticEdit::InsertNode {
                parent,
                slot,
                index,
                kind,
            },
        })
        .expect("插入结构")
        .created
        .expect("新节点")
}

fn type_text(editor: &mut Editor, node: NodeId, text: &str) {
    editor
        .apply_remote(RemoteEdit {
            actor: FIXTURE,
            edit: SemanticEdit::InsertText {
                node,
                at: 0,
                text: text.to_string(),
            },
        })
        .expect("插入文本");
}

fn slot_child(editor: &Editor, node: NodeId, slot: usize, index: usize) -> NodeId {
    editor.document().slot(node, slot).expect("槽位")[index]
}

fn texts(layout: &scholium_spike_core::Layout) -> Vec<(f32, f32, f32, String)> {
    layout
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Text {
                x,
                baseline,
                size,
                content,
                ..
            } => Some((*x, *baseline, *size, content.clone())),
            Item::Rule { .. } => None,
        })
        .collect()
}

fn rules(layout: &scholium_spike_core::Layout) -> Vec<(f32, f32, f32)> {
    layout
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Rule {
                y, width, height, ..
            } => Some((*y, *width, *height)),
            Item::Text { .. } => None,
        })
        .collect()
}

#[test]
fn fraction_draws_a_bar_between_numerator_and_denominator() {
    let mut editor = build_editor();
    let doc = editor.document();
    let paragraph = doc.slot(doc.root(), 0).expect("根槽位")[0];
    let math = doc.slot(paragraph, 0).expect("段落槽位")[1];
    let fraction = create(&mut editor, math, 0, 0, NodeKind::Fraction);
    let numerator = slot_child(&editor, fraction, 0, 0);
    let denominator = slot_child(&editor, fraction, 1, 0);
    type_text(&mut editor, numerator, "a");
    type_text(&mut editor, denominator, "b");

    let layout = layout_node(editor.document(), fraction, Metrics::default());
    let bars = rules(&layout);
    assert_eq!(bars.len(), 1, "分数必须画出恰好一条分数线");

    let numerator_item = texts(&layout)
        .into_iter()
        .find(|(_, _, _, content)| content == "a")
        .expect("分子文本");
    let denominator_item = texts(&layout)
        .into_iter()
        .find(|(_, _, _, content)| content == "b")
        .expect("分母文本");

    let (bar_y, bar_width, _) = bars[0];
    assert!(
        numerator_item.1 < bar_y,
        "分子基线必须在分数线之上：分子 {} 线 {}",
        numerator_item.1,
        bar_y
    );
    assert!(
        denominator_item.1 > bar_y,
        "分母基线必须在分数线之下：分母 {} 线 {}",
        denominator_item.1,
        bar_y
    );
    assert!(bar_width > 0.0, "分数线必须有宽度");
}

#[test]
fn script_raises_superscript_and_lowers_subscript() {
    let mut editor = build_editor();
    let doc = editor.document();
    let paragraph = doc.slot(doc.root(), 0).expect("根槽位")[0];
    let math = doc.slot(paragraph, 0).expect("段落槽位")[1];
    let script = create(&mut editor, math, 0, 0, NodeKind::Script);
    let base = slot_child(&editor, script, 0, 0);
    let sub = slot_child(&editor, script, 1, 0);
    let sup = slot_child(&editor, script, 2, 0);
    type_text(&mut editor, base, "x");
    type_text(&mut editor, sub, "1");
    type_text(&mut editor, sup, "2");

    let layout = layout_node(editor.document(), script, Metrics::default());
    let items = texts(&layout);
    let find = |content: &str| {
        items
            .iter()
            .find(|(_, _, _, text)| text == content)
            .cloned()
            .unwrap_or_else(|| panic!("缺少文本 {content}"))
    };
    let (_, base_baseline, base_size, _) = find("x");
    let (_, sub_baseline, sub_size, _) = find("1");
    let (_, sup_baseline, sup_size, _) = find("2");

    assert!(
        sup_baseline < base_baseline,
        "上标基线必须高于底：上标 {sup_baseline} 底 {base_baseline}"
    );
    assert!(
        sub_baseline > base_baseline,
        "下标基线必须低于底：下标 {sub_baseline} 底 {base_baseline}"
    );
    // 排版惯例：上标抬约 0.42em、下标降约 0.20em，不能偏离到一整行。
    let sup_shift = base_baseline - sup_baseline;
    let sub_shift = sub_baseline - base_baseline;
    assert!(
        (sup_shift - 0.42 * base_size).abs() < 0.5,
        "上标抬高应为 0.42em（{}），实测 {sup_shift}",
        0.42 * base_size
    );
    assert!(
        (sub_shift - 0.20 * base_size).abs() < 0.5,
        "下标降低应为 0.20em（{}），实测 {sub_shift}",
        0.20 * base_size
    );
    assert!(
        sub_size < base_size && sup_size < base_size,
        "上下标字号必须小于底：{sub_size} / {sup_size} vs {base_size}"
    );
}

#[test]
fn matrix_places_cells_on_a_grid() {
    let mut editor = build_editor();
    let doc = editor.document();
    let paragraph = doc.slot(doc.root(), 0).expect("根槽位")[0];
    let math = doc.slot(paragraph, 0).expect("段落槽位")[1];
    let matrix = create(&mut editor, math, 0, 0, NodeKind::Matrix);
    let first = slot_child(&editor, matrix, 0, 0);
    type_text(&mut editor, first, "1");
    let second = create(&mut editor, matrix, 0, 1, NodeKind::Text);
    type_text(&mut editor, second, "2");
    let third = create(&mut editor, matrix, 0, 2, NodeKind::Text);
    type_text(&mut editor, third, "3");

    let layout = layout_node(editor.document(), matrix, Metrics::default());
    let items = texts(&layout);
    assert_eq!(items.len(), 3, "三个单元格都应参与布局");

    let position = |content: &str| {
        items
            .iter()
            .find(|(_, _, _, text)| text == content)
            .map(|(x, y, _, _)| (*x, *y))
            .unwrap_or_else(|| panic!("缺少文本 {content}"))
    };
    let (x1, y1) = position("1");
    let (x2, y2) = position("2");
    let (_, y3) = position("3");

    assert!(x2 > x1 && (y2 - y1).abs() < 0.5, "第 2 格应在同一行右侧");
    assert!(y3 > y1, "第 3 格应在下一行（2 列网格）");
}

#[test]
fn sqrt_draws_radical_and_overline() {
    let mut editor = build_editor();
    let doc = editor.document();
    let paragraph = doc.slot(doc.root(), 0).expect("根槽位")[0];
    let math = doc.slot(paragraph, 0).expect("段落槽位")[1];
    let sqrt = create(&mut editor, math, 0, 0, NodeKind::Sqrt);
    let radicand = slot_child(&editor, sqrt, 0, 0);
    type_text(&mut editor, radicand, "y");

    let layout = layout_node(editor.document(), sqrt, Metrics::default());
    assert_eq!(rules(&layout).len(), 1, "根式必须有上横线");
    assert!(
        texts(&layout)
            .iter()
            .any(|(_, _, _, text)| text == "\u{221A}"),
        "根式必须有根号符号"
    );
}

#[test]
fn document_layout_covers_whole_graph() {
    let mut editor = build_editor();
    let text = {
        let doc = editor.document();
        doc.first_text_descendant(doc.root()).expect("文本叶子")
    };
    type_text(&mut editor, text, "示例");
    let layout = layout_document(editor.document());
    assert!(
        layout.width > 0.0 && layout.height > 0.0,
        "文档布局不能是空尺寸"
    );
    assert!(!layout.items.is_empty(), "文档布局必须产生图元");
}

#[test]
fn undo_after_typing_keeps_layout_consistent() {
    let mut editor = build_editor();
    let text = {
        let doc = editor.document();
        doc.first_text_descendant(doc.root()).expect("文本叶子")
    };
    editor
        .apply(
            ActorId(1),
            Intent::Typing,
            SemanticEdit::InsertText {
                node: text,
                at: 0,
                text: "abc".to_string(),
            },
        )
        .expect("输入");
    let before = layout_document(editor.document());
    editor.undo(ActorId(1)).expect("撤销");
    let after = layout_document(editor.document());
    assert!(
        after.width < before.width,
        "撤销后布局宽度应变小：{} → {}",
        before.width,
        after.width
    );
}

#[test]
fn caret_maps_byte_offset_to_layout_position() {
    let mut editor = build_editor();
    let text = {
        let doc = editor.document();
        doc.first_text_descendant(doc.root()).expect("文本叶子")
    };
    type_text(&mut editor, text, "ab");

    let layout = layout_document(editor.document());
    let start = layout.caret(text, 0).expect("文本开头有光标");
    let end = layout.caret(text, 2).expect("文本末尾有光标");

    assert!(start.x.abs() < 0.01, "开头光标应在最左：{}", start.x);
    assert!(
        end.x > start.x,
        "末尾光标应在右侧：{} vs {}",
        end.x,
        start.x
    );
    assert!(
        (start.baseline - end.baseline).abs() < 0.01,
        "同一行光标基线应一致"
    );
    assert!(
        (end.x - 24.0).abs() < 0.01,
        "两个 ASCII 字符宽 1.2em：{}",
        end.x
    );
}

#[test]
fn caret_is_none_for_non_text_node() {
    let editor = build_editor();
    let layout = layout_document(editor.document());
    let root = editor.document().root();
    assert!(layout.caret(root, 0).is_none(), "非文本节点不应产出光标");
}

#[test]
fn caret_inside_fraction_lands_inside_that_fraction() {
    let mut editor = build_editor();
    let doc = editor.document();
    let paragraph = doc.slot(doc.root(), 0).expect("根槽位")[0];
    let math = doc.slot(paragraph, 0).expect("段落槽位")[1];
    let fraction = create(&mut editor, math, 0, 0, NodeKind::Fraction);
    let numerator = slot_child(&editor, fraction, 0, 0);
    let denominator = slot_child(&editor, fraction, 1, 0);
    type_text(&mut editor, numerator, "12");
    type_text(&mut editor, denominator, "3");

    let layout = layout_document(editor.document());
    let numerator_caret = layout.caret(numerator, 1).expect("分子光标");
    let denominator_caret = layout.caret(denominator, 0).expect("分母光标");
    assert!(
        numerator_caret.baseline < denominator_caret.baseline,
        "分子光标基线应高于分母：{} vs {}",
        numerator_caret.baseline,
        denominator_caret.baseline
    );
}

#[test]
fn hit_test_maps_point_to_node_and_byte_offset() {
    let mut editor = build_editor();
    let text = {
        let doc = editor.document();
        doc.first_text_descendant(doc.root()).expect("文本叶子")
    };
    type_text(&mut editor, text, "ab");

    let layout = layout_document(editor.document());
    let caret_end = layout.caret(text, 2).expect("末尾光标");
    let top = Item::top_of(caret_end.baseline, caret_end.size);

    // 最左端 → 偏移 0；越过最后一个字符中点 → 偏移 2。
    assert_eq!(layout.hit_test(-100.0, top + 2.0), Some((text, 0)));
    assert_eq!(
        layout.hit_test(caret_end.x + 100.0, top + 2.0),
        Some((text, 2))
    );
}

#[test]
fn hit_test_inside_fraction_selects_that_slot() {
    let mut editor = build_editor();
    let doc = editor.document();
    let paragraph = doc.slot(doc.root(), 0).expect("根槽位")[0];
    let math = doc.slot(paragraph, 0).expect("段落槽位")[1];
    let fraction = create(&mut editor, math, 0, 0, NodeKind::Fraction);
    let numerator = slot_child(&editor, fraction, 0, 0);
    let denominator = slot_child(&editor, fraction, 1, 0);
    type_text(&mut editor, numerator, "1");
    type_text(&mut editor, denominator, "2");

    let layout = layout_document(editor.document());
    let numerator_caret = layout.caret(numerator, 1).expect("分子光标");
    let denominator_caret = layout.caret(denominator, 1).expect("分母光标");

    let numerator_top = Item::top_of(numerator_caret.baseline, numerator_caret.size);
    let denominator_top = Item::top_of(denominator_caret.baseline, denominator_caret.size);

    assert_eq!(
        layout.hit_test(numerator_caret.x - 100.0, numerator_top + 2.0),
        Some((numerator, 0))
    );
    assert_eq!(
        layout.hit_test(denominator_caret.x - 100.0, denominator_top + 2.0),
        Some((denominator, 0))
    );
}

#[test]
fn hit_test_outside_any_text_is_none() {
    let mut editor = build_editor();
    let text = {
        let doc = editor.document();
        doc.first_text_descendant(doc.root()).expect("文本叶子")
    };
    type_text(&mut editor, text, "a");
    let layout = layout_document(editor.document());
    assert!(layout.hit_test(0.0, -500.0).is_none(), "远离文本不应命中");
}

#[test]
fn pointer_hit_never_splits_combining_or_zwj_graphemes() {
    let mut editor = build_editor();
    let text = editor
        .document()
        .first_text_descendant(editor.document().root())
        .expect("leaf");
    type_text(&mut editor, text, "e\u{301}👩‍💻中");
    let layout = layout_document(editor.document());
    let caret = layout.caret(text, 0).expect("caret");
    for x in 0..150 {
        if let Some((node, byte)) = layout.hit_test(x as f32, caret.baseline) {
            assert!(
                editor
                    .document()
                    .node(node)
                    .expect("node")
                    .text
                    .is_grapheme_boundary(byte),
                "x={x}, byte={byte}"
            );
        }
    }
}
