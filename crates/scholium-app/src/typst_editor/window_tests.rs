//! Whole-window frames: shortcuts, chrome, Ribbon and the editor in one egui pass.
use super::*;
use crate::{chrome, i18n::LocaleSetting, state::ViewMode, typst_editor::tests::Fixture};

fn frame(f: &mut Fixture, size: egui::Vec2, events: Vec<egui::Event>) -> egui::FullOutput {
    let mut output = f.ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
            events,
            ..Default::default()
        },
        |ui| {
            crate::command::shortcuts::shortcuts(ui.ctx(), &mut f.state);
            chrome::show(ui, &mut f.state);
            f.editor.show(ui, &mut f.state);
        },
    );
    output.textures_delta.clear();
    output
}

fn window() -> Fixture {
    let f = Fixture::new();
    crate::theme::install(&f.ctx);
    f
}

fn command(key: egui::Key) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::COMMAND,
    }
}

fn toggle_mode() -> egui::Event {
    egui::Event::Key {
        key: egui::Key::M,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
    }
}

#[test]
fn whole_window_stays_inside_tiled_sizes_in_both_modes() {
    let mut f = window();
    f.settle();
    for (mode, navigation) in [
        (ViewMode::Visual, false),
        (ViewMode::Visual, true),
        (ViewMode::Source, false),
    ] {
        f.state.mode = mode;
        f.state.navigation = navigation;
        for size in [
            egui::vec2(480.0, 480.0),
            egui::vec2(640.0, 480.0),
            egui::vec2(844.0, 1011.0),
            egui::vec2(1280.0, 900.0),
        ] {
            let output = frame(&mut f, size, vec![]);
            let bounds = output
                .shapes
                .iter()
                .map(|s| s.shape.visual_bounding_rect().intersect(s.clip_rect))
                .reduce(|a, b| a.union(b))
                .expect("chrome produces shapes");
            assert!(
                egui::Rect::from_min_size(egui::Pos2::ZERO, size).contains_rect(bounds),
                "{mode:?} {size:?} {bounds:?}"
            );
        }
    }
}

#[test]
fn view_shortcuts_switch_modes_without_touching_the_document() {
    let mut f = window();
    f.events(vec![tests::text("abc")]);
    f.settle();
    let before = f.editor.record();
    let size = egui::vec2(1280.0, 900.0);
    frame(&mut f, size, vec![toggle_mode()]);
    assert_eq!(f.state.mode, ViewMode::Source);
    frame(&mut f, size, vec![command(egui::Key::J)]);
    assert!(f.state.diagnostics);
    frame(&mut f, size, vec![toggle_mode()]);
    assert_eq!(f.state.mode, ViewMode::Visual);
    frame(&mut f, size, vec![command(egui::Key::F1)]);
    assert!(!f.state.toolbars_visible);
    assert_eq!(f.editor.record().snapshot, before.snapshot);
}

#[test]
fn toolbar_intents_apply_in_order_through_the_editor_session() {
    let mut f = window();
    f.events(vec![tests::text("x")]);
    f.settle();
    if let Some(view) = &mut f.state.editor {
        view.commands.extend([Command::Math, Command::Fraction]);
    }
    frame(&mut f, egui::vec2(1280.0, 900.0), vec![]);
    let values: Vec<_> = input::leaves(&f.editor.snapshot)
        .iter()
        .map(|l| l.text.to_owned())
        .collect();
    assert_eq!(
        values,
        ["x", "", "", ""],
        "formula, then a fraction around its slot"
    );
    assert!(
        f.state
            .editor
            .as_ref()
            .is_some_and(|v| v.commands.is_empty())
    );
}

#[test]
fn save_shortcut_persists_through_the_editor_and_clears_dirty() {
    let mut f = window();
    f.events(vec![tests::text("saved")]);
    f.settle();
    assert!(f.state.unsaved());
    frame(
        &mut f,
        egui::vec2(1280.0, 900.0),
        vec![command(egui::Key::S)],
    );
    assert!(!f.state.unsaved());
    assert!(f.state.storage_error.is_none());
}

