//! Status bar behaviour: user-facing position, context and backend state.
use super::tests::{EN, Window, ZH, overflow};
use crate::state::ViewMode;

#[test]
fn narrow_status_hides_zero_problems_and_draws_only_whole_labels() {
    for locale in [ZH, EN] {
        let mut w = Window::new(480.0, locale);
        assert!(!w.ids("status").contains(&"ui.status.count".into()));
        w.editor().words = 123_456_789;
        w.editor().preview_page = Some((1234, 5678));
        w.settle();
        assert!(
            w.clipped_status_labels().is_empty(),
            "{:?}",
            w.clipped_status_labels()
        );
        w.editor().problems = vec![overflow()];
        w.settle();
        assert!(w.ids("status").contains(&"ui.status.count".into()));
    }
}

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
#[test]
fn source_status_has_kernel_specific_segments_and_engine_opens_settings() {
    use scholium_storage::project::ProjectLanguage;
    for language in [ProjectLanguage::Typst, ProjectLanguage::Latex] {
        let mut w = Window::new(1440.0, EN);
        w.state.source_language = Some(language);
        w.state.source_position = (12, 56);
        w.state.kernel_status = Some("Preview engine".into());
        w.editor().words = 1284;
        w.settle();
        let texts = w.texts();
        assert!(texts.iter().any(|t| t.contains("12") && t.contains("56")));
        assert_eq!(
            texts.contains(&"1,284 words".into()),
            language == ProjectLanguage::Latex
        );
        assert!(!texts.contains(&"Body".into()));
        w.click("document.project_settings", "status");
        assert!(w.state.file_dialog.is_some());
    }
}
#[test]
fn breadcrumb_targets_are_clickable_and_stay_clear_of_the_mode_switch() {
    use crate::source_editor::navigation::Crumb;
    for width in [480.0, 1024.0, 1440.0] {
        let mut w = Window::new(width, EN);
        w.state.source_language = Some(scholium_storage::project::ProjectLanguage::Latex);
        w.state.source_navigation.items = [
            ("very-long-project-member-name.tex", 0),
            ("Long introduction with Unicode 标题", 30),
            ("A method subsection", 80),
            ("equation", 120),
        ]
        .into_iter()
        .map(|(label, byte)| Crumb {
            label: label.into(),
            byte,
        })
        .collect();
        w.settle();
        let rects = w.rects("focus");
        assert!(
            rects
                .windows(2)
                .all(|pair| pair[0].right() <= pair[1].left()),
            "{width}: {rects:?}"
        );
        w.click("ui.breadcrumb.3", "focus");
        assert_eq!(w.state.source_navigation.jump, Some(120));
    }
}
