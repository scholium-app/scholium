use super::*;
use crate::typst_editor::tests::{Fixture, key, text};
use egui::accesskit::{self, Role};
use std::collections::HashMap;

const LONG: &str = "long paragraph long paragraph long paragraph long paragraph long paragraph \
                    long paragraph long paragraph long paragraph long paragraph long paragraph end";

struct Tree(HashMap<accesskit::NodeId, accesskit::Node>);

impl Tree {
    fn node(&self, id: egui::Id) -> &accesskit::Node {
        self.0.get(&id.accesskit_id()).expect("published node")
    }

    fn body(&self) -> &accesskit::Node {
        self.node(super::super::paint::id())
    }

    fn children(&self, role: Role) -> Vec<(accesskit::NodeId, &accesskit::Node)> {
        self.body()
            .children()
            .iter()
            .filter_map(|id| self.0.get(id).map(|n| (*id, n)))
            .filter(|(_, n)| n.role() == role)
            .collect()
    }

    fn text(&self) -> String {
        self.children(Role::TextRun)
            .iter()
            .filter_map(|(_, n)| n.value())
            .collect()
    }
}

/// One whole frame with AccessKit on; returns the published accessibility tree.
fn frame(f: &mut Fixture, events: Vec<egui::Event>) -> Tree {
    f.ctx.enable_accesskit();
    let mut output = f.ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1280.0, 900.0),
            )),
            events,
            ..Default::default()
        },
        |ui| f.editor.show(ui, &mut f.state),
    );
    output.textures_delta.clear();
    let update = output
        .platform_output
        .accesskit_update
        .expect("AccessKit enabled");
    Tree(update.nodes.into_iter().collect())
}

fn settled(f: &mut Fixture) -> Tree {
    f.settle();
    frame(f, vec![]);
    frame(f, vec![])
}

fn select(tree: &Tree, anchor: (usize, usize), focus: (usize, usize)) -> egui::Event {
    let runs = tree.children(Role::TextRun);
    let position = |(run, character_index): (usize, usize)| accesskit::TextPosition {
        node: runs[run].0,
        character_index,
    };
    egui::Event::AccessKitActionRequest(accesskit::ActionRequest {
        action: accesskit::Action::SetTextSelection,
        target_tree: accesskit::TreeId::ROOT,
        target_node: super::super::paint::id().accesskit_id(),
        data: Some(accesskit::ActionData::SetTextSelection(
            accesskit::TextSelection {
                anchor: position(anchor),
                focus: position(focus),
            },
        )),
    })
}

fn values(f: &Fixture) -> Vec<String> {
    input::leaves(&f.editor.snapshot)
        .iter()
        .map(|l| l.text.to_owned())
        .collect()
}

#[test]
fn body_is_a_named_text_input_whose_runs_read_the_document_in_order() {
    let mut f = Fixture::new();
    f.events(vec![egui::Event::Paste("中文 abc\n\nxyz 👩‍🔬".into())]);
    let tree = settled(&mut f);
    assert_eq!(tree.body().role(), Role::MultilineTextInput);
    assert_eq!(tree.body().label(), Some("正文"));
    assert!(
        tree.body()
            .supports_action(accesskit::Action::SetTextSelection)
    );
    assert_eq!(tree.text(), "中文 abc\n\nxyz 👩‍🔬");
    for (_, run) in tree.children(Role::TextRun) {
        let value = run.value().expect("run text");
        let lengths: Vec<u8> = value.chars().map(|c| c.len_utf8() as u8).collect();
        assert_eq!(run.character_lengths(), lengths.as_slice());
        assert!(run.bounds().is_some(), "current scene runs carry geometry");
    }
}

#[test]
fn wrapped_paragraph_is_split_into_visual_line_runs() {
    let mut f = Fixture::new();
    f.events(vec![egui::Event::Paste(LONG.into())]);
    let tree = settled(&mut f);
    let runs = tree.children(Role::TextRun);
    assert!(runs.len() >= 2, "one run per visual line");
    assert_eq!(tree.text(), LONG);
    let tops: Vec<f64> = runs
        .iter()
        .map(|(_, r)| r.bounds().expect("bounds").y0)
        .collect();
    assert!(tops.windows(2).all(|w| w[1] > w[0]), "{tops:?}");
    for (_, run) in runs {
        let chars = run.value().expect("text").chars().count();
        assert_eq!(run.character_positions().map(<[f32]>::len), Some(chars));
        assert_eq!(run.character_widths().map(<[f32]>::len), Some(chars));
    }
}

