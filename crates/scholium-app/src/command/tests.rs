use super::{
    registry,
    spec::{Availability, CommandContext, CommandSpec, MenuId, Submenu},
};
use crate::{
    i18n::{self, Locale, LocaleSetting},
    state::{ViewMode, WorkspaceState},
    theme::ThemeKind,
};
use eframe::egui;
use std::collections::BTreeSet;

/// Catalog keys of submenus, for the catalog's unused-key check.
pub(crate) fn submenu_keys() -> Vec<String> {
    [
        Submenu::ParagraphStyle,
        Submenu::WorkMode,
        Submenu::Zoom,
        Submenu::Theme,
        Submenu::Language,
    ]
    .iter()
    .map(|s| s.key().to_owned())
    .collect()
}

/// Every reason key any command can report in any combination of facts.
pub(crate) fn reason_keys() -> BTreeSet<String> {
    let mut reasons = BTreeSet::new();
    for bits in 0..512u32 {
        let context = context_from(bits);
        for spec in registry::all() {
            if let Some(reason) = (spec.availability)(&context).reason() {
                reasons.insert(reason.to_owned());
            }
        }
    }
    reasons
}

fn context_from(bits: u32) -> CommandContext {
    let bit = |n: u32| bits & (1 << n) != 0;
    CommandContext {
        has_document: bit(0),
        mode: if bit(1) {
            ViewMode::Source
        } else {
            ViewMode::Page
        },
        composing: bit(2),
        in_math: bit(6),
        in_root: bit(7),
        in_delimiter: bit(8),
        selected: bit(3),
        can_undo: bit(4),
        can_redo: bit(5),
        strong: false,
        emphasis: false,
        navigation: false,
        diagnostics: false,
        fit_width: true,
        actual_size: false,
        toolbars_visible: true,
        theme: ThemeKind::Light,
        locale: LocaleSetting::Auto,
    }
}

fn editable() -> CommandContext {
    CommandContext {
        selected: true,
        can_undo: true,
        can_redo: true,
        ..context_from(0b1)
    }
}

fn spec(id: &str) -> &'static CommandSpec {
    registry::get(id).expect("registered command")
}

#[test]
fn command_ids_are_unique_dotted_names() {
    let mut seen = BTreeSet::new();
    for spec in registry::all() {
        assert!(seen.insert(spec.id), "duplicate id {}", spec.id);
        assert!(
            spec.id.split('.').all(|p| !p.is_empty()
                && p.chars()
                    .all(|c| c.is_ascii_lowercase() || c == '_' || c.is_ascii_digit())),
            "{}",
            spec.id
        );
        assert_eq!(spec.title, format!("command.{}.title", spec.id));
        assert_eq!(spec.tooltip, format!("command.{}.tooltip", spec.id));
    }
}

#[test]
fn every_command_has_a_title_and_tooltip_in_both_locales() {
    for locale in Locale::ALL {
        for spec in registry::all() {
            for key in [spec.title, spec.tooltip] {
                assert!(i18n::defines(locale, key), "{locale:?} lacks {key}");
            }
        }
    }
}

#[test]
fn every_reason_a_command_can_give_is_in_the_catalog() {
    let reasons = reason_keys();
    assert!(reasons.len() >= 6, "{reasons:?}");
    for locale in Locale::ALL {
        for reason in &reasons {
            assert!(i18n::defines(locale, reason), "{locale:?} lacks {reason}");
        }
    }
}

#[test]
fn every_p1_menu_lists_its_commands() {
    let expected: [(MenuId, &[&str]); 8] = [
        (
            MenuId::File,
            &[
                "file.new",
                "file.open",
                "file.recent",
                "file.save",
                "file.save_as",
                "file.close",
            ],
        ),
        (
            MenuId::Edit,
            &[
                "edit.undo",
                "edit.redo",
                "edit.cut",
                "edit.copy",
                "edit.paste",
            ],
        ),
        (
            MenuId::Insert,
            &[
                "insert.formula",
                "insert.display_formula",
                "insert.fraction",
                "insert.root",
                "insert.superscript",
                "insert.subscript",
                "insert.delimiter",
            ],
        ),
        (
            MenuId::Format,
            &["format.bold", "format.italic", "format.clear"],
        ),
        (MenuId::Document, &[]),
        (
            MenuId::View,
            &["view.navigation", "view.diagnostics", "view.toolbars"],
        ),
        (MenuId::Tools, &[]),
        (MenuId::Help, &["help.shortcuts"]),
    ];
    for (menu, ids) in expected {
        let listed: Vec<_> = registry::menu_items(menu, None)
            .iter()
            .map(|s| s.id)
            .collect();
        for id in ids {
            assert!(listed.contains(id), "{menu:?} lacks {id}");
        }
    }
}

