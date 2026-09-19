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
            drafts: std::array::from_fn(|_| {
                let source = Session::new(core, Dialect::Latex);
                let text = source.generated.text.clone();
                (source, text)
            }),
        }
    }
}

impl SpikeApp {
    pub(super) fn draw_team(&mut self, ui: &mut egui::Ui) {
        let Some(mut team) = self.team.take() else {
            return;
        };
        let mut selected = team.selected;
        ui.horizontal_wrapped(|ui| {
            ui.label("团队门禁 spike · 三成员模拟 · 正文只读");
            for (index, name) in MEMBERS.iter().enumerate() {
                ui.selectable_value(&mut selected, index, *name);
            }
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
        team_controls(ui, &mut team);
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
