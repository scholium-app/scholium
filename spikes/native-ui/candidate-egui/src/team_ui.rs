//! Opt-in three-client control-plane simulation in one native window.
use super::*;
use scholium_spike_language_coordination::window::{Dialect as TeamDialect, MEMBERS, Team};
use scholium_spike_reconcile::{Session, generate::Dialect};

pub(super) struct TeamUi {
    pub coordinator: Team,
    pub selected: usize,
    drafts: [(Session, String); 3],
    epochs: [u64; 3],
    message: String,
    composing: bool,
    composition_frame: bool,
}

impl TeamUi {
    pub fn from_env(core: &Editor) -> Option<Self> {
        std::env::var_os("SCHOLIUM_SPIKE_TEAM")?;
        Some(Self::new(core))
    }

    fn new(core: &Editor) -> Self {
        Self {
            coordinator: Team::default(),
            selected: 0,
            epochs: [1; 3],
            message: String::new(),
            composing: false,
            composition_frame: false,
            drafts: std::array::from_fn(|_| {
                let source = Session::new(core, Dialect::Latex);
                let text = source.generated.text.clone();
                (source, text)
            }),
        }
    }
}

impl SpikeApp {
    pub(super) fn source_composition_blocked(&self) -> bool {
        self.team
            .as_ref()
            .is_some_and(|team| team.composition_frame)
    }

    pub(super) fn observe_team_ime(&mut self, ctx: &egui::Context) {
        let Some(team) = &mut self.team else {
            return;
        };
        let focused =
            ctx.memory(|memory| memory.has_focus(egui::Id::new(("team_source", team.selected))));
        team.composition_frame = team.composing;
        if !focused && !team.composing {
            return;
        }
        ctx.input(|input| {
            for event in &input.events {
                match event {
                    egui::Event::Ime(egui::ImeEvent::Preedit { text, .. }) => {
                        team.composing = !text.is_empty();
                        team.composition_frame = true;
                    }
                    egui::Event::Ime(egui::ImeEvent::Commit(_)) => {
                        team.composing = false;
                        team.composition_frame = true;
                    }
                    _ => {}
                }
            }
        });
        // The finishing event must reach the old TextEdit before any actor/epoch changes.
        if team.composition_frame {
            ctx.request_repaint();
        }
    }

    pub(super) fn draw_team(&mut self, ui: &mut egui::Ui) {
        let Some(mut team) = self.team.take() else {
            return;
        };
        let mut selected = team.selected;
        ui.horizontal_wrapped(|ui| {
            ui.label("团队门禁 spike · 三成员模拟 · 正文只读");
            ui.add_enabled_ui(!team.composition_frame, |ui| {
                for (index, name) in MEMBERS.iter().enumerate() {
                    ui.selectable_value(&mut selected, index, *name);
                }
            });
        });
        if selected != team.selected {
            team.drafts[team.selected] = (self.source.clone(), self.source_buffer.clone());
            (self.source, self.source_buffer) = team.drafts[selected].clone();
            team.selected = selected;
        }
        let status = team.coordinator.status(selected);
        ui.label(format!(
            "活动语言 {:?} · epoch {} · {} 许可 epoch {} · 已接受 {}",
            status.dialect, status.epoch, MEMBERS[selected], status.client_epoch, status.accepted
        ));
        ui.add_enabled_ui(!team.composition_frame, |ui| team_controls(ui, &mut team));
        if team.composition_frame {
            ui.label("输入法组合中：请先选词或取消，再提交或切换成员/语言");
        }
        ui.label(format!(
            "本地草稿 epoch {} · {}",
            team.epochs[selected], team.message
        ));
        self.team = Some(team);
    }

    pub(super) fn team_regenerated(&mut self) {
        if let Some(team) = &mut self.team {
            team.epochs[team.selected] = team.coordinator.status(team.selected).epoch;
        }
    }

    pub(super) fn commit_team_source(&mut self) {
        if self.source_composition_blocked() {
            self.last_event = "输入法组合尚未结束，未提交源码".into();
            return;
        }
        let Some(team) = &mut self.team else {
            return;
        };
        // Both candidates are private until every gate succeeds. No accepted receipt for a failed reconcile.
        let mut core = self.core.clone();
        let mut source = self.source.clone();
        let mut coordinator = team.coordinator.clone();
        let accepted = coordinator.submit(
            team.selected,
            dialect(self.source.dialect),
            team.epochs[team.selected],
            &self.source_buffer,
        );
        let result = accepted.map_err(|e| e.to_string()).and_then(|()| {
            source
                .commit(&mut core, &self.source_buffer)
                .map_err(|e| e.to_string())
        });
        self.last_event = match result {
            Ok(()) => {
                self.core = core;
                self.source = source;
                team.coordinator = coordinator;
                "团队源码已应用到正文".into()
            }
            Err(error) => format!("提交被拒，草稿保留：{error}"),
        };
        team.message = self.last_event.clone();
    }
}

fn team_controls(ui: &mut egui::Ui, team: &mut TeamUi) {
    let selected = team.selected;
    let status = team.coordinator.status(selected);
    let mut result = None;
    ui.horizontal_wrapped(|ui| {
        let mut offline = status.offline;
        if ui.checkbox(&mut offline, "模拟离线").changed() {
            result = Some(team.coordinator.set_offline(selected, offline));
        }
        if ui.button("获取当前许可").clicked() {
            result = Some(team.coordinator.refresh(selected));
        }
        for (label, target) in [
            ("请求切换到 LaTeX", TeamDialect::Latex),
            ("请求切换到 Typst", TeamDialect::Typst),
        ] {
            if ui.button(label).clicked() {
                result = Some(team.coordinator.request_switch(selected, target));
            }
        }
    });
    if let Some(target) = status.target {
        ui.label(format!(
            "正在切换到 {target:?} · 确认状态 {:?}",
            status.acknowledged
        ));
        ui.horizontal_wrapped(|ui| {
            if ui.button("保留草稿并确认切换").clicked() {
                result = Some(team.coordinator.acknowledge(selected));
            }
            if ui.button("完成团队切换").clicked() {
                result = Some(team.coordinator.complete_switch());
            }
        });
    }
    if let Some(result) = result {
        team.message =
            result.map_or_else(|e| e.to_string(), |()| "协调命令完成，草稿未改变".into());
    }
}

fn dialect(value: Dialect) -> TeamDialect {
    match value {
        Dialect::Latex => TeamDialect::Latex,
        Dialect::Typst => TeamDialect::Typst,
    }
}

#[cfg(test)]
mod tests;
