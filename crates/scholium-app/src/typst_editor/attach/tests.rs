use super::*;
use crate::typst_editor::tests::{Fixture, key, text};
use scholium_model::NodeId;

fn press(key: egui::Key, shift: bool) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: if shift {
            egui::Modifiers::SHIFT
        } else {
            egui::Modifiers::NONE
        },
    }
}

fn root(f: &Fixture) -> MathNode {
    f.editor
        .snapshot
        .blocks
        .iter()
        .flat_map(|b| &b.content)
        .find_map(|i| match &i.body {
            InlineBody::Math { root } => Some(root.clone()),
            _ => None,
        })
        .expect("a formula")
}

fn literal(node: &MathNode) -> &str {
    match &node.body {
        MathBody::Text { text } => text,
        other => panic!("expected text, found {other:?}"),
    }
}

/// (base, sub, sup) of an Attach node.
fn scripts(node: &MathNode) -> (&MathNode, Option<&MathNode>, Option<&MathNode>) {
    match &node.body {
        MathBody::Attach { base, sub, sup } => (base, sub.as_deref(), sup.as_deref()),
        other => panic!("expected attach, found {other:?}"),
    }
}

fn caret(f: &Fixture) -> (NodeId, usize) {
    (f.editor.cursor.leaf, f.editor.cursor.byte)
}

#[test]
fn typing_caret_two_right_plus_y_builds_an_attach_followed_by_text() {
    let mut f = Fixture::new();
    f.events(vec![
        text("$"),
        text("x"),
        text("^"),
        text("2"),
        press(egui::Key::ArrowRight, false),
        text("+y"),
    ]);
    let formula = root(&f);
    let MathBody::Row { children } = &formula.body else {
        panic!("attach and its follower share one row: {formula:?}");
    };
    assert_eq!(children.len(), 2);
    let (base, sub, sup) = scripts(&children[0]);
    assert_eq!(literal(base), "x");
    assert!(sub.is_none());
    assert_eq!(literal(sup.expect("sup")), "2");
    assert_eq!(literal(&children[1]), "+y");
    assert_eq!(caret(&f), (children[1].node, 2));
    f.settle();
}

#[test]
fn subscript_then_superscript_is_one_attach_with_both_slots() {
    let mut f = Fixture::new();
    f.events(vec![
        text("$"),
        text("x"),
        text("_"),
        text("i"),
        text("^"),
        text("2"),
    ]);
    let formula = root(&f);
    let (base, sub, sup) = scripts(&formula);
    assert_eq!(literal(base), "x");
    assert_eq!(literal(sub.expect("sub")), "i");
    assert_eq!(literal(sup.expect("sup")), "2");
    assert!(
        !input::leaves(&f.editor.snapshot)
            .iter()
            .any(|l| l.text.contains(['^', '_'])),
        "no literal script markers"
    );
}

#[test]
fn script_markers_are_literal_in_body_text_and_paste() {
    let mut f = Fixture::new();
    f.events(vec![text("a^b_c")]);
    assert_eq!(input::leaves(&f.editor.snapshot)[0].text, "a^b_c");
    f.events(vec![text("$"), egui::Event::Paste("x^2".into())]);
    assert_eq!(literal(&root(&f)), "x^2");
}

#[test]
fn caret_at_text_start_attaches_to_an_empty_base() {
    let mut f = Fixture::new();
    f.events(vec![text("$"), text("ab")]);
    let id = root(&f).node;
    f.editor.cursor = Position {
        leaf: id,
        byte: 0,
        affinity: Affinity::Downstream,
    };
    f.events(vec![text("^")]);
    let formula = root(&f);
    let MathBody::Row { children } = &formula.body else {
        panic!("row expected: {formula:?}");
    };
    assert_eq!(scripts(&children[0]).0.body, MathBody::Hole);
    assert_eq!(literal(&children[1]), "ab");
}

#[test]
fn tab_walks_base_sub_sup_and_then_out_of_the_formula() {
    let mut f = Fixture::new();
    f.events(vec![
        text("$"),
        text("x"),
        text("_"),
        text("i"),
        text("^"),
        text("2"),
    ]);
    let formula = root(&f);
    let (base, sub, sup) = scripts(&formula);
    let ids = (base.node, sub.expect("sub").node, sup.expect("sup").node);
    f.events(vec![press(egui::Key::Tab, true)]);
    assert_eq!(caret(&f).0, ids.1);
    f.events(vec![press(egui::Key::Tab, true)]);
    assert_eq!(caret(&f).0, ids.0);
    f.events(vec![
        press(egui::Key::Tab, false),
        press(egui::Key::Tab, false),
    ]);
    assert_eq!(caret(&f).0, ids.2);
    f.events(vec![press(egui::Key::Tab, false)]);
    assert!(
        f.editor.body_leaf(f.editor.cursor.leaf),
        "out of the formula"
    );
}

