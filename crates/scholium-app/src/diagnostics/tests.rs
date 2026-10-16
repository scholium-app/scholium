//! Diagnostics list, status-bar segment and panel behaviour in headless egui frames.
use super::{Diagnostic, Severity, Target, count_text, excerpt, ordered};
use crate::{
    chrome::tests::{EN, Window, ZH, overflow},
    i18n::Locale,
};
use scholium_model::NodeId;

fn at(byte: usize) -> Target {
    Target {
        node: NodeId::fresh(),
        byte,
    }
}

fn located(key: &'static str, target: Target) -> Diagnostic {
    Diagnostic::new(Severity::Warning, key).at(target, Some("excerpt".into()))
}

#[test]
fn the_list_orders_by_severity_and_keeps_collection_order_within_one() {
    let list = ordered(vec![
        Diagnostic::new(Severity::Info, "diag.notice"),
        Diagnostic::new(Severity::Warning, "diag.rejected"),
        Diagnostic::new(Severity::Error, "diag.edit"),
        Diagnostic::new(Severity::Warning, "diag.formula_overflow"),
        Diagnostic::new(Severity::Error, "diag.save"),
    ]);
    let keys: Vec<_> = list.iter().map(|d| d.key).collect();
    assert_eq!(
        keys,
        [
            "diag.edit",
            "diag.save",
            "diag.rejected",
            "diag.formula_overflow",
            "diag.notice"
        ]
    );
}

#[test]
fn the_count_reads_gong_n_tiao_and_n_issues() {
    assert_eq!(count_text(Locale::ZhHans, 3), "共 3 条");
    assert_eq!(count_text(Locale::En, 3), "3 issues");
    assert_eq!(count_text(Locale::En, 1), "1 issue");
}

#[test]
fn messages_render_in_the_chosen_language_with_their_arguments() {
    let save = Diagnostic::new(Severity::Error, "diag.save").arg("error", "disk full");
    assert_eq!(save.text(Locale::ZhHans), "保存失败：disk full");
    assert_eq!(save.text(Locale::En), "Save failed: disk full");
    assert_eq!(overflow().text(Locale::ZhHans), "公式超出行宽");
}

#[test]
fn an_excerpt_is_one_short_line() {
    assert_eq!(excerpt("  "), None);
    assert_eq!(excerpt("a\n b").as_deref(), Some("a b"));
    let long = "字".repeat(60);
    let short = excerpt(&long).expect("excerpt");
    assert_eq!(short.chars().count(), 41);
    assert!(short.ends_with('…'));
}

fn texts_contain(w: &Window, text: &str) -> bool {
    w.texts().iter().any(|t| t == text)
}

#[test]
fn several_diagnostics_show_the_most_serious_and_the_count() {
    for (locale, count) in [(ZH, "共 2 条"), (EN, "2 issues")] {
        let mut w = Window::new(1280.0, locale);
        w.editor().problems = vec![
            Diagnostic::new(Severity::Error, "diag.edit").arg("detail", "boom"),
            overflow(),
        ];
        w.settle();
        assert!(texts_contain(&w, "boom"), "{:?}", w.texts());
        assert!(texts_contain(&w, count), "{:?}", w.texts());
    }
    let mut w = Window::new(1280.0, ZH);
    w.editor().problems = vec![overflow()];
    w.settle();
    assert!(
        !w.texts().iter().any(|t| t.contains("共")),
        "one issue shows no count"
    );
}

#[test]
fn clicking_a_located_warning_requests_a_jump_and_a_bare_one_opens_the_panel() {
    let mut w = Window::new(1280.0, ZH);
    let target = at(0);
    w.editor().problems = vec![located("diag.formula_overflow", target)];
    w.settle();
    w.click("ui.status.warning", "status");
    assert_eq!(w.state.jump_request, Some(target));
    assert!(
        !w.state.diagnostics,
        "a located warning does not open the panel"
    );
    w.state.jump_request = None;
    w.editor().problems = vec![Diagnostic::new(Severity::Error, "diag.edit").arg("detail", "x")];
    w.settle();
    w.click("ui.status.warning", "status");
    assert!(
        w.state.diagnostics,
        "a warning without a place opens the panel"
    );
    assert_eq!(w.state.jump_request, None);
}

#[test]
fn enter_on_the_focused_warning_does_what_a_click_does() {
    let mut w = Window::new(1280.0, ZH);
    let target = at(2);
    w.editor().problems = vec![located("diag.formula_overflow", target)];
    w.settle();
    w.focus("ui.status.warning", "status");
    w.press(egui_key(eframe::egui::Key::Enter));
    assert_eq!(w.state.jump_request, Some(target));
}

fn egui_key(key: eframe::egui::Key) -> eframe::egui::Event {
    eframe::egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: eframe::egui::Modifiers::NONE,
    }
}

