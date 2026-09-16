//! 模型层验收测试。
//!
//! 对应 `docs/NATIVE_UI_VALIDATION.md` 第 4 节验收表的可自动判定部分。真正需要真实窗口、
//! 输入法和辅助功能的项目不在本文件覆盖范围，它们在各候选的报告里标记为 Blocked。
//!
//! 命名描述行为而不是函数名（`AGENT.md` 测试规范）。

use scholium_spike_core::{
    ActorId, Cursor, Dialect, Direction, Document, EditError, Editor, Intent, NodeId, NodeKind,
    RemoteEdit, SemanticEdit, SourcePane,
};

const A: ActorId = ActorId(1);
const B: ActorId = ActorId(2);

/// 新建编辑器并返回（编辑器, 第一个文本叶子）。
fn editor_with_text() -> (Editor, NodeId) {
    let editor = Editor::new();
    let doc = editor.document();
    let text = doc
        .first_text_descendant(doc.root())
        .expect("空文档包含一个文本叶子");
    (editor, text)
}

/// 在段落末尾插入一个数学节点，返回其 NodeId。
fn insert_math(editor: &mut Editor) -> NodeId {
    let doc = editor.document();
    let root = doc.root();
    let paragraph = child_at(doc, root, 0, 0);
    let outcome = editor
        .apply(
            A,
            Intent::Structural,
            SemanticEdit::InsertNode {
                parent: paragraph,
                slot: 0,
                index: 1,
                kind: NodeKind::Math,
            },
        )
        .expect("插入数学节点");
    outcome.expect("结构编辑应产生动作");
    child_at(editor.document(), paragraph, 0, 1)
}

/// 在 `parent` 的第 `slot` 槽位插入结构节点并返回其 NodeId。
fn insert_child(editor: &mut Editor, parent: NodeId, slot: usize, kind: NodeKind) -> NodeId {
    editor
        .apply(
            A,
            Intent::Structural,
            SemanticEdit::InsertNode {
                parent,
                slot,
                index: 0,
                kind,
            },
        )
        .expect("插入结构节点")
        .expect("结构编辑应产生动作");
    child_at(editor.document(), parent, slot, 0)
}

fn type_text(editor: &mut Editor, node: NodeId, at: usize, text: &str) {
    editor
        .apply(
            A,
            Intent::Typing,
            SemanticEdit::InsertText {
                node,
                at,
                text: text.to_string(),
            },
        )
        .expect("插入文本")
        .expect("文本插入应产生动作");
}

fn text_at(editor: &Editor, node: NodeId) -> String {
    editor.document().text_of(node).expect("文本节点")
}

/// 取某节点某槽位的第 `at` 个子节点。
fn child_at(doc: &Document, node: NodeId, slot: usize, at: usize) -> NodeId {
    doc.slot(node, slot).expect("槽位存在")[at]
}

// ---------------------------------------------------------------- 中文与 Unicode

#[test]
fn ime_preedit_is_not_an_action() {
    let (mut editor, text) = editor_with_text();
    editor.preedit_update("wo");
    editor.preedit_update("wo fu");
    assert_eq!(editor.preedit(), "wo fu");
    assert_eq!(editor.history().len(), 0, "预编辑不得进入历史");
    assert_eq!(text_at(&editor, text), "");
}

#[test]
fn ime_commit_is_a_single_action() {
    let (mut editor, text) = editor_with_text();
    editor.preedit_update("我服了");
    let action = editor.preedit_commit(A, text, 0).expect("提交预编辑");
    assert!(action.is_some());
    assert_eq!(editor.preedit(), "", "提交后清空预编辑");
    assert_eq!(text_at(&editor, text), "我服了");
    assert_eq!(editor.history().len(), 1, "整串提交只产生一个动作");
}

#[test]
fn ime_cancel_leaves_no_trace() {
    let (mut editor, text) = editor_with_text();
    editor.preedit_update("w");
    editor.preedit_cancel();
    assert_eq!(editor.preedit(), "");
    assert_eq!(editor.history().len(), 0);
    assert_eq!(text_at(&editor, text), "");
}