#[test]
fn up_and_down_move_between_base_subscript_and_superscript() {
    let mut f = Fixture::new();
    f.events(vec![
        text("$"),
        text("x"),
        text("_"),
        text("i"),
        text("^"),
        text("2"),
    ]);
    f.settle();
    let formula = root(&f);
    let (base, sub, sup) = scripts(&formula);
    let ids = (base.node, sub.expect("sub").node, sup.expect("sup").node);
    f.events(vec![press(egui::Key::ArrowDown, false)]);
    assert_eq!(caret(&f).0, ids.1, "down from the superscript");
    f.events(vec![press(egui::Key::ArrowUp, false)]);
    assert_eq!(caret(&f).0, ids.2, "up from the subscript");
    f.editor.cursor.leaf = ids.0;
    f.editor.cursor.byte = 1;
    f.events(vec![press(egui::Key::ArrowUp, false)]);
    assert_eq!(caret(&f).0, ids.2, "up from the base");
    f.editor.cursor.leaf = ids.0;
    f.events(vec![press(egui::Key::ArrowDown, false)]);
    assert_eq!(caret(&f).0, ids.1, "down from the base");
}

#[test]
fn left_enters_the_base_end_and_right_leaves_to_just_after_the_attach() {
    let mut f = Fixture::new();
    f.events(vec![
        text("$"),
        text("x"),
        text("_"),
        text("i"),
        text("^"),
        text("2"),
    ]);
    let formula = root(&f);
    let (base, sub, sup) = scripts(&formula);
    let ids = (base.node, sub.expect("sub").node, sup.expect("sup").node);
    f.editor.cursor.leaf = ids.2;
    f.editor.cursor.byte = 0;
    f.events(vec![press(egui::Key::ArrowLeft, false)]);
    assert_eq!(caret(&f), (ids.0, 1), "left of the superscript start");
    f.editor.cursor.leaf = ids.1;
    f.editor.cursor.byte = 1;
    f.events(vec![press(egui::Key::ArrowRight, false)]);
    let after = root(&f);
    let MathBody::Row { children } = &after.body else {
        panic!("a placeholder follows the attach: {after:?}");
    };
    assert_eq!(caret(&f), (children[1].node, 0));
    assert_eq!(children[1].body, MathBody::Hole);
    f.events(vec![press(egui::Key::ArrowRight, false)]);
    assert!(
        matches!(root(&f).body, MathBody::Attach { .. }),
        "the placeholder is gone once the caret leaves it"
    );
}

#[test]
fn backspace_in_an_empty_slot_removes_it_and_undo_restores_it() {
    let mut f = Fixture::new();
    f.events(vec![text("$"), text("x"), text("^")]);
    let sup = scripts(&root(&f)).2.map(|s| s.node);
    assert_eq!(Some(caret(&f).0), sup);
    let attached = f.editor.snapshot.clone();
    f.events(vec![press(egui::Key::Backspace, false)]);
    let flat = root(&f);
    assert_eq!(literal(&flat), "x");
    assert_eq!(caret(&f), (flat.node, 1));
    f.editor.history(false, &mut f.state);
    assert_eq!(*f.editor.snapshot, *attached);
}

#[test]
fn removing_the_last_slot_of_mid_text_merges_the_text_back() {
    let mut f = Fixture::new();
    f.events(vec![text("$"), text("abc")]);
    let id = root(&f).node;
    f.editor.cursor = Position {
        leaf: id,
        byte: 2,
        affinity: Affinity::Downstream,
    };
    f.events(vec![text("^")]);
    f.events(vec![press(egui::Key::Backspace, false)]);
    let flat = root(&f);
    assert_eq!(literal(&flat), "abc");
    assert_eq!(caret(&f), (flat.node, 2));
}

#[test]
fn attach_commands_are_ignored_while_composing_or_selecting() {
    let mut f = Fixture::new();
    f.events(vec![text("$"), text("x")]);
    f.state.composition = Some("あ".into());
    f.events(vec![text("^")]);
    f.state.composition = None;
    assert!(matches!(root(&f).body, MathBody::Text { .. }));
    f.events(vec![key(egui::Key::Home, true)]);
    f.events(vec![text("^")]);
    assert!(
        !matches!(root(&f).body, MathBody::Attach { .. }),
        "a body caret keeps literal text"
    );
}