#[test]
fn returning_from_source_view_restores_typing_on_the_page() {
    let mut f = window();
    f.settle();
    let size = egui::vec2(1280.0, 900.0);
    frame(&mut f, size, vec![]);
    frame(&mut f, size, vec![toggle_mode()]);
    frame(&mut f, size, vec![]);
    frame(&mut f, size, vec![toggle_mode()]);
    frame(&mut f, size, vec![]);
    frame(&mut f, size, vec![tests::text("k")]);
    assert_eq!(input::leaves(&f.editor.snapshot)[0].text, "k");
}

#[test]
fn toolbar_intents_queued_in_source_view_never_reach_the_document() {
    let mut f = window();
    f.events(vec![tests::text("x")]);
    f.settle();
    f.state.mode = ViewMode::Source;
    let revision = f.editor.snapshot.revision;
    if let Some(view) = &mut f.state.editor {
        view.commands
            .extend([Command::Math, Command::Style(TextStyle::Strong)]);
    }
    frame(&mut f, egui::vec2(1280.0, 900.0), vec![]);
    assert_eq!(f.editor.snapshot.revision, revision);
    assert!(f.state.edit_error.is_some());
}

fn painted_text(output: &egui::FullOutput) -> Vec<(egui::Pos2, String)> {
    output
        .shapes
        .iter()
        .filter_map(|s| match &s.shape {
            egui::Shape::Text(t) => Some((t.pos, t.galley.text().to_owned())),
            _ => None,
        })
        .collect()
}

const STATUS_HEIGHT: f32 = 24.0;

fn status_text(output: &egui::FullOutput, height: f32) -> String {
    painted_text(output)
        .into_iter()
        .filter(|(pos, _)| pos.y > height - STATUS_HEIGHT)
        .map(|(_, text)| text)
        .collect::<Vec<_>>()
        .join(" | ")
}

fn page_top(f: &Fixture) -> f32 {
    f.editor.placement.expect("page placed").0.min.y
}

#[test]
fn no_developer_text_is_painted_above_the_page() {
    let mut f = window();
    f.events(vec![tests::text("x")]);
    f.settle();
    let size = egui::vec2(1280.0, 900.0);
    frame(&mut f, size, vec![]);
    let output = frame(&mut f, size, vec![]);
    let top = page_top(&f);
    let stray: Vec<_> = painted_text(&output)
        .into_iter()
        .filter(|(pos, _)| pos.y < top && pos.y > top - 24.0)
        .collect();
    assert!(stray.is_empty(), "{stray:?}");
    for (_, text) in painted_text(&output) {
        assert!(
            !text.contains("受控单页流") && !text.contains("当前叶"),
            "{text}"
        );
    }
}

#[test]
fn page_stays_put_when_an_error_appears_and_the_status_bar_shows_it() {
    let mut f = window();
    f.settle();
    let size = egui::vec2(1280.0, 900.0);
    frame(&mut f, size, vec![]);
    let before = page_top(&f);
    f.state.edit_error = Some("编辑失败：测试".into());
    frame(&mut f, size, vec![]);
    let output = frame(&mut f, size, vec![]);
    assert_eq!(page_top(&f), before);
    assert!(status_text(&output, size.y).contains("编辑失败：测试"));
}

#[test]
fn page_is_centered_in_a_wide_viewport() {
    let mut f = window();
    f.settle();
    f.state.fit_width = false;
    f.state.zoom = 0.5;
    let size = egui::vec2(1280.0, 900.0);
    frame(&mut f, size, vec![]);
    frame(&mut f, size, vec![]);
    let (rect, _) = f.editor.placement.expect("page placed");
    let (left, right) = (rect.left(), size.x - rect.right());
    assert!((left - right).abs() < 20.0, "left {left} right {right}");
}

#[test]
fn status_bar_is_redrawn_once_a_stale_marker_clears() {
    let mut f = window();
    f.settle();
    let size = egui::vec2(1280.0, 900.0);
    frame(&mut f, size, vec![]);
    frame(&mut f, size, vec![]);
    assert!(
        !f.editor.metadata(&mut f.state),
        "settled view is unchanged"
    );
    // The status bar paints from the previous frame's view; the editor then adopts the Scene.
    if let Some(view) = &mut f.state.editor {
        view.pending = true;
    }
    frame(&mut f, size, vec![]);
    assert!(!f.state.editor.as_ref().is_some_and(|v| v.pending));
}

