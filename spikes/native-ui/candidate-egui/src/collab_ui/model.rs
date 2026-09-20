//! Two source-authority replicas. Transport contains only coordinator-accepted updates.
use loro::{ExportMode, LoroDoc, UndoManager};
use scholium_spike_language_coordination::window::{Dialect, Team};

const MAX_TEXT: usize = 16 * 1024;
const MAX_QUEUE: usize = 128;
const MAX_UPDATE: usize = 1024 * 1024;

#[derive(Debug, thiserror::Error)]
pub(super) enum Error {
    #[error("{0}")]
    Rejected(&'static str),
    #[error(transparent)]
    Engine(#[from] loro::LoroError),
    #[error(transparent)]
    Encode(#[from] loro::LoroEncodeError),
    #[error(transparent)]
    Gate(#[from] scholium_spike_language_coordination::window::WindowError),
}
type Result<T> = std::result::Result<T, Error>;

pub(super) struct Client {
    doc: LoroDoc,
    undo: UndoManager,
    pub draft: String,
    base: String,
    pub epoch: u64,
    pub dialect: Dialect,
    pub composing: bool,
    pub frame_barrier: bool,
}
impl Client {
    pub fn text(&self) -> String {
        self.doc.get_text("source").to_string()
    }
    pub fn dirty(&self) -> bool {
        self.draft != self.base
    }
    pub fn blocked(&self) -> bool {
        self.composing || self.frame_barrier
    }
    fn refresh_projection(&mut self) {
        if !self.dirty() && !self.blocked() {
            self.base = self.text();
            self.draft = self.base.clone();
        }
    }
}

pub(super) struct Pair {
    pub clients: [Client; 2],
    pub gate: Team,
    queue: Vec<(usize, Vec<u8>)>,
    poisoned: bool,
}

impl Pair {
    pub fn new() -> Result<Self> {
        let seed = LoroDoc::new();
        seed.set_peer_id(99)?;
        seed.get_text("source").insert(0, "共同文本")?;
        seed.commit();
        let snapshot = seed.export(ExportMode::Snapshot)?;
        let make = |peer| -> Result<Client> {
            let doc = LoroDoc::new();
            doc.set_peer_id(peer)?;
            doc.import(&snapshot)?;
            let mut undo = UndoManager::new(&doc);
            undo.set_merge_interval(0);
            Ok(Client {
                doc,
                undo,
                draft: "共同文本".into(),
                base: "共同文本".into(),
                epoch: 1,
                dialect: Dialect::Latex,
                composing: false,
                frame_barrier: false,
            })
        };
        Ok(Self {
            clients: [make(1)?, make(2)?],
            gate: Team::default(),
            queue: vec![],
            poisoned: false,
        })
    }

    pub fn pending(&self) -> usize {
        self.queue.len()
    }

    fn authorize(&self, index: usize) -> Result<Team> {
        let client = &self.clients[index];
        if self.poisoned || self.queue.len() >= MAX_QUEUE {
            return Err(Error::Rejected("会话失败或队列已满"));
        }
        if client.blocked() {
            return Err(Error::Rejected("输入法组合未结束"));
        }
        let mut gate = self.gate.clone();
        gate.submit(index, client.dialect, client.epoch, &client.draft)?;
        Ok(gate)
    }

    pub fn apply(&mut self, index: usize) -> Result<()> {
        let client = &self.clients[index];
        validate_text(&client.draft)?;
        if client.base != client.text() {
            return Err(Error::Rejected("远端已改变基文，草稿保留"));
        }
        if !client.dirty() {
            return Err(Error::Rejected("草稿没有变化"));
        }
        let gate = self.authorize(index)?;
        let before = client.doc.oplog_vv();
        // Minimal scalar-index splice preserves the IDs of unchanged remote characters.
        let (at, removed, inserted) = splice(&client.base, &client.draft);
        let client = &mut self.clients[index];
        client.undo.record_new_checkpoint()?;
        self.poisoned = true;
        client.doc.get_text("source").delete(at, removed)?;
        if let Err(error) = client.doc.get_text("source").insert(at, &inserted) {
            self.poisoned = true;
            return Err(error.into());
        }
        client.doc.commit();
        self.publish(index, gate, &before)
    }

    pub fn undo(&mut self, index: usize) -> Result<()> {
        let client = &self.clients[index];
        if client.dirty() {
            return Err(Error::Rejected("先保留或丢弃未提交草稿"));
        }
        let gate = self.authorize(index)?;
        let before = client.doc.oplog_vv();
        self.poisoned = true;
        if !self.clients[index].undo.undo()? {
            self.poisoned = false;
            return Err(Error::Rejected("没有本地动作可撤销"));
        }
        self.clients[index].doc.commit();
        self.publish(index, gate, &before)
    }

    fn publish(&mut self, index: usize, gate: Team, before: &loro::VersionVector) -> Result<()> {
        self.poisoned = true;
        let client = &mut self.clients[index];
        let update = client.doc.export(ExportMode::updates(before))?;
        if update.len() > MAX_UPDATE {
            return Err(Error::Rejected("更新超过上限，会话停止"));
        }
        self.gate = gate;
        self.queue.push((1 - index, update));
        client.base = client.text();
        client.draft = client.base.clone();
        self.poisoned = false;
        Ok(())
    }

    pub fn deliver(&mut self, reverse: bool, duplicate: bool) -> Result<()> {
        if self.poisoned {
            return Err(Error::Rejected("会话失败"));
        }
        if self.clients.iter().any(Client::blocked) {
            return Err(Error::Rejected("组合输入期间暂缓交付"));
        }
        self.poisoned = true;
        if reverse {
            self.queue.reverse();
        }
        for (target, update) in &self.queue {
            self.clients[*target].doc.import(update)?;
            if duplicate {
                self.clients[*target].doc.import(update)?;
            }
        }
        self.queue.clear();
        for client in &mut self.clients {
            client.refresh_projection();
        }
        self.poisoned = false;
        Ok(())
    }

    pub fn regenerate(&mut self, index: usize) -> Result<()> {
        if self.clients[index].blocked() {
            return Err(Error::Rejected("输入法组合未结束"));
        }
        self.gate.refresh(index)?;
        let status = self.gate.status(index);
        let client = &mut self.clients[index];
        let changed_epoch = client.epoch != status.epoch;
        client.base = client.text();
        client.draft = client.base.clone();
        client.epoch = status.epoch;
        client.dialect = status.dialect;
        // Undo from the previous source language cannot be replayed under a new permit.
        if changed_epoch {
            client.undo = UndoManager::new(&client.doc);
            client.undo.set_merge_interval(0);
        }
        Ok(())
    }

    pub fn switch(&mut self) -> Result<()> {
        let target = match self.gate.status(0).dialect {
            Dialect::Latex => Dialect::Typst,
            Dialect::Typst => Dialect::Latex,
        };
        self.gate.request_switch(0, target)?;
        Ok(())
    }

    pub fn finish_switch(&mut self) -> Result<()> {
        if self.poisoned || !self.queue.is_empty() || self.clients.iter().any(Client::blocked) {
            return Err(Error::Rejected("等待消息交付及两端输入法结束；草稿保留"));
        }
        // Third control-plane fixture member has no data replica or editable inputs.
        for member in 0..3 {
            self.gate.acknowledge(member)?;
        }
        self.gate.complete_switch()?;
        Ok(())
    }
}

fn validate_text(text: &str) -> Result<()> {
    if text.len() > MAX_TEXT
        || !text
            .chars()
            .all(|c| c.is_alphanumeric() || c.is_whitespace() || "-_.".contains(c))
    {
        return Err(Error::Rejected(
            "实验仅接受 16 KiB 内纯文字片段，不接受宏与结构语法",
        ));
    }
    Ok(())
}

// Positions passed to Loro use Unicode scalar indices, not UTF-8 bytes or UTF-16 units.
fn splice(before: &str, after: &str) -> (usize, usize, String) {
    let left: Vec<_> = before.chars().collect();
    let right: Vec<_> = after.chars().collect();
    let prefix = left.iter().zip(&right).take_while(|(a, b)| a == b).count();
    let suffix = left[prefix..]
        .iter()
        .rev()
        .zip(right[prefix..].iter().rev())
        .take_while(|(a, b)| a == b)
        .count();
    (
        prefix,
        left.len() - prefix - suffix,
        right[prefix..right.len() - suffix].iter().collect(),
    )
}

#[cfg(test)]
mod tests;