#[test]
fn delete_backward_does_not_split_zwj_emoji() {
    let (mut editor, text) = editor_with_text();
    let family = "👨‍👩‍👧";
    type_text(&mut editor, text, 0, &format!("a{family}b"));
    let after_family = 1 + family.len();
    editor
        .apply(
            A,
            Intent::Typing,
            SemanticEdit::DeleteBackward {
                node: text,
                at: after_family,
            },
        )
        .expect("删除");
    assert_eq!(text_at(&editor, text), "ab", "整个 ZWJ 序列应作为一个字素删除");
}

#[test]
fn delete_backward_does_not_split_combining_mark() {
    let (mut editor, text) = editor_with_text();
    type_text(&mut editor, text, 0, "ae\u{301}b");
    // "ae\u{301}b" 的字节长度是 5，光标落在组合字符之后。
    editor
        .apply(
            A,
            Intent::Typing,
            SemanticEdit::DeleteBackward { node: text, at: 4 },
        )
        .expect("删除");
    assert_eq!(text_at(&editor, text), "ab");
}

#[test]
fn offset_inside_grapheme_is_rejected() {
    let (mut editor, text) = editor_with_text();
    type_text(&mut editor, text, 0, "e\u{301}");
    let err = editor
        .apply(
            A,
            Intent::Typing,
            SemanticEdit::InsertText {
                node: text,
                at: 1,
                text: "x".to_string(),
            },
        )
        .expect_err("组合字符中间的偏移必须被拒绝");
    assert!(matches!(err, EditError::NotGraphemeBoundary { .. }));
    assert_eq!(text_at(&editor, text), "e\u{301}", "拒绝后不得有部分修改");
}

// ---------------------------------------------------------------- 数学结构

#[test]
fn vertical_navigation_moves_between_numerator_and_denominator() {
    let (mut editor, _) = editor_with_text();
    let math = insert_math(&mut editor);
    let fraction = insert_child(&mut editor, math, 0, NodeKind::Fraction);
    let doc = editor.document();
    let numerator = child_at(doc, fraction, 0, 0);
    let denominator = child_at(doc, fraction, 1, 0);

    type_text(&mut editor, numerator, 0, "1");
    type_text(&mut editor, denominator, 0, "2");

    let cursor = Cursor::Text {
        node: numerator,
        byte: 1,
    };
    let moved = scholium_spike_core::cursor::move_cursor(
        editor.document(),
        cursor,
        Direction::Down,
    )
    .expect("导航")
    .expect("分母存在");
    assert_eq!(
        moved,
        Cursor::Text {
            node: denominator,
            byte: 0
        }
    );
}

#[test]
fn vertical_navigation_walks_script_slots_in_order() {
    let (mut editor, _) = editor_with_text();
    let math = insert_math(&mut editor);
    let script = insert_child(&mut editor, math, 0, NodeKind::Script);
    let doc = editor.document();
    let base = child_at(doc, script, 0, 0);
    let sub = child_at(doc, script, 1, 0);
    let sup = child_at(doc, script, 2, 0);

    let mut cursor = Cursor::Text { node: base, byte: 0 };
    for expected in [sub, sup] {
        cursor = scholium_spike_core::cursor::move_cursor(
            editor.document(),
            cursor,
            Direction::Down,
        )
        .expect("导航")
        .expect("下一个槽位");
        assert_eq!(cursor.focus(), expected);
    }
}

#[test]
fn vertical_navigation_moves_between_matrix_cells() {
    let (mut editor, _) = editor_with_text();
    let math = insert_math(&mut editor);
    let matrix = insert_child(&mut editor, math, 0, NodeKind::Matrix);
    // Matrix 创建时已在 cells 槽放入一个空占位单元格，这里追加第二个。
    let first_cell = child_at(editor.document(), matrix, 0, 0);
    editor
        .apply(
            A,
            Intent::Structural,
            SemanticEdit::InsertNode {
                parent: matrix,
                slot: 0,
                index: 1,
                kind: NodeKind::Text,
            },
        )
        .expect("追加单元格")
        .expect("结构编辑应产生动作");
    let second_cell = child_at(editor.document(), matrix, 0, 1);
    assert_ne!(first_cell, second_cell);

    let moved = scholium_spike_core::cursor::move_cursor(
        editor.document(),
        Cursor::Text {
            node: first_cell,
            byte: 0,
        },
        Direction::Down,
    )
    .expect("导航")
    .expect("第二个单元格");
    assert_eq!(moved.focus(), second_cell);
}

