//! 文本 CRDT：Logoot 类序列 + LWW 存活位。
//!
//! 状态是三样东西的并集，全部与投递顺序无关：
//!
//! - **字符条目** `(位置标识, 字符标识, 字符)`：插入是集合插入，按全序排列，幂等；
//!   并发插入不需要任何 "跳过同源后继" 的规则，因为顺序完全由位置标识决定。
//! - **存活位**：每个字符一个 LWW 寄存器，删除与撤销删除都是写寄存器，不是移除条目。
//! - **位置索引**：`字符标识 → 位置标识`，用于按标识更新存活位和去重。
//!
//! 删除保留墓碑是有意为之：墓碑让 "撤销本地删除" 与 "远端在删除边界输入" 都能靠稳定
//! 标识重定位。代价是序列长度只增不减，规模证据里单独报告墓碑数量。

use std::collections::HashMap;

use crate::codec::{Reader, Writer};
use crate::error::CrdtError;
use crate::ids::{CharId, Lamport};
use crate::position::{Position, between};
use crate::rng::Rng;
use crate::seq::{ChunkedSeq, Entry};

/// 一个字符的存活寄存器。
#[derive(Clone, Copy, Debug)]
pub(crate) struct Liveness {
    /// 当前是否可见。
    pub(crate) alive: bool,
    /// 最后一次写入该寄存器的时间戳。
    pub(crate) ts: Lamport,
}

/// 单个节点的文本序列。
#[derive(Clone, Debug, Default)]
pub(crate) struct TextCrdt {
    seq: ChunkedSeq,
    positions: HashMap<CharId, Position>,
    liveness: HashMap<CharId, Liveness>,
}

impl TextCrdt {
    /// 可见字符数。
    pub(crate) fn visible_len(&self) -> usize {
        self.seq.visible_len()
    }

    /// 条目总数（含墓碑）。
    pub(crate) fn entry_count(&self) -> usize {
        self.seq.len()
    }

    /// 分块数量。
    pub(crate) fn chunk_count(&self) -> usize {
        self.seq.chunk_count()
    }

    /// 遍历全部条目的位置标识（含墓碑），用于统计最长位置标识。
    pub(crate) fn iter_positions(&self) -> impl Iterator<Item = &[u32]> {
        self.seq.iter().map(|entry| entry.pos.as_slice())
    }

    /// 渲染可见字符。
    pub(crate) fn render(&self) -> String {
        self.seq.iter().filter(|e| e.alive).map(|e| e.ch).collect()
    }

    /// 字符当前是否可见。
    pub(crate) fn is_alive(&self, id: CharId) -> bool {
        self.liveness.get(&id).is_none_or(|l| l.alive)
    }

    /// 字符存活寄存器的时间戳；从未写过则为 `None`。
    pub(crate) fn liveness_ts(&self, id: CharId) -> Option<Lamport> {
        self.liveness.get(&id).map(|l| l.ts)
    }

    /// 为本地插入挑选位置标识，**不修改状态**。
    ///
    /// `offset` 是**可见**偏移（UTF-8 字符边界即字符边界，本 spike 的文本以 `char` 为单位，
    /// 不是 byte offset）。选位失败时返回错误，调用方因此不会写出半个操作。
    ///
    /// # Errors
    ///
    /// `offset > visible_len()` 时返回 [`CrdtError::InvalidOffset`]。
    pub(crate) fn choose_position(
        &self,
        offset: usize,
        rng: &mut Rng,
    ) -> Result<Position, CrdtError> {
        let len = self.visible_len();
        let index = self
            .seq
            .physical_index_of_visible(offset)
            .ok_or(CrdtError::InvalidOffset { offset, len })?;
        let left = if index == 0 {
            None
        } else {
            self.seq.get(index - 1).map(|e| e.pos.as_slice())
        };
        let right = self.seq.get(index).map(|e| e.pos.as_slice());
        match between(left, right, rng) {
            Ok(pos) => Ok(pos),
            // 上下界位置标识相同：并发插入已经占用了这一层位置。复用该位置，
            // 组内顺序由字符标识 (actor, seq) 决定，仍然收敛。
            Err(CrdtError::UnorderedBounds) => Ok(left
                .or(right)
                .map(<[u32]>::to_vec)
                .unwrap_or_else(|| vec![1])),
            Err(other) => Err(other),
        }
    }

