//! Explicit source-authority collaboration experiment, independent of the SDG editor.
mod model;
use eframe::egui;
use model::Pair;

pub(super) struct CollabUi {
    pair: Result<Pair, model::Error>,
    message: String,
}

impl CollabUi {
    pub fn from_env() -> Option<Self> {
        std::env::var_os("SCHOLIUM_SPIKE_REPLICAS")?;
        Some(Self {
            pair: Pair::new(),
            message: String::new(),
        })
    }

    pub fn draw(&mut self, ui: &mut egui::Ui) {
        ui.heading("源码权威 · Loro 双副本实验");
        ui.label("仅纯文字片段；两个独立副本，手动交付消息；无网络、无持久化");
        let pair = match &mut self.pair {
            Ok(pair) => pair,
            Err(error) => {
                ui.label(error.to_string());
                return;
            }
        };
        observe_ime(pair, ui.ctx());
        let status = pair.gate.status(0);
        ui.label(format!(
            "活动语言 {:?} · epoch {} · 待交付 {} · 已接受 {}",
            status.dialect,
            status.epoch,
            pair.pending(),
            status.accepted
        ));
        let mut result = None;
        ui.horizontal_wrapped(|ui| {
            if ui.button("交付消息").clicked() {
                result = Some(pair.deliver(false, false));
            }
            if ui.button("逆序并重复交付").clicked() {
                result = Some(pair.deliver(true, true));
            }
            if ui.button("请求语言切换").clicked() {
                result = Some(pair.switch());
            }
            if ui.button("完成语言切换").clicked() {
                result = Some(pair.finish_switch());
            }
        });
        if status.target.is_some() {
            ui.label("切换中：新写入冻结，等待已接受消息及组合输入结束");
        }
        ui.columns(2, |columns| {
            for (index, name) in ["Alice", "Bob"].iter().enumerate() {
                if let Some(action) = draw_client(pair, &mut columns[index], index, name) {
                    result = Some(action);
                }
            }
        });
        if let Some(result) = result {
            self.message = result.map_or_else(|error| format!("拒绝：{error}"), |()| "完成".into());
            println!(
                "[replicas] {} | A={:?} B={:?} pending={}",
                self.message,
                pair.clients[0].text(),
                pair.clients[1].text(),
                pair.pending()
            );
        }
        ui.label(&self.message);
    }
}

fn draw_client(
    pair: &mut Pair,
    ui: &mut egui::Ui,
    index: usize,
    name: &str,
) -> Option<Result<(), model::Error>> {
    ui.heading(name);
    let client = &mut pair.clients[index];
    ui.label(format!("{name} 已接受正文：{}", client.text()));
    ui.label(format!(
        "{name} 草稿 {:?} / epoch {}",
        client.dialect, client.epoch
    ));
    ui.add(
        egui::TextEdit::multiline(&mut client.draft)
            .id(egui::Id::new(("replica_source", index)))
            .hint_text(format!("{name} 草稿")),
    );
    if client.blocked() {
        ui.label(format!("{name} 输入法组合中"));
    }
    let mut result = None;
    ui.horizontal_wrapped(|ui| {
        if ui.button(format!("应用 {name}")).clicked() {
            result = Some(pair.apply(index));
        }
        if ui.button(format!("撤销 {name}")).clicked() {
            result = Some(pair.undo(index));
        }
        if ui.button(format!("重新生成 {name}")).clicked() {
            result = Some(pair.regenerate(index));
        }
    });
    result
}

fn observe_ime(pair: &mut Pair, ctx: &egui::Context) {
    for (index, client) in pair.clients.iter_mut().enumerate() {
        client.frame_barrier = client.composing;
        let focused =
            ctx.memory(|memory| memory.has_focus(egui::Id::new(("replica_source", index))));
        if !focused {
            continue;
        }
        ctx.input(|input| {
            for event in &input.events {
                match event {
                    egui::Event::Ime(egui::ImeEvent::Preedit { text, .. }) => {
                        client.composing = !text.is_empty();
                        client.frame_barrier = true;
                    }
                    egui::Event::Ime(egui::ImeEvent::Commit(_)) => {
                        client.composing = false;
                        client.frame_barrier = true;
                    }
                    _ => {}
                }
            }
        });
        if client.frame_barrier {
            ctx.request_repaint();
        }
    }
}