#[test]
fn the_format_and_view_menus_keep_their_submenus() {
    let styles: Vec<_> = registry::menu_items(MenuId::Format, Some(Submenu::ParagraphStyle))
        .iter()
        .map(|s| s.id)
        .collect();
    assert_eq!(
        styles,
        [
            "format.paragraph.body",
            "format.paragraph.heading1",
            "format.paragraph.heading2"
        ]
    );
    assert_eq!(
        registry::submenus(MenuId::Format),
        [Submenu::ParagraphStyle]
    );
    assert_eq!(
        registry::submenus(MenuId::View),
        [
            Submenu::WorkMode,
            Submenu::Zoom,
            Submenu::Theme,
            Submenu::Language
        ]
    );
}

#[test]
fn source_mode_disables_every_writing_command_with_a_reason() {
    let source = CommandContext {
        mode: ViewMode::Source,
        ..editable()
    };
    for id in [
        "format.bold",
        "format.paragraph.heading1",
        "insert.formula",
        "insert.display_formula",
        "edit.paste",
    ] {
        assert_eq!(
            (spec(id).availability)(&source),
            Availability::Disabled("reason.source_readonly"),
            "{id}"
        );
        assert!((spec(id).availability)(&editable()).is_enabled(), "{id}");
    }
}

#[test]
fn composition_blocks_writes_file_actions_and_mode_switches() {
    let composing = CommandContext {
        composing: true,
        ..editable()
    };
    for id in [
        "format.bold",
        "file.new",
        "file.save",
        "view.mode.source",
        "view.mode.toggle",
        "edit.undo",
    ] {
        assert_eq!(
            (spec(id).availability)(&composing),
            Availability::Disabled("reason.composing"),
            "{id}"
        );
    }
    // View preferences stay available while composing.
    assert!((spec("view.zoom.in").availability)(&composing).is_enabled());
}

#[test]
fn unimplemented_commands_are_absent_from_the_registry_and_keymap() {
    for id in [
        "edit.find",
        "format.underline",
        "document.page_size",
        "document.final_check",
        "tools.search",
    ] {
        assert!(registry::get(id).is_none(), "{id}");
        assert!(super::keymap::effective().hint(id).is_none(), "{id}");
    }
}

#[test]
fn history_and_clipboard_availability_follow_the_facts() {
    let none = CommandContext {
        can_undo: false,
        can_redo: false,
        selected: false,
        ..editable()
    };
    assert_eq!(
        (spec("edit.undo").availability)(&none),
        Availability::Disabled("reason.nothing_to_undo")
    );
    assert_eq!(
        (spec("edit.redo").availability)(&none),
        Availability::Disabled("reason.nothing_to_redo")
    );
    assert_eq!(
        (spec("edit.copy").availability)(&none),
        Availability::Disabled("reason.no_selection")
    );
    assert!((spec("edit.copy").availability)(&editable()).is_enabled());
    assert_eq!(
        (spec("file.save").availability)(&context_from(0)),
        Availability::Disabled("reason.no_document")
    );
}

fn press(state: &mut WorkspaceState, key: egui::Key, modifiers: egui::Modifiers) {
    let ctx = egui::Context::default();
    let input = egui::RawInput {
        events: vec![egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }],
        ..Default::default()
    };
    let mut output = ctx.run_ui(input, |ui| super::shortcuts::shortcuts(ui.ctx(), state));
    output.textures_delta.clear();
}

fn with_history() -> WorkspaceState {
    WorkspaceState {
        editor: Some(crate::typst_editor::EditorView {
            can_undo: true,
            can_redo: true,
            ..Default::default()
        }),
        ..Default::default()
    }
}

#[test]
fn ctrl_shift_b_toggles_navigation_and_plain_ctrl_b_is_left_to_the_editor() {
    let mut state = WorkspaceState::default();
    press(
        &mut state,
        egui::Key::B,
        egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
    );
    assert!(state.navigation);
    let mut state = WorkspaceState::default();
    press(&mut state, egui::Key::B, egui::Modifiers::COMMAND);
    assert!(!state.navigation);
}