    /// 收集可见区间 `[start, end)` 内的字符标识（本地删除用）。
    ///
    /// # Errors
    ///
    /// `start > end` 或 `end > visible_len()` 时返回 [`CrdtError::InvalidOffset`]。
    pub(crate) fn ids_in_visible_range(
        &self,
        start: usize,
        end: usize,
    ) -> Result<Vec<CharId>, CrdtError> {
        let len = self.visible_len();
        if start > end || end > len {
            return Err(CrdtError::InvalidOffset { offset: end, len });
        }
        let mut index = self
            .seq
            .physical_index_of_visible(start)
            .ok_or(CrdtError::InvalidOffset { offset: start, len })?;
        let mut out = Vec::with_capacity(end - start);
        while out.len() < end - start {
            let entry = self
                .seq
                .get(index)
                .ok_or(CrdtError::InvalidOffset { offset: end, len })?;
            if entry.alive {
                out.push(entry.id);
            }
            index += 1;
        }
        Ok(out)
    }

    /// 应用一个插入操作。重复操作返回 `false`（幂等）。
    ///
    /// 存活位来自存活寄存器；若删除操作先于插入到达，这里会直接落成墓碑。
    pub(crate) fn apply_insert(&mut self, id: CharId, pos: Position, ch: char) -> bool {
        if self.positions.contains_key(&id) {
            return false;
        }
        let alive = self.is_alive(id);
        let index = self.seq.lower_bound(&pos, id);
        self.seq.insert_at(
            index,
            Entry {
                pos: pos.clone(),
                id,
                ch,
                alive,
            },
        );
        self.positions.insert(id, pos);
        true
    }

    /// 应用一个存活位写入。旧时间戳的写入被丢弃（LWW）。
    pub(crate) fn apply_alive(&mut self, id: CharId, alive: bool, ts: Lamport) -> bool {
        let register = self.liveness.entry(id).or_insert(Liveness {
            alive: true,
            ts: Lamport::zero(),
        });
        if ts <= register.ts {
            return false;
        }
        // 必须写回时间戳：否则后续写入永远"更新"，乱序到达的旧操作会把新状态覆盖掉。
        register.alive = alive;
        register.ts = ts;
        if let Some(pos) = self.positions.get(&id)
            && let Some(index) = self.seq.index_of(pos, id)
        {
            self.seq.set_alive(index, alive);
        }
        true
    }

    /// 写出用于比较哈希的规范状态：按全序排列的条目（含存活位）。
    pub(crate) fn write_canonical(&self, w: &mut Writer) {
        w.u32(self.seq.len() as u32);
        for entry in self.seq.iter() {
            w.id(entry.id);
            w.position(&entry.pos);
            w.char(entry.ch);
            w.bool(entry.alive);
        }
    }

    /// 写出完整快照（含存活寄存器与位置索引可重建的信息）。
    pub(crate) fn write_snapshot(&self, w: &mut Writer) {
        w.u32(self.seq.len() as u32);
        for entry in self.seq.iter() {
            w.id(entry.id);
            w.position(&entry.pos);
            w.char(entry.ch);
            w.bool(entry.alive);
        }
        let mut registers: Vec<(&CharId, &Liveness)> = self.liveness.iter().collect();
        registers.sort_by_key(|(id, _)| **id);
        w.u32(registers.len() as u32);
        for (id, register) in registers {
            w.id(*id);
            w.bool(register.alive);
            w.lamport(register.ts);
        }
    }

    /// 从快照读回。
    ///
    /// # Errors
    ///
    /// 输入截断、判别式非法或位置标识不满足不变量时返回错误。
    pub(crate) fn read_snapshot(r: &mut Reader<'_>) -> Result<Self, CrdtError> {
        let count = r.u32()? as usize;
        let mut entries = Vec::with_capacity(count);
        for _ in 0..count {
            let id = r.id()?;
            let pos = r.position()?;
            let ch = r.char()?;
            let alive = r.bool()?;
            entries.push(Entry { pos, id, ch, alive });
        }
        let registers = r.u32()? as usize;
        let mut liveness = HashMap::with_capacity(registers);
        for _ in 0..registers {
            let id = r.id()?;
            let alive = r.bool()?;
            let ts = r.lamport()?;
            liveness.insert(id, Liveness { alive, ts });
        }
        let positions = entries.iter().map(|e| (e.id, e.pos.clone())).collect();
        Ok(Self {
            seq: ChunkedSeq::from_entries(entries),
            positions,
            liveness,
        })
    }
}
