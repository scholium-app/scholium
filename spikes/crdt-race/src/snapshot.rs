//! 快照与恢复：状态层序列化。
//!
//! 快照包含身份、逻辑时钟、序号、随机状态、操作日志、撤销栈与文档状态。恢复出的副本与快照
//! 字节一一对应：`restore(snapshot(r)).snapshot() == snapshot(r)`。
//!
//! 携带日志是刻意的：恢复出来的设备要能继续与别人交换缺失操作，只恢复文档状态会让它无法回答
//! "你有哪些操作"。代价是快照体积随历史增长，正式实现需要 `docs/HISTORY_COLLABORATION.md` §7
//! 的压缩与 checkpoint。

use crate::codec::{Reader, Writer};
use crate::doc::Doc;
use crate::error::CrdtError;
use crate::ids::ActorId;
use crate::op::Op;
use crate::replica::Replica;
use crate::rng::Rng;
use crate::undo::Action;

/// 快照魔数（"SCRD" 的小端 u32）。
const SNAPSHOT_MAGIC: u32 = 0x4452_4353;
/// 快照格式版本。布局变化必须递增，旧版本要么拒绝要么迁移。
const SNAPSHOT_VERSION: u16 = 1;

impl Replica {
    /// 完整快照：身份、时钟、序号、随机状态、操作日志、撤销栈、文档状态。
    pub(crate) fn snapshot(&self) -> Vec<u8> {
        let mut w = Writer::new();
        w.u32(SNAPSHOT_MAGIC);
        w.u16(SNAPSHOT_VERSION);
        w.u32(self.actor.value());
        w.u64(self.clock);
        w.u32(self.next_seq);
        w.u64(self.rng.state());
        w.u32(self.log.len() as u32);
        for op in &self.log {
            op.write(&mut w);
        }
        w.u32(self.undo.len() as u32);
        for action in &self.undo {
            action.write(&mut w);
        }
        self.doc.write_snapshot(&mut w);
        w.finish()
    }

    /// 从快照恢复。
    ///
    /// # Errors
    ///
    /// 魔数或版本不匹配、输入截断、字段非法、尾部有多余字节时返回错误。
    pub(crate) fn restore(bytes: &[u8]) -> Result<Self, CrdtError> {
        let mut r = Reader::new(bytes);
        if r.u32()? != SNAPSHOT_MAGIC {
            return Err(CrdtError::SnapshotMagic);
        }
        let version = r.u16()?;
        if version != SNAPSHOT_VERSION {
            return Err(CrdtError::VersionMismatch {
                expected: SNAPSHOT_VERSION,
                found: version,
            });
        }
        let actor = ActorId(r.u32()?);
        let clock = r.u64()?;
        let next_seq = r.u32()?;
        let rng = Rng::from_state(r.u64()?);
        let log_count = r.u32()? as usize;
        let mut log = Vec::with_capacity(log_count);
        for _ in 0..log_count {
            log.push(Op::read(&mut r)?);
        }
        let undo_count = r.u32()? as usize;
        let mut undo = Vec::with_capacity(undo_count);
        for _ in 0..undo_count {
            undo.push(Action::read(&mut r)?);
        }
        let doc = Doc::read_snapshot(&mut r)?;
        if !r.is_empty() {
            let mut extra = 0usize;
            while !r.is_empty() {
                let _ = r.u8()?;
                extra += 1;
            }
            return Err(CrdtError::TrailingBytes(extra));
        }
        let seen = log.iter().map(Op::key).collect();
        Ok(Self {
            actor,
            clock,
            next_seq,
            rng,
            doc,
            log,
            seen,
            undo,
            position_fallbacks: 0,
        })
    }
}
