use super::*;
use crate::typst_editor::tests::{Fixture, key, text};

const SIZE: egui::Vec2 = egui::vec2(900.0, 500.0);

fn frame(f: &mut Fixture, events: Vec<egui::Event>) {
    let mut output = f.ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, SIZE)),
            events,
            ..Default::default()
        },
        |ui| f.editor.show(ui, &mut f.state),
    );
    output.textures_delta.clear();
}

fn frames(f: &mut Fixture, count: usize) {
    for _ in 0..count {
        frame(f, vec![]);
    }
}

/// Frames until the visible range stops changing (wheel scrolling is smoothed over frames).
fn quiesce(f: &mut Fixture) {
    let mut last = f.editor.view;
    let mut steady = 0;
    for _ in 0..300 {
        frame(f, vec![]);
        let view = f.editor.view;
        steady = if view == last { steady + 1 } else { 0 };
        last = view;
        if steady >= 5 {
            return;
        }
    }
    panic!("view never settled: {last:?}");
}

fn window() -> Fixture {
    let f = Fixture::new();
    crate::theme::install(&f.ctx);
    f
}

/// "Top paragraph." then `lines` more paragraphs, typed through the event path.
fn long_document(f: &mut Fixture, lines: usize) {
    f.events(vec![text("Top paragraph.")]);
    for n in 1..=lines {
        f.events(vec![
            key(egui::Key::Enter, false),
            text(&format!("Body line number {n} with some words.")),
        ]);
    }
    f.settle();
}

/// Caret rectangle in page pt, plus the visible page range.
fn caret_and_view(f: &Fixture) -> ([f64; 2], [f64; 2], ViewRange) {
    let caret = f
        .editor
        .scene
        .as_ref()
        .and_then(|s| s.geometry.caret(f.editor.cursor))
        .expect("current caret");
    (caret.top, caret.bottom, f.editor.view)
}

fn assert_caret_visible_with_margin(f: &Fixture) {
    let (top, bottom, view) = caret_and_view(f);
    let margin = f64::from(FOLLOW_MARGIN_LINES) * (bottom[1] - top[1]);
    assert!(
        top[1] - margin >= view.top_pt - 0.5 && bottom[1] + margin <= view.bottom_pt + 0.5,
        "caret {top:?}..{bottom:?} margin {margin} view {view:?}"
    );
}

fn wheel(f: &mut Fixture, dy: f32) {
    frame(
        f,
        vec![
            egui::Event::PointerMoved(egui::pos2(450.0, 250.0)),
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(0.0, dy),
                phase: egui::TouchPhase::Move,
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );
    quiesce(f);
}

/// Wheel the page up in modest steps so the caret ends well below the viewport.
fn scroll_away(f: &mut Fixture) {
    for _ in 0..3 {
        wheel(f, 300.0);
    }
    let (_, bottom, view) = caret_and_view(f);
    assert!(bottom[1] > view.bottom_pt, "{view:?}");
}

fn started() -> Fixture {
    let mut f = window();
    f.settle();
    frames(&mut f, 2);
    long_document(&mut f, 45);
    frames(&mut f, 4);
    f
}

#[test]
fn typing_past_the_viewport_bottom_scrolls_the_caret_into_view_with_margin() {
    let f = started();
    assert_caret_visible_with_margin(&f);
}

#[test]
fn wheel_scrolling_away_is_not_undone_until_the_next_keystroke() {
    let mut f = started();
    scroll_away(&mut f);
    let away = f.editor.view;
    frames(&mut f, 5);
    assert_eq!(f.editor.view, away);
    frame(&mut f, vec![text("!")]);
    f.settle();
    quiesce(&mut f);
    assert_caret_visible_with_margin(&f);
}

#[test]
fn ctrl_end_and_ctrl_home_reach_the_page_ends() {
    let mut f = started();
    frame(&mut f, vec![key(egui::Key::Home, true)]);
    frames(&mut f, 4);
    assert!(f.editor.view.top_pt <= 0.0, "{:?}", f.editor.view);
    frame(&mut f, vec![key(egui::Key::End, true)]);
    frames(&mut f, 4);
    let size = f.editor.scene.as_ref().expect("scene").size_pt;
    assert!(
        f.editor.view.bottom_pt >= size[1] - 0.5,
        "{:?} of {size:?}",
        f.editor.view
    );
    assert_caret_visible_with_margin(&f);
}

#[test]
fn arrow_up_near_the_top_edge_scrolls_up_to_keep_the_margin() {
    let mut f = started();
    for _ in 0..6 {
        frame(&mut f, vec![key(egui::Key::ArrowUp, false)]);
        f.settle();
        frames(&mut f, 3);
        assert_caret_visible_with_margin(&f);
    }
}

#[test]
fn follow_waits_for_the_new_scene_instead_of_aiming_at_a_stale_caret() {
    let mut f = started();
    scroll_away(&mut f);
    let before = f.editor.view;
    frame(
        &mut f,
        vec![text("x"), key(egui::Key::Enter, false), text("y")],
    );
    // The Scene for the edit has not been adopted yet, so the pending follow must not fire.
    assert!(!f.editor.current() && f.editor.follow.pending);
    assert_eq!(f.editor.view.top_pt, before.top_pt);
    f.settle();
    quiesce(&mut f);
    assert!(!f.editor.follow.pending);
    assert_caret_visible_with_margin(&f);
}

#[test]
fn a_click_never_scrolls() {
    let mut f = started();
    scroll_away(&mut f);
    let top = f.editor.view.top_pt;
    let click = |pressed| egui::Event::PointerButton {
        pos: egui::pos2(450.0, 120.0),
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    frame(&mut f, vec![click(true)]);
    frame(&mut f, vec![click(false)]);
    frames(&mut f, 4);
    assert_eq!(f.editor.view.top_pt, top);
}

#[test]
fn reveal_widens_to_the_page_edge_only_for_document_edge_keys() {
    let page = egui::Rect::from_min_size(egui::pos2(100.0, 50.0), egui::vec2(400.0, 1000.0));
    let caret = egui::Rect::from_min_size(egui::pos2(150.0, 200.0), egui::vec2(1.5, 10.0));
    let plain = reveal(caret, page, 24.0, None);
    assert_eq!(plain.top(), 200.0 - 20.0);
    assert_eq!(plain.bottom(), 210.0 + 20.0);
    assert_eq!(reveal(caret, page, 24.0, Some(Edge::Top)).top(), 26.0);
    assert_eq!(
        reveal(caret, page, 24.0, Some(Edge::Bottom)).bottom(),
        1074.0
    );
}

#[test]
fn undo_after_typing_past_the_viewport_removes_the_whole_word_and_follows_the_caret() {
    let mut f = started();
    frame(&mut f, vec![text(" one ")]);
    frame(&mut f, vec![text("t"), text("w"), text("o")]);
    f.settle();
    quiesce(&mut f);
    scroll_away(&mut f);
    f.state.undo_requested = true;
    frame(&mut f, vec![]);
    f.settle();
    quiesce(&mut f);
    let cursor = f.editor.cursor.leaf;
    let last = input::leaves(&f.editor.snapshot)
        .into_iter()
        .find(|l| l.id == cursor)
        .map(|l| l.text.to_owned())
        .expect("leaf");
    assert_eq!(last, "Body line number 45 with some words. one ");
    assert_caret_visible_with_margin(&f);
}