#[test]
fn ctrl_shift_m_toggles_the_work_mode_both_ways() {
    let mut state = WorkspaceState::default();
    let chord = egui::Modifiers::COMMAND | egui::Modifiers::SHIFT;
    press(&mut state, egui::Key::M, chord);
    assert_eq!(state.mode, ViewMode::Source);
    press(&mut state, egui::Key::M, chord);
    assert_eq!(state.mode, ViewMode::Page);
    assert!(
        state.focus_requested,
        "returning to the page asks for focus back"
    );
}

#[test]
fn ctrl_1_and_ctrl_2_leave_the_work_mode_alone() {
    let mut state = WorkspaceState::default();
    press(&mut state, egui::Key::Num1, egui::Modifiers::COMMAND);
    press(&mut state, egui::Key::Num2, egui::Modifiers::COMMAND);
    assert_eq!(state.mode, ViewMode::Page);
    state.mode = ViewMode::Source;
    press(&mut state, egui::Key::Num1, egui::Modifiers::COMMAND);
    assert_eq!(state.mode, ViewMode::Source);
}

#[test]
fn undo_and_redo_chords_never_fire_each_other() {
    let shift = egui::Modifiers::COMMAND | egui::Modifiers::SHIFT;
    let mut state = with_history();
    press(&mut state, egui::Key::Z, egui::Modifiers::COMMAND);
    assert!((state.undo_requested, state.redo_requested) == (true, false));
    let mut state = with_history();
    press(&mut state, egui::Key::Z, shift);
    assert!((state.undo_requested, state.redo_requested) == (false, true));
    let mut state = with_history();
    press(&mut state, egui::Key::Y, egui::Modifiers::COMMAND);
    assert!((state.undo_requested, state.redo_requested) == (false, true));
}

#[test]
fn zoom_chords_still_work_with_shift_held() {
    let mut state = WorkspaceState::default();
    press(
        &mut state,
        egui::Key::Plus,
        egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
    );
    assert!(!state.fit_width, "Ctrl+Shift+= is Ctrl+Plus on a US layout");
}

#[test]
fn the_same_registry_title_renders_in_each_language_without_restart() {
    let mut state = WorkspaceState::default();
    let title = |state: &WorkspaceState| super::ui::text(state, spec("view.mode.source").title);
    state.locale = LocaleSetting::ZhHans;
    assert_eq!(title(&state), "源码预览");
    state.locale = LocaleSetting::En;
    assert_eq!(title(&state), "Source Preview");
    state.locale = LocaleSetting::ZhHans;
    assert_eq!(title(&state), "源码预览");
}

#[test]
fn script_commands_need_the_caret_in_a_formula_outside_source_and_composition() {
    let in_formula = CommandContext {
        in_math: true,
        ..editable()
    };
    for id in ["insert.superscript", "insert.subscript"] {
        assert!((spec(id).availability)(&in_formula).is_enabled(), "{id}");
        assert_eq!(
            (spec(id).availability)(&editable()),
            Availability::Disabled("reason.not_in_formula"),
            "{id}"
        );
        let source = CommandContext {
            mode: ViewMode::Source,
            ..in_formula
        };
        assert_eq!(
            (spec(id).availability)(&source),
            Availability::Disabled("reason.source_readonly")
        );
        let composing = CommandContext {
            composing: true,
            ..in_formula
        };
        assert_eq!(
            (spec(id).availability)(&composing),
            Availability::Disabled("reason.composing")
        );
    }
    assert_eq!(super::keymap::effective().hint("insert.superscript"), None);
    assert_eq!(super::keymap::effective().hint("insert.subscript"), None);
}
#[test]
fn labelled_menu_commands_do_not_repeat_labels_in_tooltips() {
    let state = crate::state::WorkspaceState::default();
    let command = super::ui::resolve(&state, "file.new").expect("implemented command");
    assert!(command.meta(&state, "menu").tip.is_none());
    assert!(
        command.meta(&state, "toolbar").tip.is_some(),
        "icons need a name and shortcut"
    );
}

#[test]
fn pdf_export_is_available_for_every_open_project_kernel() {
    let export = registry::get("file.export").expect("shared PDF export command");
    for language in [
        None,
        Some(scholium_storage::project::ProjectLanguage::Typst),
        Some(scholium_storage::project::ProjectLanguage::Latex),
    ] {
        let state = WorkspaceState {
            editor: Some(Default::default()),
            source_language: language,
            ..Default::default()
        };
        assert!(export.visible(&state));
        assert!((export.availability)(&CommandContext::of(&state)).is_enabled());
    }
    assert!(!(export.availability)(&CommandContext::of(&WorkspaceState::default())).is_enabled());
    assert!(super::keymap::effective().hint("file.export").is_some());
}
