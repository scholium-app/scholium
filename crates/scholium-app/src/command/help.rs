//! 帮助 menu.
use super::{avail, spec::CommandSpec};

pub(super) fn specs() -> Vec<CommandSpec> {
    vec![
        command! {
            "help.shortcuts",
            menu: menu!(Help, 0, 0),

        },
        command! {
            "help.commands",
            menu: menu!(Help, 0, 1),
            availability: avail::unimplemented,
        },
        command! {
            "help.about",
            menu: menu!(Help, 1, 0),
            availability: avail::unimplemented,
        },
    ]
}

pub(crate) fn show(ctx: &eframe::egui::Context, state: &crate::state::WorkspaceState) -> bool {
    use eframe::egui;
    let id = egui::Id::new("shortcut-help");
    let mut open = ctx.data(|d| d.get_temp::<bool>(id).unwrap_or(false));
    if open {
        let buttons = [crate::widgets::DialogButton::new(
            "ok",
            super::ui::text(state, "project.ok"),
            true,
        )];
        if crate::widgets::dialog(
            ctx,
            "shortcut-help-dialog",
            &super::ui::text(state, "command.help.shortcuts.title"),
            state.locale.resolve(),
            &buttons,
            |ui| {
                let map = super::keymap::effective();
                if let Some(error) = &map.diagnostic {
                    ui.label(error);
                }
                for binding in &map.bindings {
                    let Some(spec) =
                        super::registry::get(&binding.command).filter(|s| s.visible(state))
                    else {
                        continue;
                    };
                    if binding.keys.is_empty() {
                        continue;
                    }
                    ui.horizontal(|ui| {
                        ui.label(super::ui::text(state, spec.title));
                        ui.monospace(
                            binding
                                .keys
                                .iter()
                                .map(|k| k.display())
                                .collect::<Vec<_>>()
                                .join(", "),
                        );
                    });
                }
            },
        )
        .is_some()
        {
            open = false;
        }
    }
    ctx.data_mut(|d| d.insert_temp(id, open));
    open
}