#[test]
fn the_count_toggles_the_panel() {
    let mut w = Window::new(1280.0, ZH);
    w.editor().problems = vec![overflow(), overflow()];
    w.settle();
    assert!(!w.state.diagnostics);
    w.click("ui.status.count", "status");
    assert!(w.state.diagnostics);
    w.click("ui.status.count", "status");
    assert!(!w.state.diagnostics);
}

#[test]
fn the_panel_lists_rows_with_excerpts_and_an_empty_state() {
    for (locale, empty, problems) in [(ZH, "没有问题", "问题"), (EN, "No problems", "Problems")]
    {
        let mut w = Window::new(1280.0, locale);
        w.state.diagnostics = true;
        w.settle();
        assert!(texts_contain(&w, empty), "{:?}", w.texts());
        assert!(texts_contain(&w, problems));
        w.editor().problems = vec![located("diag.formula_overflow", at(0)), overflow()];
        w.settle();
        assert!(!texts_contain(&w, empty));
        assert!(texts_contain(&w, "excerpt"), "{:?}", w.texts());
        assert_eq!(w.ids("diagnostics"), ["ui.diagnostics.row"; 2]);
        let rows = w.rects("diagnostics");
        assert!(rows[1].min.y >= rows[0].max.y - 0.5, "rows stack: {rows:?}");
        assert_eq!(rows[0].min.x, rows[1].min.x);
    }
}

#[test]
fn the_panel_shows_layout_facts_and_the_status_bar_no_longer_does() {
    let mut w = Window::new(1280.0, ZH);
    w.editor().revision = 7;
    w.editor().shown_revision = Some(6);
    w.editor().pending = true;
    w.editor().layout = Some(super::LayoutStats {
        built: 3,
        reused: 9,
        source_reads: 0,
        elapsed_ms: 12,
    });
    w.state.diagnostics = true;
    w.settle();
    let texts = w.texts();
    for expected in [
        "后端：Typst Content",
        "正文 r7",
        "画面 r6 · 待更新",
        "Content 新建 3 / 复用 9 · source reads 0",
        "布局+栅格 12 ms",
    ] {
        assert!(
            texts.contains(&expected.to_owned()),
            "{expected} in {texts:?}"
        );
    }
    let stats_at = w.position_of("布局+栅格 12 ms");
    assert!(
        stats_at.y < w.status_top(),
        "the stats are in the panel, above the status bar"
    );
}

#[test]
fn clicking_a_row_requests_the_jump_to_its_location() {
    let mut w = Window::new(1280.0, ZH);
    let (first, second) = (at(0), at(1));
    w.editor().problems = vec![
        located("diag.formula_overflow", first),
        located("diag.formula_overflow", second),
    ];
    w.state.diagnostics = true;
    w.settle();
    w.click_nth("ui.diagnostics.row", "diagnostics", 1);
    assert_eq!(w.state.jump_request, Some(second));
}

#[test]
fn rows_move_with_up_and_down_and_escape_returns_to_the_page() {
    let mut w = Window::new(1280.0, ZH);
    w.editor().problems = vec![overflow(), overflow(), overflow()];
    w.state.diagnostics = true;
    w.settle();
    // Opening the panel puts the keyboard on the first row.
    assert_eq!(w.focused_row(), Some(0));
    w.press(egui_key(eframe::egui::Key::ArrowDown));
    w.settle();
    assert_eq!(w.focused_row(), Some(1));
    w.press(egui_key(eframe::egui::Key::ArrowDown));
    w.settle();
    assert_eq!(w.focused_row(), Some(2));
    w.press(egui_key(eframe::egui::Key::ArrowDown));
    w.settle();
    assert_eq!(w.focused_row(), Some(2), "stops at the last row");
    w.press(egui_key(eframe::egui::Key::ArrowUp));
    w.settle();
    assert_eq!(w.focused_row(), Some(1));
    assert!(!w.state.focus_requested);
    w.press(egui_key(eframe::egui::Key::Escape));
    assert!(
        w.state.focus_requested,
        "Esc hands the focus back to the page"
    );
}

#[test]
fn the_panel_never_covers_the_status_bar_and_is_a_resizable_bottom_dock() {
    let mut w = Window::new(1280.0, ZH);
    w.state.diagnostics = true;
    w.settle();
    let panel = w.panel_rect().expect("panel drawn");
    assert!(panel.bottom() <= w.status_top() + 0.5, "{panel:?}");
    assert!(
        (panel.height() - super::panel::DEFAULT_HEIGHT).abs() < 12.0,
        "{panel:?}"
    );
}

#[test]
fn the_panel_is_hidden_until_toggled() {
    let mut w = Window::new(1280.0, ZH);
    assert!(w.panel_rect().is_none());
    w.state.diagnostics = true;
    w.settle();
    assert!(w.panel_rect().is_some());
    w.state.diagnostics = false;
    w.settle();
    assert!(w.panel_rect().is_none());
}
