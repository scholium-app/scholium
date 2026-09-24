//! Scholium 原生工作区外壳；示例内容只用于布局，不连接编辑核心。

mod chrome;
mod commands;
mod icons;
mod native_text;
mod page_editor;
mod paper;
mod preview;
mod sample;
mod session;
mod state;
mod theme;
mod workspace;

use eframe::egui;
use state::WorkspaceState;

#[derive(Default)]
struct ScholiumApp {
    state: WorkspaceState,
    session: session::SessionBridge,
}

impl eframe::App for ScholiumApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        commands::shortcuts(ui.ctx(), &mut self.state);
        chrome::show(ui, &mut self.state);
        workspace::show(ui, &mut self.state);
        self.session.update(ui.ctx(), &mut self.state);
    }
}

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("谱与振动 — Scholium · 界面预览")
            .with_inner_size([1280.0, 900.0])
            .with_min_inner_size([640.0, 480.0]),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    eframe::run_native(
        "Scholium",
        options,
        Box::new(|cc| {
            theme::install(&cc.egui_ctx);
            Ok(Box::<ScholiumApp>::default())
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::{chrome, theme, workspace};
    use crate::state::WorkspaceState;
    use eframe::egui::{self, Rect};

    fn draw_at(size: egui::Vec2, mode: crate::state::ViewMode) -> egui::Rect {
        let ctx = egui::Context::default();
        theme::install(&ctx);
        let mut state = WorkspaceState {
            mode,
            ..Default::default()
        };
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, size)),
                ..Default::default()
            },
            |ui| {
                chrome::show(ui, &mut state);
                workspace::show(ui, &mut state);
            },
        );
        let bounds = output
            .shapes
            .iter()
            .map(|shape| {
                shape
                    .shape
                    .visual_bounding_rect()
                    .intersect(shape.clip_rect)
            })
            .reduce(|left, right| left.union(right))
            // The menu and panel backgrounds always produce visible shapes.
            .expect("base UI produces visible shapes");
        output.textures_delta.clear();
        bounds
    }

    #[test]
    fn visual_layout_stays_inside_tiled_window() {
        for size in [
            egui::vec2(640.0, 480.0),
            egui::vec2(844.0, 1011.0),
            egui::vec2(1280.0, 900.0),
        ] {
            let bounds = draw_at(size, crate::state::ViewMode::Visual);
            assert!(
                Rect::from_min_size(egui::Pos2::ZERO, size).contains_rect(bounds),
                "{bounds:?}"
            );
        }
    }

    #[test]
    fn source_layout_stays_inside_tiled_window() {
        for size in [
            egui::vec2(640.0, 480.0),
            egui::vec2(844.0, 1011.0),
            egui::vec2(1280.0, 900.0),
        ] {
            let bounds = draw_at(size, crate::state::ViewMode::Source);
            assert!(
                Rect::from_min_size(egui::Pos2::ZERO, size).contains_rect(bounds),
                "{bounds:?}"
            );
        }
    }

    #[test]
    fn keyboard_events_switch_views_and_resize_splitter() {
        let ctx = egui::Context::default();
        theme::install(&ctx);
        let mut state = WorkspaceState::default();
        let mut frame = |events| {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1280.0, 900.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| {
                    super::commands::shortcuts(ui.ctx(), &mut state);
                    chrome::show(ui, &mut state);
                    workspace::show(ui, &mut state);
                },
            );
            output.textures_delta.clear();
            (state.mode, state.split, state.diagnostics)
        };
        frame(vec![]);
        let result = frame(vec![key(egui::Key::Num2, egui::Modifiers::COMMAND)]);
        assert_eq!(result.0, crate::state::ViewMode::Source);
        frame(vec![]);
        let pos = egui::pos2(640.0, 300.0);
        frame(vec![
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ]);
        frame(vec![egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }]);
        frame(vec![]);
        let result = frame(vec![key(egui::Key::ArrowRight, egui::Modifiers::NONE)]);
        assert!(result.1 > 0.5);
        let result = frame(vec![key(egui::Key::Home, egui::Modifiers::NONE)]);
        assert_eq!(result.1, 0.5);
        let result = frame(vec![key(egui::Key::J, egui::Modifiers::COMMAND)]);
        assert!(result.2);
        // The tab-bar icon switch stays clickable: in the third bar row the visual
        // toggle is the second icon from the right (menus 28 + tools 34 + half of 32).
        frame(vec![
            egui::Event::PointerMoved(egui::pos2(1226.0, 78.0)),
            egui::Event::PointerButton {
                pos: egui::pos2(1226.0, 78.0),
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ]);
        let result = frame(vec![egui::Event::PointerButton {
            pos: egui::pos2(1226.0, 78.0),
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }]);
        assert_eq!(result.0, crate::state::ViewMode::Visual);
    }

    fn key(key: egui::Key, modifiers: egui::Modifiers) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }
    }
}