#[test]
fn pending_scene_keeps_the_current_text_without_stale_geometry() {
    let mut f = Fixture::new();
    settled(&mut f);
    f.events(vec![text("pending😀")]);
    let tree = frame(&mut f, vec![]);
    assert!(!f.editor.current(), "layout has not caught up yet");
    assert_eq!(tree.text(), "pending😀");
    assert!(
        tree.children(Role::TextRun)
            .iter()
            .all(|(_, r)| r.bounds().is_none())
    );
}

#[test]
fn caret_and_selection_are_published_as_text_positions() {
    let mut f = Fixture::new();
    f.events(vec![egui::Event::Paste("abc\ndef".into())]);
    let tree = settled(&mut f);
    let selection = tree.body().text_selection().expect("caret");
    assert_eq!(selection.anchor, selection.focus, "collapsed caret");
    let runs = tree.children(Role::TextRun);
    assert_eq!(selection.focus.node, runs[1].0);
    assert_eq!(selection.focus.character_index, 3, "end of def");
    f.events(vec![egui::Event::Key {
        key: egui::Key::Home,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::SHIFT,
    }]);
    let tree = settled(&mut f);
    let selection = tree.body().text_selection().expect("range");
    assert_eq!(selection.anchor.character_index, 3);
    assert_eq!(selection.focus.character_index, 0);
}

#[test]
fn assistive_selection_maps_to_semantic_endpoints_and_typing_replaces_it() {
    let mut f = Fixture::new();
    f.events(vec![egui::Event::Paste("中文abc\ndef".into())]);
    let tree = settled(&mut f);
    let revision = f.editor.snapshot.revision;
    frame(&mut f, vec![select(&tree, (0, 1), (1, 1))]);
    assert!(f.editor.selected());
    f.events(vec![text("X")]);
    assert_eq!(values(&f).concat(), "中Xef");
    assert_eq!(f.editor.snapshot.revision.0, revision.0 + 1);
}

#[test]
fn assistive_selection_against_an_older_publication_is_rejected() {
    let mut f = Fixture::new();
    f.events(vec![egui::Event::Paste("abc".into())]);
    let tree = settled(&mut f);
    f.events(vec![text("Z")]);
    let before = f.editor.cursor;
    frame(&mut f, vec![select(&tree, (0, 0), (0, 1))]);
    assert_eq!(f.editor.cursor, before);
    assert!(!f.editor.selected());
    assert!(f.state.edit_error.is_some());
}

#[test]
fn assistive_selection_inside_a_grapheme_is_rejected() {
    let mut f = Fixture::new();
    f.events(vec![text("a👩‍🔬b")]);
    let tree = settled(&mut f);
    let before = f.editor.cursor;
    frame(&mut f, vec![select(&tree, (0, 2), (0, 2))]);
    assert_eq!(f.editor.cursor, before);
    assert!(f.state.edit_error.is_some());
}

#[test]
fn formula_is_a_math_object_and_slot_context_is_a_live_status() {
    let mut f = Fixture::new();
    f.events(vec![
        text("A "),
        text("$"),
        text("12"),
        key(egui::Key::Slash, true),
    ]);
    let tree = settled(&mut f);
    let math = tree.children(Role::Math);
    assert_eq!(math.len(), 1);
    assert_eq!(math[0].1.label(), Some("公式：分数（分子：12；分母：空）"));
    let status = tree.children(Role::Status);
    assert_eq!(status.len(), 1);
    assert_eq!(status[0].1.live(), Some(accesskit::Live::Polite));
    assert_eq!(status[0].1.label(), Some("分数分母：空"));
    assert_eq!(
        tree.text(),
        "A ",
        "formulas are objects, not body characters"
    );
}

#[test]
fn speech_reads_nested_fractions_outside_in_and_empty_slots_as_empty() {
    let inner = MathNode {
        node: NodeId::fresh(),
        body: MathBody::Fraction {
            numerator: Box::new(MathNode {
                node: NodeId::fresh(),
                body: MathBody::Text { text: "a".into() },
            }),
            denominator: Box::new(MathNode::hole()),
        },
    };
    let leaf = match &inner.body {
        MathBody::Fraction { denominator, .. } => denominator.node,
        _ => unreachable!("built above"),
    };
    let outer = MathNode {
        node: NodeId::fresh(),
        body: MathBody::Fraction {
            numerator: Box::new(MathNode {
                node: NodeId::fresh(),
                body: MathBody::Symbol { symbol: 'π' },
            }),
            denominator: Box::new(inner),
        },
    };
    assert_eq!(
        speech::describe(&outer),
        "分数（分子：π；分母：分数（分子：a；分母：空））"
    );
    assert_eq!(
        speech::slot_context(&outer, leaf).as_deref(),
        Some("分数分母，分数分母：空")
    );
}

