use super::*;
use scholium_model::diagnostic::{Diagnostic as Issue, DiagnosticBatch, DiagnosticKernel};

fn issue(resource: Option<&str>, severity: scholium_model::diagnostic::Severity) -> Issue {
    Issue {
        severity,
        kernel: DiagnosticKernel::Latex,
        resource: resource.map(str::to_owned),
        range: None,
        node: None,
        message: "compiler message".into(),
        code: None,
        log: None,
    }
}
#[test]
fn replacing_a_batch_drops_stale_results_and_a_good_build_clears_rows() {
    let mut panel = Problems::default();
    let batch = DiagnosticBatch {
        revision: 4,
        items: vec![issue(
            Some("main.tex"),
            scholium_model::diagnostic::Severity::Error,
        )],
    };
    panel.publish(4, batch.clone(), "log".into());
    assert_eq!(panel.batch.items.len(), 1);
    panel.publish(5, batch, "stale".into());
    assert!(panel.batch.items.is_empty());
    assert_eq!(panel.output, "log");
    panel.publish(
        5,
        DiagnosticBatch {
            revision: 5,
            items: vec![],
        },
        "good".into(),
    );
    assert!(panel.batch.items.is_empty());
    assert_eq!(panel.output, "good");
}
#[test]
fn next_and_previous_wrap_and_reset_when_the_list_changes() {
    let mut panel = Problems::default();
    panel.batch.items = vec![
        issue(Some("b.tex"), scholium_model::diagnostic::Severity::Warning),
        issue(Some("a.tex"), scholium_model::diagnostic::Severity::Error),
    ];
    assert_eq!(panel.cycle(false), Some(0));
    assert_eq!(panel.cycle(false), Some(1));
    assert_eq!(panel.cycle(false), Some(0));
    assert_eq!(panel.cycle(true), Some(1));
    panel.publish(0, DiagnosticBatch::default(), String::new());
    assert_eq!(panel.cycle(false), None);
    assert_eq!(panel.selected, None);
}
#[test]
fn groups_preserve_real_paths_and_unmapped_diagnostics_do_not_invent_a_file() {
    let items = vec![
        issue(
            Some("chap/a.tex"),
            scholium_model::diagnostic::Severity::Warning,
        ),
        issue(None, scholium_model::diagnostic::Severity::Error),
        issue(
            Some("chap/a.tex"),
            scholium_model::diagnostic::Severity::Error,
        ),
    ];
    let groups = groups(&items);
    assert_eq!(
        groups,
        vec![(Some("chap/a.tex"), vec![0, 2]), (None, vec![1])]
    );
}

#[test]
fn scalar_source_ranges_reject_invalid_positions_and_keep_exclusive_ends() {
    use scholium_model::diagnostic::{DiagnosticRange, SourcePoint};
    let range = DiagnosticRange {
        start: SourcePoint { line: 2, column: 2 },
        end: Some(SourcePoint { line: 2, column: 3 }),
        bytes: None,
    };
    assert_eq!(source_range("intro\n中éx", &range), Some(9..11));
    let bad = DiagnosticRange {
        start: SourcePoint { line: 9, column: 1 },
        ..range.clone()
    };
    assert!(source_range("intro\n中éx", &bad).is_none());
    let bad = DiagnosticRange {
        bytes: Some(7..8),
        ..range
    };
    assert!(source_range("intro\n中éx", &bad).is_none());
}

#[test]
fn keyboard_cycles_native_notices_without_duplicating_the_shared_batch() {
    let mut state = crate::state::WorkspaceState {
        editor: Some(crate::typst_editor::EditorView::default()),
        ..Default::default()
    };
    state.editor.as_mut().expect("view").problems = vec![
        Diagnostic::new(Severity::Warning, "diag.rejected"),
        Diagnostic::new(Severity::Error, "diag.edit"),
    ];
    for expected in [0, 1, 0] {
        cycle(&mut state, false);
        assert_eq!(state.problems.selected, Some(expected));
        assert_eq!(current_batch(&state).items.len(), 2);
    }
}

#[test]
fn shared_source_rows_have_file_locations_close_tabs_and_accessible_names() {
    use crate::chrome::tests::{EN, Window, ZH};
    use scholium_model::diagnostic::{DiagnosticRange, SourcePoint};
    for locale in [ZH, EN] {
        let mut window = Window::new(1280.0, locale);
        window.state.source_language = Some(scholium_storage::project::ProjectLanguage::Latex);
        let mut item = issue(
            Some("main.tex"),
            scholium_model::diagnostic::Severity::Error,
        );
        item.range = Some(DiagnosticRange {
            start: SourcePoint { line: 4, column: 2 },
            end: None,
            bytes: None,
        });
        window.state.problems.publish(
            0,
            DiagnosticBatch {
                revision: 0,
                items: vec![item],
            },
            "compiler output".into(),
        );
        window.state.diagnostics = true;
        window.settle();
        assert!(window.texts().contains(&"4:2".into()));
        assert!(window.texts().contains(&"compiler message".into()));
        let rows = window.rects("diagnostics");
        assert_eq!(rows[0].height(), 26.0);
        window.click("ui.menu.view", "menubar");
        window.hover("ui.menu.view.theme", "menu");
        assert!(window.ids("menu").contains(&"view.theme.light".into()));
        assert!(window.ids("menu").contains(&"view.theme.dark".into()));
        window.click("view.theme.dark", "menu");
        assert_eq!(
            window.ids("diagnostics").len(),
            1,
            "theme menus keep the dock and diagnostics alive"
        );
        window.click("ui.problems.output", "problem-header");
        assert!(window.texts().contains(&"compiler output".into()));
        assert!(window.ids("diagnostics").is_empty());
        window.click("ui.problems.tab", "problem-header");
        window.click("ui.problems.group", "problem-header");
        assert!(window.ids("diagnostics").is_empty());
        window.click("ui.problems.group", "problem-header");
        assert_eq!(window.ids("diagnostics").len(), 1);
        window.click("ui.problems.close", "problem-header");
        assert!(!window.state.diagnostics);
    }
}