#[test]
fn metadata_reports_changes_the_status_bar_shows() {
    let mut f = window();
    f.settle();
    frame(&mut f, egui::vec2(1280.0, 900.0), vec![]);
    f.editor.metadata(&mut f.state);
    f.state.edit_error = Some("失败".into());
    assert!(f.editor.metadata(&mut f.state), "new warning");
    assert!(!f.editor.metadata(&mut f.state), "unchanged");
    f.state.edit_error = None;
    assert!(f.editor.metadata(&mut f.state), "warning cleared");
}

#[test]
fn ctrl_1_and_ctrl_2_no_longer_switch_the_work_mode() {
    let mut f = window();
    f.settle();
    let size = egui::vec2(1280.0, 900.0);
    frame(&mut f, size, vec![]);
    frame(&mut f, size, vec![command(egui::Key::Num2)]);
    assert_eq!(f.state.mode, ViewMode::Visual);
    f.state.mode = ViewMode::Source;
    frame(&mut f, size, vec![command(egui::Key::Num1)]);
    assert_eq!(f.state.mode, ViewMode::Source);
}

#[test]
fn escape_never_drops_the_pages_keyboard_focus() {
    let mut f = window();
    f.events(vec![tests::text("$"), tests::text("1")]);
    f.settle();
    let size = egui::vec2(1280.0, 900.0);
    frame(&mut f, size, vec![]);
    frame(&mut f, size, vec![]);
    assert!(f.ctx.memory(|m| m.has_focus(paint::id())));
    for _ in 0..2 {
        frame(&mut f, size, vec![tests::key(egui::Key::Escape, false)]);
        assert!(f.ctx.memory(|m| m.has_focus(paint::id())));
    }
    assert!(!f.editor.in_math(), "the first Esc left the formula");
}

#[test]
fn the_interface_language_switches_at_runtime_without_touching_the_document() {
    let mut f = window();
    f.settle();
    let size = egui::vec2(1280.0, 900.0);
    let before = f.editor.record().snapshot;
    let mut shown = Vec::new();
    for locale in [LocaleSetting::En, LocaleSetting::ZhHans, LocaleSetting::En] {
        f.state.locale = locale;
        frame(&mut f, size, vec![]);
        let output = frame(&mut f, size, vec![]);
        shown.push(
            painted_text(&output)
                .into_iter()
                .map(|(_, t)| t)
                .collect::<Vec<_>>(),
        );
    }
    assert!(shown[0].iter().any(|t| t == "File"));
    assert!(shown[1].iter().any(|t| t == "文件") && !shown[1].iter().any(|t| t == "File"));
    assert!(shown[2].iter().any(|t| t == "File"));
    assert_eq!(f.editor.record().snapshot, before);
}

fn zh_window() -> Fixture {
    let mut f = window();
    f.state.locale = LocaleSetting::ZhHans;
    f.settle();
    f
}

#[test]
fn a_jump_request_moves_the_caret_in_order_and_is_consumed() {
    let mut f = zh_window();
    f.events(vec![
        tests::text("first"),
        tests::key(egui::Key::Enter, false),
        tests::text("second"),
    ]);
    f.settle();
    let first = input::leaves(&f.editor.snapshot)[0].id;
    f.state.jump_request = Some(crate::diagnostics::Target {
        node: first,
        byte: 2,
    });
    frame(&mut f, egui::vec2(1280.0, 900.0), vec![]);
    assert_eq!(f.state.jump_request, None);
    assert_eq!((f.editor.cursor.leaf, f.editor.cursor.byte), (first, 2));
}

#[test]
fn ctrl_j_docks_the_panel_and_layout_stats_leave_the_status_bar() {
    let mut f = zh_window();
    let size = egui::vec2(1280.0, 900.0);
    frame(&mut f, size, vec![]);
    let closed = frame(&mut f, size, vec![]);
    assert!(!painted_text(&closed).iter().any(|(_, t)| t == "问题"));
    frame(&mut f, size, vec![command(egui::Key::J)]);
    frame(&mut f, size, vec![]);
    let open = frame(&mut f, size, vec![]);
    assert!(f.state.diagnostics);
    let texts = painted_text(&open);
    assert!(texts.iter().any(|(_, t)| t == "问题"), "{texts:?}");
    assert!(texts.iter().any(|(_, t)| t == "没有问题"), "{texts:?}");
    let stats = texts
        .iter()
        .find(|(_, t)| t.starts_with("Content 新建"))
        .expect("layout stats in the panel");
    assert!(stats.0.y < size.y - STATUS_HEIGHT, "above the status bar");
    assert!(
        !status_text(&open, size.y).contains("Content 新建"),
        "{}",
        status_text(&open, size.y)
    );
    frame(&mut f, size, vec![command(egui::Key::J)]);
    let shut = frame(&mut f, size, vec![]);
    assert!(!f.state.diagnostics);
    assert!(!painted_text(&shut).iter().any(|(_, t)| t == "问题"));
}

