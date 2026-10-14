use super::{
    registry,
    spec::{
        Availability, ChordKey, CommandContext, CommandSpec, KeyChord, MenuId, Routing, Submenu,
    },
};
use crate::{
    i18n::{self, Locale, LocaleSetting},
    state::{ViewMode, WorkspaceState},
};
use eframe::egui;
use std::collections::BTreeSet;

/// Catalog keys of submenus, for the catalog's unused-key check.
pub(crate) fn submenu_keys() -> Vec<String> {
    [Submenu::ParagraphStyle, Submenu::WorkMode, Submenu::Zoom]
        .iter()
        .map(|s| s.key().to_owned())
        .collect()
}

/// Every reason key any command can report in any combination of facts.
pub(crate) fn reason_keys() -> BTreeSet<String> {
    let mut reasons = BTreeSet::new();
    for bits in 0..64u32 {
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
            ViewMode::Visual
        },
        composing: bit(2),
        in_math: false,
        selected: bit(3),
        can_undo: bit(4),
        can_redo: bit(5),
        strong: false,
        emphasis: false,
        navigation: false,
        diagnostics: false,
        fit_width: true,
        actual_size: false,
        ribbon_collapsed: false,
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
fn no_two_commands_share_a_chord() {
    let mut owners: Vec<(KeyChord, &str)> = Vec::new();
    for spec in registry::all() {
        for chord in spec.shortcuts {
            if let Some((_, other)) = owners.iter().find(|(c, _)| c == chord) {
                panic!("{} and {} both bind {}", other, spec.id, chord.display());
            }
            owners.push((*chord, spec.id));
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
                "file.export",
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
                "edit.find",
            ],
        ),
        (
            MenuId::Insert,
            &[
                "insert.formula",
                "insert.fraction",
                "insert.root",
                "insert.attach",
                "insert.delimiter",
            ],
        ),
        (
            MenuId::Format,
            &[
                "format.bold",
                "format.italic",
                "format.underline",
                "format.strike",
                "format.clear",
            ],
        ),
        (
            MenuId::Document,
            &[
                "document.style_template",
                "document.page_size",
                "document.language",
                "document.font",
                "document.font_size",
                "document.final_check",
            ],
        ),
        (
            MenuId::View,
            &[
                "view.navigation",
                "view.diagnostics",
                "view.split",
                "view.theme",
            ],
        ),
        (MenuId::Tools, &["tools.search", "tools.preferences"]),
        (
            MenuId::Help,
            &["help.shortcuts", "help.commands", "help.about"],
        ),
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
        [Submenu::WorkMode, Submenu::Zoom]
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
fn unimplemented_commands_are_greyed_with_a_reason() {
    for id in [
        "file.open",
        "edit.find",
        "format.underline",
        "insert.root",
        "document.page_size",
    ] {
        assert_eq!(
            (spec(id).availability)(&editable()),
            Availability::Unimplemented("reason.unimplemented"),
            "{id}"
        );
    }
    assert_eq!(
        (spec("document.final_check").availability)(&editable()),
        Availability::Unimplemented("reason.final_build")
    );
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

#[test]
fn page_chords_are_registered_for_the_page_and_app_chords_for_the_app() {
    let ctrl_shift = |c| KeyChord::ctrl_shift(ChordKey::Letter(c));
    let ctrl = |key| KeyChord::ctrl(key);
    let routed = |chord| registry::editor_command(chord).map(|s| s.id);
    assert_eq!(
        routed(ctrl(ChordKey::Digit(0))),
        Some("format.paragraph.body")
    );
    assert_eq!(
        routed(ctrl(ChordKey::Digit(1))),
        Some("format.paragraph.heading1")
    );
    assert_eq!(
        routed(ctrl(ChordKey::Digit(2))),
        Some("format.paragraph.heading2")
    );
    assert_eq!(routed(ctrl(ChordKey::Letter('M'))), Some("insert.formula"));
    assert_eq!(
        routed(ctrl_shift('M')),
        None,
        "Ctrl+Shift+M is the mode switch"
    );
    assert_eq!(
        routed(ctrl_shift('B')),
        None,
        "Ctrl+Shift+B is the navigation toggle"
    );
    assert_eq!(
        routed(KeyChord::ctrl_shift(ChordKey::Slash)),
        Some("insert.fraction")
    );
    assert!(
        registry::app_shortcuts()
            .iter()
            .all(|(_, s)| s.routing == Routing::App)
    );
    assert!(
        registry::app_shortcuts()[0].0.shift,
        "shifted chords go first"
    );
    assert_eq!(spec("view.mode.toggle").shortcuts, [ctrl_shift('M')]);
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
    assert_eq!(state.mode, ViewMode::Visual);
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
    assert_eq!(state.mode, ViewMode::Visual);
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
fn every_menu_renders_in_both_locales_for_each_mode() {
    let ctx = egui::Context::default();
    crate::theme::install(&ctx);
    for locale in [LocaleSetting::ZhHans, LocaleSetting::En] {
        for mode in [ViewMode::Visual, ViewMode::Source] {
            let mut state = WorkspaceState {
                locale,
                mode,
                ..Default::default()
            };
            let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
                for menu in MenuId::ALL {
                    super::ui::menu(ui, &mut state, menu);
                }
            });
            output.textures_delta.clear();
        }
    }
}
