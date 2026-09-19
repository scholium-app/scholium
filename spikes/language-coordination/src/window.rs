//! Native-window port over the existing strict coordinator.
use crate::{
    coordinator::{Coordinator, GatePolicy},
    fixtures::Client,
    model::{Decision, PROTOCOL_VERSION, Phase, ScopeId, WriteOp, WritePacket},
};

pub use crate::model::Dialect;

/// Three simulated actors; one coordinator, no transport or authentication.
pub const MEMBERS: [&str; 3] = ["Alice", "Bob", "Carol"];

/// A rejected command. The draft remains owned by its client.
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct WindowError(String);

/// Coordinator projection for the native window.
#[derive(Clone, Debug)]
pub struct Status {
    /// Unique active language.
    pub dialect: Dialect,
    /// Coordinator generation.
    pub epoch: u64,
    /// Target while draining; new window submissions are frozen.
    pub target: Option<Dialect>,
    /// Client's cached generation, never automatically renewed.
    pub client_epoch: u64,
    /// Simulated connectivity.
    pub offline: bool,
    /// Explicit acknowledgements.
    pub acknowledged: [bool; 3],
    /// Accepted source packets.
    pub accepted: usize,
}

/// Shared control authority for the three simulated window clients.
#[derive(Clone, Debug)]
pub struct Team {
    coordinator: Coordinator,
    clients: [Client; 3],
    offline: [bool; 3],
    acknowledged: [bool; 3],
}

impl Default for Team {
    fn default() -> Self {
        let mut coordinator = Coordinator::new(
            ScopeId::new("window-spike/main"),
            Dialect::Latex,
            GatePolicy::Strict,
        );
        let clients = MEMBERS.map(|name| Client::join(&mut coordinator, name));
        Self {
            coordinator,
            clients,
            offline: [false; 3],
            acknowledged: [false; 3],
        }
    }
}

impl Team {
    /// Read a projection; invalid indices select the last client for display only.
    pub fn status(&self, member: usize) -> Status {
        let member = member.min(2);
        Status {
            dialect: self.coordinator.active(),
            epoch: self.coordinator.epoch(),
            target: match self.coordinator.phase() {
                Phase::Draining { to, .. } => Some(to),
                _ => None,
            },
            client_epoch: self.clients[member].permit.as_ref().map_or(0, |p| p.epoch),
            offline: self.offline[member],
            acknowledged: self.acknowledged,
            accepted: self.coordinator.applied_len(),
        }
    }

    /// Request a barrier without inventing remote acknowledgements.
    /// # Errors
    /// Invalid/offline member, unchanged language, or existing barrier.
    pub fn request_switch(&mut self, member: usize, dialect: Dialect) -> Result<(), WindowError> {
        self.connected(member)?;
        self.coordinator
            .request_switch(&self.clients[member].actor, dialect)
            .map_err(error)?;
        self.acknowledged = [false; 3];
        Ok(())
    }

    /// Confirm inputs are retained locally, with no source writes in flight.
    /// # Errors
    /// Invalid/offline member or no barrier.
    pub fn acknowledge(&mut self, member: usize) -> Result<(), WindowError> {
        self.connected(member)?;
        if !matches!(self.coordinator.phase(), Phase::Draining { .. }) {
            return Err(error("没有待确认切换"));
        }
        self.coordinator.ack_drain(&self.clients[member].actor);
        self.acknowledged[member] = true;
        Ok(())
    }

    /// Commit after all clients acknowledge; never force or quarantine.
    /// # Errors
    /// No barrier or missing acknowledgements.
    pub fn complete_switch(&mut self) -> Result<(), WindowError> {
        self.coordinator.complete_switch(false).map_err(error)?;
        Ok(())
    }

    /// Explicitly acquire a permit for the current generation.
    /// # Errors
    /// Invalid/offline member or barrier in progress.
    pub fn refresh(&mut self, member: usize) -> Result<(), WindowError> {
        self.connected(member)?;
        let client = &mut self.clients[member];
        client.permit = Some(
            self.coordinator
                .grant_permit(&client.actor, crate::model::PERMIT_TTL_TICKS)
                .map_err(error)?,
        );
        Ok(())
    }

    /// Toggle simulated connectivity, retaining the old permit and drafts.
    /// # Errors
    /// Unknown member.
    pub fn set_offline(&mut self, member: usize, offline: bool) -> Result<(), WindowError> {
        if member >= self.clients.len() {
            return Err(error("未知成员"));
        }
        self.offline[member] = offline;
        self.coordinator.set_partitioned(
            self.clients
                .iter()
                .enumerate()
                .filter(|(i, _)| self.offline[*i])
                .map(|(_, c)| c.actor.clone())
                .collect(),
        );
        Ok(())
    }

    /// Validate epoch, permit, language and write set at the coordinator.
    /// Call on a private candidate and publish only after semantic validation also succeeds.
    /// # Errors
    /// Invalid member, draining, offline, stale/expired permit, or invalid write set.
    pub fn submit(
        &mut self,
        member: usize,
        dialect: Dialect,
        draft_epoch: u64,
        text: &str,
    ) -> Result<(), WindowError> {
        if member >= self.clients.len() {
            return Err(error("未知成员"));
        }
        if matches!(self.coordinator.phase(), Phase::Draining { .. }) {
            return Err(error("切换等待确认：新提交已冻结，草稿保留"));
        }
        let client = &mut self.clients[member];
        let permit = client
            .permit
            .as_ref()
            .ok_or_else(|| error("缺少源码许可"))?;
        client.seq += 1;
        let packet = WritePacket {
            protocol: PROTOCOL_VERSION,
            scope: self.coordinator.scope().clone(),
            actor: client.actor.clone(),
            declared: dialect,
            epoch: draft_epoch,
            permit: permit.id,
            seq: client.seq,
            ops: vec![WriteOp {
                path: format!("source.{}", dialect.extension()),
                dialect,
                text: text.into(),
            }],
        };
        match self.coordinator.submit(&packet) {
            Decision::Accepted(_) => Ok(()),
            Decision::Rejected(reason) => Err(error(reason)),
        }
    }

    fn connected(&self, member: usize) -> Result<(), WindowError> {
        if member >= self.clients.len() {
            return Err(error("未知成员"));
        }
        if self.offline[member] {
            return Err(error("成员离线，草稿保留"));
        }
        Ok(())
    }
}

fn error(value: impl ToString) -> WindowError {
    WindowError(value.to_string())
}