fn click_at(f: &mut Fixture, size: egui::Vec2, pos: egui::Pos2) {
    let button = |pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    frame(f, size, vec![egui::Event::PointerMoved(pos)]);
    // A real click can deliver the press and the release in one frame.
    frame(f, size, vec![button(true), button(false)]);
    frame(f, size, vec![]);
}

#[test]
fn clicking_the_status_bar_warning_jumps_without_the_page_taking_the_click() {
    let mut f = zh_window();
    f.events(vec![
        tests::text("$"),
        tests::text(&"abcdefghij".repeat(10)),
        tests::key(egui::Key::ArrowRight, false),
        tests::text(" tail"),
    ]);
    f.settle();
    let size = egui::vec2(1280.0, 900.0);
    for _ in 0..3 {
        frame(&mut f, size, vec![]);
    }
    let formula = input::leaves(&f.editor.snapshot)
        .into_iter()
        .find(|l| l.math)
        .expect("formula leaf")
        .id;
    assert_ne!(f.editor.cursor.leaf, formula);
    let warning = crate::widgets::audit::entries(&f.ctx)
        .into_iter()
        .find(|e| e.id == "ui.status.warning")
        .expect("the overflow warning is in the status bar");
    click_at(&mut f, size, warning.rect.center());
    assert_eq!(
        f.editor.cursor.leaf, formula,
        "the caret lands in the formula"
    );
    assert_eq!(f.editor.cursor.byte, 0);
}

#[test]
fn clicking_a_panel_row_jumps_without_the_page_taking_the_click() {
    let mut f = zh_window();
    f.events(vec![
        tests::text("$"),
        tests::text(&"abcdefghij".repeat(10)),
        tests::key(egui::Key::ArrowRight, false),
        tests::text(" tail"),
    ]);
    f.settle();
    let size = egui::vec2(1280.0, 900.0);
    f.state.diagnostics = true;
    for _ in 0..4 {
        frame(&mut f, size, vec![]);
    }
    let formula = input::leaves(&f.editor.snapshot)
        .into_iter()
        .find(|l| l.math)
        .expect("formula leaf")
        .id;
    let row = crate::widgets::audit::entries(&f.ctx)
        .into_iter()
        .find(|e| e.id == crate::diagnostics::panel::ROW_ID)
        .expect("a panel row");
    click_at(&mut f, size, row.rect.center());
    assert_eq!(f.editor.cursor.leaf, formula);
}

#[test]
fn enter_on_a_panel_row_jumps_and_is_not_typed_into_the_page() {
    let mut f = zh_window();
    f.events(vec![
        tests::text("$"),
        tests::text(&"abcdefghij".repeat(10)),
        tests::key(egui::Key::ArrowRight, false),
        tests::text(" tail"),
    ]);
    f.settle();
    let size = egui::vec2(1280.0, 900.0);
    f.state.diagnostics = true;
    for _ in 0..4 {
        frame(&mut f, size, vec![]);
    }
    let formula = input::leaves(&f.editor.snapshot)
        .into_iter()
        .find(|l| l.math)
        .expect("formula leaf")
        .id;
    let revision = f.editor.snapshot.revision;
    f.ctx
        .memory_mut(|m| m.request_focus(crate::diagnostics::panel::row_id(0)));
    frame(&mut f, size, vec![]);
    frame(&mut f, size, vec![tests::key(egui::Key::Enter, false)]);
    frame(&mut f, size, vec![]);
    assert_eq!(f.editor.cursor.leaf, formula);
    assert_eq!(
        f.editor.snapshot.revision, revision,
        "Enter did not split the paragraph"
    );
}