#[test]
fn wrap_and_unwrap_sqrt_preserve_content() {
    let (mut editor, text) = editor_with_text();
    type_text(&mut editor, text, 0, "x");
    let sqrt = editor
        .apply(
            A,
            Intent::Structural,
            SemanticEdit::Wrap {
                node: text,
                kind: NodeKind::Sqrt,
            },
        )
        .expect("包裹")
        .expect("结构编辑应产生动作");
    let _ = sqrt;
    assert_eq!(editor.document().to_plain_text().trim(), "sqrt(x)");

    let wrapper = {
        let doc = editor.document();
        let paragraph = child_at(doc, doc.root(), 0, 0);
        child_at(doc, paragraph, 0, 0)
    };
    editor
        .apply(A, Intent::Structural, SemanticEdit::Unwrap { node: wrapper })
        .expect("解除包裹");
    assert_eq!(editor.document().to_plain_text().trim(), "x");
}

#[test]
fn unwrap_refuses_structure_without_content_policy() {
    let (mut editor, _) = editor_with_text();
    let math = insert_math(&mut editor);
    let fraction = insert_child(&mut editor, math, 0, NodeKind::Fraction);
    let err = editor
        .apply(
            A,
            Intent::Structural,
            SemanticEdit::Unwrap { node: fraction },
        )
        .expect_err("多槽结构没有安全保留策略，必须拒绝");
    assert!(matches!(err, EditError::Unsupported { .. }));
}

#[test]
fn cycle_variant_wraps_around() {
    let (mut editor, _) = editor_with_text();
    let math = insert_math(&mut editor);
    let fraction = insert_child(&mut editor, math, 0, NodeKind::Fraction);
    let variants = NodeKind::Fraction.variant_count();
    for expected in (1..variants).chain(std::iter::once(0)) {
        editor
            .apply(
                A,
                Intent::Structural,
                SemanticEdit::CycleVariant { node: fraction },
            )
            .expect("循环变体");
        assert_eq!(editor.document().node(fraction).expect("节点").variant, expected);
    }
}

// ---------------------------------------------------------------- 核心集成

#[test]
fn every_committed_edit_appends_an_action() {
    let (mut editor, text) = editor_with_text();
    let before = editor.revision();
    type_text(&mut editor, text, 0, "abc");
    assert!(editor.revision() > before, "编辑必须推进 revision");
    assert_eq!(editor.history().len(), 1);
    assert_eq!(editor.history().actions()[0].intent, Intent::Typing);
}

#[test]
fn stale_base_revision_is_rejected() {
    let (mut editor, text) = editor_with_text();
    type_text(&mut editor, text, 0, "a");
    let stale = editor.revision() - 1;
    let err = editor
        .apply_at(
            A,
            Intent::Typing,
            stale,
            SemanticEdit::InsertText {
                node: text,
                at: 1,
                text: "b".to_string(),
            },
        )
        .expect_err("过期 revision 必须被拒绝");
    assert!(matches!(err, EditError::StaleRevision { .. }));
    assert_eq!(text_at(&editor, text), "a");
}

// ---------------------------------------------------------------- 团队规则（模型层）

#[test]
fn local_undo_preserves_remote_insert() {
    let (mut editor, text) = editor_with_text();
    type_text(&mut editor, text, 0, "abc");
    editor.commit_action();

    editor
        .apply_remote(RemoteEdit {
            actor: B,
            edit: SemanticEdit::InsertText {
                node: text,
                at: 1,
                text: "X".to_string(),
            },
        })
        .expect("远端插入");
    assert_eq!(text_at(&editor, text), "aXbc");

    let outcome = editor
        .undo(A)
        .expect("撤销")
        .expect("存在可撤销动作");
    assert_eq!(outcome.removed_chars, 3, "只删除自己插入的 3 个字符");
    assert_eq!(text_at(&editor, text), "X", "远端插入必须保留");
}

#[test]
fn remote_action_is_never_in_local_undo_scope() {
    let (mut editor, text) = editor_with_text();
    editor
        .apply_remote(RemoteEdit {
            actor: B,
            edit: SemanticEdit::InsertText {
                node: text,
                at: 0,
                text: "remote".to_string(),
            },
        })
        .expect("远端插入");
    assert_eq!(editor.history().len(), 1);
    assert_eq!(editor.history().actions()[0].intent, Intent::External);
    assert_eq!(editor.history().last_undoable(A), None);
    assert_eq!(editor.undo(A).expect("撤销查询"), None);
    assert_eq!(text_at(&editor, text), "remote");
}