#[test]
fn mixed_cjk_and_latin_on_one_line_stay_in_one_run() {
    let mut f = Fixture::new();
    f.events(vec![text("中文abc")]);
    let tree = settled(&mut f);
    let runs = tree.children(Role::TextRun);
    assert_eq!(runs.len(), 1, "fallback-font glyphs share the visual line");
    assert_eq!(runs[0].1.value(), Some("中文abc"));
}

#[test]
fn caret_in_the_empty_leaf_after_a_formula_is_still_published() {
    let mut f = Fixture::new();
    f.events(vec![text("A"), text("$"), text("1"), text("$")]);
    assert!(f.editor.body_leaf(f.editor.cursor.leaf), "left the formula");
    let tree = settled(&mut f);
    let caret = tree
        .body()
        .text_selection()
        .expect("caret after the formula");
    let runs = tree.children(Role::TextRun);
    assert_eq!(
        (caret.focus.node, caret.focus.character_index),
        (runs[0].0, 1)
    );
}

#[test]
fn formula_only_paragraph_keeps_a_run_and_a_caret() {
    let mut f = Fixture::new();
    f.events(vec![egui::Event::Paste("abc\n".into())]);
    f.events(vec![text("$"), text("1")]);
    let tree = settled(&mut f);
    let runs = tree.children(Role::TextRun);
    assert_eq!(runs.len(), 2, "the formula-only paragraph still has a run");
    assert_eq!(tree.text(), "abc\n");
    let caret = tree
        .body()
        .text_selection()
        .expect("math caret reads at its paragraph");
    assert_eq!(caret.focus.node, runs[1].0);
    frame(&mut f, vec![select(&tree, (1, 0), (1, 0))]);
    assert!(f.state.edit_error.is_none(), "{:?}", f.state.edit_error);
    assert!(!f.editor.selected());
}

#[test]
fn assistive_caret_at_a_soft_wrap_stays_on_the_earlier_line() {
    let mut f = Fixture::new();
    f.events(vec![egui::Event::Paste(LONG.into())]);
    let tree = settled(&mut f);
    let first = tree.children(Role::TextRun)[0]
        .1
        .value()
        .expect("line")
        .chars()
        .count();
    frame(&mut f, vec![select(&tree, (0, first), (0, first))]);
    assert_eq!(f.editor.cursor.affinity, Affinity::Upstream);
    let tree = frame(&mut f, vec![]);
    let caret = tree.body().text_selection().expect("caret");
    assert_eq!(caret.focus.node, tree.children(Role::TextRun)[0].0);
}

fn text_node(value: &str) -> MathNode {
    MathNode {
        node: NodeId::fresh(),
        body: MathBody::Text { text: value.into() },
    }
}

#[test]
fn speech_names_script_slots_like_fraction_slots() {
    let sup = MathNode::hole();
    let sup_id = sup.node;
    let sub = text_node("i");
    let sub_id = sub.node;
    let base = text_node("x");
    let base_id = base.node;
    let attach = MathNode {
        node: NodeId::fresh(),
        body: MathBody::Attach {
            base: Box::new(base),
            sub: Some(Box::new(sub)),
            sup: Some(Box::new(sup)),
        },
    };
    assert_eq!(
        speech::describe(&attach),
        "上下标（底数：x；下标：i；上标：空）"
    );
    for (leaf, expected) in [
        (base_id, "底数：x"),
        (sub_id, "下标：i"),
        (sup_id, "上标：空"),
    ] {
        assert_eq!(
            speech::slot_context(&attach, leaf).as_deref(),
            Some(expected)
        );
    }
    let only_sup = MathNode {
        node: NodeId::fresh(),
        body: MathBody::Attach {
            base: Box::new(text_node("y")),
            sub: None,
            sup: Some(Box::new(text_node("2"))),
        },
    };
    assert_eq!(speech::describe(&only_sup), "上下标（底数：y；上标：2）");
    let nested = MathNode {
        node: NodeId::fresh(),
        body: MathBody::Fraction {
            numerator: Box::new(only_sup),
            denominator: Box::new(text_node("z")),
        },
    };
    assert!(speech::describe(&nested).starts_with("分数（分子：上下标（"));
}

#[test]
fn scripts_in_a_live_formula_are_spoken_in_the_math_object_and_status() {
    let mut f = Fixture::new();
    f.events(vec![text("A"), text("$"), text("x"), text("^"), text("2")]);
    let tree = settled(&mut f);
    let math = tree.children(Role::Math);
    assert_eq!(math[0].1.label(), Some("公式：上下标（底数：x；上标：2）"));
    let status = tree.children(Role::Status);
    assert_eq!(status[0].1.label(), Some("上标：2"));
}
