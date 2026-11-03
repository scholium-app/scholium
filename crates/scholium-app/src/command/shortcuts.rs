//! Native clipboard events and key events both resolve through the effective table.
use super::{
    exec, keys, registry,
    spec::{Action, Availability, ClipboardOp, CommandContext, Routing},
};
use crate::state::WorkspaceState;
use eframe::egui;

fn native_op(event: &egui::Event) -> Option<ClipboardOp> {
    match event {
        egui::Event::Copy => Some(ClipboardOp::Copy),
        egui::Event::Cut => Some(ClipboardOp::Cut),
        egui::Event::Paste(_) => Some(ClipboardOp::Paste),
        _ => None,
    }
}
fn clipboard_id(op: ClipboardOp) -> &'static str {
    match op {
        ClipboardOp::Copy => "edit.copy",
        ClipboardOp::Cut => "edit.cut",
        ClipboardOp::Paste => "edit.paste",
    }
}

pub(crate) fn shortcuts(ctx: &egui::Context, state: &mut WorkspaceState) {
    if state.has_document() {
        state.undo_requested = false;
        state.redo_requested = false;
    }
    let events = ctx.input(|i| i.events.clone());
    for event in events {
        route(ctx, state, event, super::keymap::effective());
    }
    super::help::show(ctx, state);
}
fn route(
    ctx: &egui::Context,
    state: &mut WorkspaceState,
    event: egui::Event,
    map: &super::keymap::Keymap,
) {
    let native = native_op(&event);
    let key_event = if let Some(op) = native {
        // winit converts native clipboard keys to events before egui sees them. Explicit
        // menu/remapped requests are tagged; physical defaults are resolved as data again.
        if crate::typst_editor::clipboard::take_request(ctx, op) {
            return;
        }
        super::keymap::defaults()
            .bindings
            .iter()
            .find(|b| b.command == clipboard_id(op))
            .and_then(|b| b.keys.first())
            .and_then(|k| keys::event(*k))
    } else if let egui::Event::Key {
        key,
        modifiers,
        pressed: true,
        ..
    } = event
    {
        Some((key, modifiers))
    } else {
        None
    };
    let Some((key, modifiers)) = key_event else {
        return;
    };
    let context = CommandContext::of(state);
    let page = state.mode == crate::state::ViewMode::Page;
    let focus = state.editor.as_ref().and_then(|v| v.focus_type);
    let spec = map
        .bindings
        .iter()
        .filter(|b| b.scope.matches(page, context.in_math, focus))
        .filter(|b| b.keys.iter().any(|k| keys::matches(*k, key, modifiers)))
        .max_by_key(|b| {
            (
                b.scope.priority(),
                b.keys.iter().any(|k| k.shift == modifiers.shift),
            )
        })
        .and_then(|b| registry::get(&b.command));
    let Some(spec) = spec else {
        if native.is_some() {
            consume(ctx, &event, key, modifiers);
        }
        return;
    };
    if matches!(
        (spec.availability)(&context),
        Availability::Unimplemented(_)
    ) {
        return;
    }
    if spec.routing == Routing::Editor {
        if native.is_some() {
            ctx.input_mut(|i| {
                for e in &mut i.events {
                    if e == &event {
                        *e = egui::Event::Key {
                            key,
                            physical_key: None,
                            modifiers,
                            pressed: true,
                            repeat: false,
                        };
                    }
                }
            });
        }
        return;
    }
    if let Action::Clipboard(op) = spec.action {
        if native == Some(op) {
            return;
        }
        consume(ctx, &event, key, modifiers);
        super::ui::invoke(ctx, state, spec.id);
        return;
    }
    consume(ctx, &event, key, modifiers);
    if spec.id == "help.shortcuts" {
        super::ui::invoke(ctx, state, spec.id);
    } else {
        exec::run(state, spec);
    }
}

fn consume(ctx: &egui::Context, event: &egui::Event, key: egui::Key, mods: egui::Modifiers) {
    ctx.input_mut(|input| {
        let mut suppress = false;
        input.events.retain(|e| {
            if e == event {
                suppress = mods.alt;
                return false;
            }
            if suppress && let egui::Event::Text(text) = e {
                suppress = false;
                return !text.eq_ignore_ascii_case(key.name());
            }
            if matches!(e, egui::Event::Key { .. }) {
                suppress = false;
            }
            true
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    fn route_with(
        map: &super::super::keymap::Keymap,
        event: egui::Event,
    ) -> (egui::Context, Vec<egui::Event>) {
        let ctx = egui::Context::default();
        let mut state = WorkspaceState {
            editor: Some(crate::typst_editor::EditorView {
                selected: true,
                ..Default::default()
            }),
            ..Default::default()
        };
        let input = egui::RawInput {
            events: vec![event.clone()],
            ..Default::default()
        };
        let mut remaining = Vec::new();
        let mut output = ctx.run_ui(input, |ui| {
            route(ui.ctx(), &mut state, event.clone(), map);
            remaining = ui.input(|i| i.events.clone());
        });
        output.textures_delta.clear();
        (ctx, remaining)
    }
    #[test]
    fn remapping_clipboard_consumes_the_native_old_binding_and_requests_the_new_one() {
        let map = super::super::keymap::defaults()
            .overlay("[[binding]]\ncommand='edit.copy'\nscope='global'\nkeys=['Ctrl+Shift+C']");
        let (_, remaining) = route_with(&map, egui::Event::Copy);
        assert!(remaining.is_empty(), "old native alias must be disabled");
        let (ctx, remaining) = route_with(
            &map,
            egui::Event::Key {
                key: egui::Key::C,
                physical_key: None,
                modifiers: egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                pressed: true,
                repeat: false,
            },
        );
        assert!(remaining.is_empty());
        assert!(crate::typst_editor::clipboard::take_request(
            &ctx,
            ClipboardOp::Copy
        ));
    }
    #[test]
    fn an_explicit_menu_clipboard_request_survives_an_unbound_physical_chord() {
        let map = super::super::keymap::defaults()
            .overlay("[[binding]]\ncommand='edit.copy'\nscope='global'\nkeys=[]");
        let ctx = egui::Context::default();
        crate::typst_editor::request_clipboard(&ctx, ClipboardOp::Copy);
        let mut state = WorkspaceState::default();
        let mut output = ctx.run_ui(
            egui::RawInput {
                events: vec![egui::Event::Copy],
                ..Default::default()
            },
            |ui| {
                route(ui.ctx(), &mut state, egui::Event::Copy, &map);
                assert!(ui.input(|i| i.events.contains(&egui::Event::Copy)));
            },
        );
        output.textures_delta.clear();
    }
}