#[test]
fn undo_of_deletion_restores_original_position() {
    let (mut editor, text) = editor_with_text();
    type_text(&mut editor, text, 0, "abcd");
    editor
        .apply(
            A,
            Intent::Typing,
            SemanticEdit::DeleteRange {
                node: text,
                start: 1,
                end: 3,
            },
        )
        .expect("删除 bc");
    assert_eq!(text_at(&editor, text), "ad");
    editor.undo(A).expect("撤销").expect("可撤销");
    assert_eq!(text_at(&editor, text), "abcd");
}

#[test]
fn source_pane_is_read_only_until_language_is_granted() {
    let mut pane = SourcePane::new(Dialect::Typst, "= title\n");
    assert!(!pane.is_writable(), "默认只读，不依赖客户端自称");
    let err = pane.replace(0, 0, "x").expect_err("只读面板拒绝写入");
    assert!(matches!(err, EditError::SourceReadOnly { .. }));
    assert_eq!(pane.text(), "= title\n", "拒绝后缓冲不变");

    pane.set_writable(true);
    pane.replace(0, 0, "% ").expect("授权后可写");
    assert_eq!(pane.text(), "% = title\n");
}

// ---------------------------------------------------------------- 大源码

#[test]
fn large_source_opens_and_edits_locally() {
    const LINES: usize = 100_000;
    let open_start = std::time::Instant::now();
    let mut pane = SourcePane::with_lines(Dialect::Latex, LINES);
    let open_ms = open_start.elapsed().as_millis();
    assert_eq!(pane.line_count(), LINES);

    let offset = pane.line_offset(LINES / 2).expect("中间行存在");
    pane.set_writable(true);
    let edit_start = std::time::Instant::now();
    pane.replace(offset, offset, "% spike\n").expect("局部插入");
    let edit_us = edit_start.elapsed().as_micros();

    assert!(pane.text().contains("% spike"));
    println!("large_source: open={open_ms}ms bytes={} edit={edit_us}us", pane.len_bytes());
    assert!(edit_us < 50_000, "单次局部编辑不应达到秒级：{edit_us}us");
}

// ---------------------------------------------------------------- 文档不变量

#[test]
fn empty_document_always_has_reachable_text_leaf() {
    let doc = Document::new();
    let text = doc.first_text_descendant(doc.root());
    assert!(text.is_some(), "空文档也必须保留可编辑文本槽位");
    assert_eq!(doc.to_plain_text(), "\n");
}

#[test]
fn selection_orders_endpoints_by_document_order() {
    use scholium_spike_core::Selection;

    let (mut editor, text) = editor_with_text();
    type_text(&mut editor, text, 0, "abcdef");

    let left = Cursor::Text { node: text, byte: 1 };
    let right = Cursor::Text { node: text, byte: 4 };

    // 无论拖拽方向如何，ordered 都返回文档顺序。
    let forward = Selection {
        anchor: left,
        focus: right,
    };
    let backward = Selection {
        anchor: right,
        focus: left,
    };
    assert_eq!(
        forward.ordered(editor.document()),
        backward.ordered(editor.document()),
        "拖拽方向不应影响语义"
    );
    assert_eq!(
        forward.ordered(editor.document()),
        Some((left, right)),
        "ordered 必须按文档顺序"
    );
    assert!(!forward.is_collapsed());
    assert!(Selection::collapsed(left).is_collapsed());
}

#[test]
fn selection_text_range_only_within_one_leaf() {
    use scholium_spike_core::Selection;

    let (mut editor, text) = editor_with_text();
    type_text(&mut editor, text, 0, "abcdef");

    let selection = Selection {
        anchor: Cursor::Text { node: text, byte: 1 },
        focus: Cursor::Text { node: text, byte: 4 },
    };
    assert_eq!(
        selection.text_range(editor.document()),
        Some((text, 1, 4)),
        "同一叶子内应给出字节区间"
    );

    // 跨节点的选区（槽位端点）没有可删除的字节区间。
    let across = Selection {
        anchor: Cursor::Text { node: text, byte: 1 },
        focus: Cursor::Slot {
            node: editor.document().root(),
            slot: 0,
            index: 0,
        },
    };
    assert_eq!(
        across.text_range(editor.document()),
        None,
        "跨节点选区没有可删除的字节区间"
    );
}