#[test]
fn attach_scene_has_current_geometry_for_every_leaf() {
    let mut f = Fixture::new();
    f.events(vec![
        text("$"),
        text("x"),
        text("^"),
        text("2"),
        press(egui::Key::ArrowRight, false),
        text("+y"),
    ]);
    f.settle();
    let scene = f.editor.scene.as_ref().expect("settled scene");
    for leaf in input::leaves(&f.editor.snapshot)
        .into_iter()
        .filter(|l| l.math)
    {
        let position = Position {
            leaf: leaf.id,
            byte: 0,
            affinity: Affinity::Downstream,
        };
        assert!(scene.geometry.caret(position).is_some(), "{}", leaf.text);
    }
}

fn holes(f: &Fixture) -> usize {
    input::leaves(&f.editor.snapshot)
        .iter()
        .filter(|l| l.math && l.text.is_empty())
        .count()
}

fn body_leaf(f: &Fixture) -> NodeId {
    input::leaves(&f.editor.snapshot)
        .iter()
        .find(|l| !l.math)
        .expect("body leaf")
        .id
}

/// `$x^2`, Right: the caret sits in a placeholder; returns the state and undo depth before it.
fn into_placeholder(f: &mut Fixture) -> (StructuredDocument, usize) {
    f.events(vec![text("$"), text("x"), text("^"), text("2")]);
    let typed = ((*f.editor.snapshot).clone(), f.editor.undo.len());
    f.events(vec![press(egui::Key::ArrowRight, false)]);
    assert!(matches!(root(f).body, MathBody::Row { .. }), "placeholder");
    typed
}

fn click_body(f: &mut Fixture) {
    f.editor.cursor.leaf = body_leaf(f);
    f.editor.cursor.byte = 0;
    f.editor.settle_placeholder(&mut f.state);
}

#[test]
fn clicking_away_from_an_empty_placeholder_restores_the_state_after_the_script() {
    let mut f = Fixture::new();
    let (typed, depth) = into_placeholder(&mut f);
    click_body(&mut f);
    assert_eq!(*f.editor.snapshot, typed);
    assert_eq!(holes(&f), 0);
    assert_eq!(f.editor.undo.len(), depth, "no extra undo step");
}

#[test]
fn leaving_a_placeholder_by_left_or_ctrl_home_end_removes_it() {
    for leave in [
        press(egui::Key::ArrowLeft, false),
        key(egui::Key::End, true),
        key(egui::Key::Home, true),
    ] {
        let mut f = Fixture::new();
        let (typed, _) = into_placeholder(&mut f);
        f.events(vec![leave]);
        assert_eq!(*f.editor.snapshot, typed);
    }
}

#[test]
fn one_undo_after_leaving_a_placeholder_undoes_the_users_last_action() {
    let mut f = Fixture::new();
    into_placeholder(&mut f);
    click_body(&mut f);
    f.editor.history(false, &mut f.state);
    let formula = root(&f);
    assert_eq!(scripts(&formula).2.expect("sup slot").body, MathBody::Hole);
}

#[test]
fn right_from_an_empty_placeholder_exits_the_formula() {
    let mut f = Fixture::new();
    let (typed, _) = into_placeholder(&mut f);
    f.events(vec![press(egui::Key::ArrowRight, false)]);
    assert!(f.editor.body_leaf(f.editor.cursor.leaf), "left the formula");
    assert_eq!(*f.editor.snapshot, typed);
}

#[test]
fn typing_into_a_placeholder_keeps_it_as_content() {
    let mut f = Fixture::new();
    into_placeholder(&mut f);
    f.events(vec![text("+y")]);
    let typed = (*f.editor.snapshot).clone();
    click_body(&mut f);
    assert_eq!(*f.editor.snapshot, typed);
    let formula = root(&f);
    let MathBody::Row { children } = &formula.body else {
        panic!("row kept: {formula:?}");
    };
    assert_eq!(literal(&children[1]), "+y");
}

#[test]
fn grouped_typing_into_a_placeholder_keeps_it_and_undoes_in_one_step() {
    let mut f = Fixture::new();
    let (_, depth) = into_placeholder(&mut f);
    let with_placeholder = (*f.editor.snapshot).clone();
    f.events(vec![text("+")]);
    f.events(vec![text("y")]);
    click_body(&mut f);
    assert_eq!(
        holes(&f),
        0,
        "the typed text keeps the placeholder as content"
    );
    assert_eq!(
        f.editor.undo.len(),
        depth + 2,
        "LeaveSlot step plus one typing group"
    );
    f.editor.history(false, &mut f.state);
    assert_eq!(*f.editor.snapshot, with_placeholder);
}
