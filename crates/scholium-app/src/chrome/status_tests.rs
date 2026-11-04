//! Status bar behaviour: user-facing position, context and backend state.
use super::tests::{EN, Window, ZH, overflow};
use crate::state::ViewMode;

#[test]
fn developer_revisions_are_not_in_the_status_bar_text() {
    let mut w = Window::new(1280.0, ZH);
    w.editor().revision = 7;
    w.settle();
    assert!(!w.texts().iter().any(|t| t.contains("r7")));
}

#[test]
fn the_backend_label_is_always_shown_and_reads_updating_while_stale() {
    for (locale, steady, updating) in [
        (ZH, "Typst · 已同步", "Typst · 更新中"),
        (EN, "Typst · Synced", "Typst · Updating"),
    ] {
        for mode in [ViewMode::Page, ViewMode::Source] {
            let mut w = Window::new(1280.0, locale);
            w.state.mode = mode;
            w.editor().revision = 7;
            w.editor().shown_revision = Some(6);
            w.settle();
            let texts = w.texts();
            assert!(
                texts.contains(&steady.to_owned()),
                "{locale:?} {mode:?} {texts:?}"
            );
            assert!(!texts.contains(&updating.to_owned()));
            let steady_at = w
                .painted()
                .iter()
                .find(|(_, g)| g.text() == steady)
                .map(|(p, _)| *p);
            let next_at = |w: &Window| {
                w.painted()
                    .iter()
                    .filter(|(p, _)| Some(p.y) == steady_at.map(|s| s.y))
                    .map(|(p, _)| p.x)
                    .fold(f32::MAX, f32::min)
            };
            let before = next_at(&w);
            w.editor().pending = true;
            w.settle();
            let texts = w.texts();
            assert!(
                texts.contains(&updating.to_owned()),
                "{locale:?} {mode:?} {texts:?}"
            );
            assert!(!texts.contains(&steady.to_owned()));
            assert_eq!(next_at(&w), before, "no layout jump");
        }
    }
}

#[test]
fn the_native_status_bar_shows_page_words_and_context() {
    let mut w = Window::new(1280.0, ZH);
    w.editor().words = 1284;
    w.editor().preview_page = Some((1, 2));
    w.settle();
    let texts = w.texts();
    assert!(texts.contains(&"1,284 字".to_owned()), "{texts:?}");
    assert!(texts.contains(&"正文".to_owned()));
    assert!(
        texts.contains(&"第 1/2 页".to_owned()),
        "native page position"
    );
    w.editor().in_math = true;
    w.settle();
    assert!(w.texts().contains(&"公式".to_owned()));
    w.state.mode = ViewMode::Source;
    w.settle();
    let texts = w.texts();
    assert!(texts.contains(&"源码 · 只读".to_owned()));
    assert!(texts.contains(&"第 1/2 页".to_owned()), "{texts:?}");
}

#[test]
fn the_warning_segment_is_the_only_message_and_zoom_runs_commands() {
    let mut w = Window::new(1280.0, ZH);
    w.editor().problems = vec![overflow()];
    w.settle();
    assert!(w.texts().contains(&"公式超出行宽".to_owned()));
    w.click("view.zoom.in", "status");
    assert!(!w.state.fit_width);
    w.click("view.zoom.fit_width", "status");
    assert!(w.state.fit_width);
}
