//! 场景共用的客户端与写入包夹具。

use crate::coordinator::{Coordinator, SwitchOutcome};
use crate::error::RejectReason;
use crate::model::{
    ActorId, Decision, Dialect, PERMIT_TTL_TICKS, PROTOCOL_VERSION, Permit, PermitId, ScopeId,
    WriteOp, WritePacket,
};

/// 模拟客户端：持有许可、序号与本地草稿队列。
#[derive(Clone, Debug)]
pub(crate) struct Client {
    /// 身份。
    pub actor: ActorId,
    /// 当前持有的许可（`None` 表示尚未取得或被收回）。
    pub permit: Option<Permit>,
    /// 下一个序号。
    pub seq: u64,
    /// 本地草稿队列（被拒或离线时保留）。
    pub drafts: Vec<WritePacket>,
}

impl Client {
    /// 新建客户端。
    pub(crate) fn new(name: &str) -> Self {
        Self {
            actor: ActorId::new(name),
            permit: None,
            seq: 0,
            drafts: Vec::new(),
        }
    }

    /// 加入协调范围并领取当前语言许可。
    pub(crate) fn join(coord: &mut Coordinator, name: &str) -> Self {
        let mut client = Self::new(name);
        coord.add_member(&client.actor);
        if let Ok(permit) = coord.grant_permit(&client.actor, PERMIT_TTL_TICKS) {
            client.permit = Some(permit);
        }
        client
    }

    /// 用持有的许可提交一条自动文本写入。
    ///
    /// 返回判定与本次使用的标记文本；被拒时同时压入本地草稿队列。
    pub(crate) fn submit(&mut self, coord: &mut Coordinator, path: &str) -> (Decision, String) {
        let seq = self.seq + 1;
        let text = marker(&self.actor, seq);
        let Some(permit) = self.permit.clone() else {
            let decision = Decision::Rejected(RejectReason::PermitUnknown { id: 0 });
            return (decision, text);
        };
        self.seq = seq;
        let packet = PacketBuilder::new(coord.scope(), &self.actor, permit.id)
            .dialect(permit.dialect)
            .epoch(coord.epoch())
            .seq(seq)
            .op(path, permit.dialect, &text)
            .build();
        let decision = coord.submit(&packet);
        if !decision.is_accepted() {
            self.drafts.push(packet);
        }
        (decision, text)
    }

    /// 重新领取当前语言许可（例如重连后经显式授权）。
    pub(crate) fn refresh_permit(&mut self, coord: &mut Coordinator) -> bool {
        match coord.grant_permit(&self.actor, PERMIT_TTL_TICKS) {
            Ok(permit) => {
                self.permit = Some(permit);
                true
            }
            Err(_) => false,
        }
    }
}

/// 写入包构造器，避免超长参数列表。
#[derive(Clone, Debug)]
pub(crate) struct PacketBuilder {
    protocol: u16,
    scope: ScopeId,
    actor: ActorId,
    declared: Dialect,
    epoch: u64,
    permit: PermitId,
    seq: u64,
    ops: Vec<WriteOp>,
}

impl PacketBuilder {
    /// 新建，默认协议版本与 LaTeX 声明。
    pub(crate) fn new(scope: &ScopeId, actor: &ActorId, permit: PermitId) -> Self {
        Self {
            protocol: PROTOCOL_VERSION,
            scope: scope.clone(),
            actor: actor.clone(),
            declared: Dialect::Latex,
            epoch: 0,
            permit,
            seq: 0,
            ops: Vec::new(),
        }
    }

    /// 设置协议版本。
    pub(crate) fn protocol(mut self, protocol: u16) -> Self {
        self.protocol = protocol;
        self
    }

    /// 设置声明语言。
    pub(crate) fn dialect(mut self, dialect: Dialect) -> Self {
        self.declared = dialect;
        self
    }

    /// 设置 epoch。
    pub(crate) fn epoch(mut self, epoch: u64) -> Self {
        self.epoch = epoch;
        self
    }

    /// 设置序号。
    pub(crate) fn seq(mut self, seq: u64) -> Self {
        self.seq = seq;
        self
    }

    /// 追加一条操作。
    pub(crate) fn op(mut self, path: &str, dialect: Dialect, text: &str) -> Self {
        self.ops.push(WriteOp {
            path: path.to_owned(),
            dialect,
            text: text.to_owned(),
        });
        self
    }

    /// 直接设置写集。
    pub(crate) fn ops(mut self, ops: Vec<WriteOp>) -> Self {
        self.ops = ops;
        self
    }

    /// 构造。
    pub(crate) fn build(self) -> WritePacket {
        WritePacket {
            protocol: self.protocol,
            scope: self.scope,
            actor: self.actor,
            declared: self.declared,
            epoch: self.epoch,
            permit: self.permit,
            seq: self.seq,
            ops: self.ops,
        }
    }
}

/// 构造一条操作。
pub(crate) fn op(path: &str, dialect: Dialect, text: &str) -> WriteOp {
    WriteOp {
        path: path.to_owned(),
        dialect,
        text: text.to_owned(),
    }
}

/// 用例标记文本：既能定位内容，又便于断言"恰好出现一次"。
pub(crate) fn marker(actor: &ActorId, seq: u64) -> String {
    format!("[[{}#{}]]", actor.0, seq)
}

/// 完成一次全量切换（请求 → 所有成员 ack → 提交）。
pub(crate) fn full_switch(
    coord: &mut Coordinator,
    actor: &ActorId,
    to: Dialect,
) -> Result<SwitchOutcome, RejectReason> {
    coord.request_switch(actor, to)?;
    for pending in coord.draining_expected() {
        coord.ack_drain(&pending);
    }
    coord.complete_switch(false)
}

/// 场景默认共享范围。
pub(crate) fn scope() -> ScopeId {
    ScopeId::new("proj-scholium/branch-main")
}
