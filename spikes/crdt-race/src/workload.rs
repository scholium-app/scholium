//! 随机工作负载：规模判据与随机化收敛判据共用同一台动作发生器。
//!
//! 每个动作都是**用户层动作**（一次插入 / 一次删除 / 一次属性变更 / 一次包裹或解除），
//! 与 `docs/HISTORY_COLLABORATION.md` 的 Action 语义对齐：规模数字要能换算成用户操作，
//! 而不是 CRDT 内部操作数。
//!
//! 全部随机性来自 [`Rng`]，种子固定即可复现。

use crate::error::CrdtError;
use crate::fixture::Fixture;
use crate::ids::NodeId;
use crate::model::NodeKind;
use crate::replica::Replica;
use crate::rng::Rng;

/// 文本插入动作在总动作中的百分比。
const INSERT_PERCENT: u64 = 55;
/// 文本删除动作在总动作中的百分比。
const DELETE_PERCENT: u64 = 15;
/// 属性变更动作在总动作中的百分比。
const ATTR_PERCENT: u64 = 8;
/// 本地撤销动作在总动作中的百分比（其余为包裹 / 解除包裹）。
const UNDO_PERCENT: u64 = 5;
/// 单次删除的最大字符数。
const MAX_DELETE_LEN: usize = 8;
/// 包裹相对解除包裹的偏好百分比。
const WRAP_PERCENT: u64 = 55;
/// 属性值的循环集合（含移除）。
const ATTR_VALUES: [Option<&str>; 3] = [Some("left"), Some("center"), None];
/// 插入字符的字母表大小。
const ALPHABET: u64 = 26;

/// 一台有状态的随机动作发生器。
///
/// `wrappers` 跟踪当前仍然存活的包裹节点，保证解除包裹总是作用在最新的包裹上（LIFO），
/// 不会去解除一个已经被删掉的节点。随机 undo 可能删掉栈里的包裹节点，因此解除前要复查存活。
#[derive(Debug)]
pub(crate) struct Workload {
    rng: Rng,
    wrappers: Vec<NodeId>,
    attr_cursor: usize,
    insertions: usize,
    deletions: usize,
    attrs: usize,
    wraps: usize,
    unwraps: usize,
    undos: usize,
}

impl Workload {
    /// 以种子构造。
    pub(crate) fn new(seed: u64) -> Self {
        Self {
            rng: Rng::new(seed),
            wrappers: Vec::new(),
            attr_cursor: 0,
            insertions: 0,
            deletions: 0,
            attrs: 0,
            wraps: 0,
            unwraps: 0,
            undos: 0,
        }
    }

    /// 各动作计数：`(插入, 删除, 属性, 包裹, 解除包裹, 撤销)`。
    pub(crate) fn counts(&self) -> (usize, usize, usize, usize, usize, usize) {
        (
            self.insertions,
            self.deletions,
            self.attrs,
            self.wraps,
            self.unwraps,
            self.undos,
        )
    }

    /// 对副本执行一个随机动作，返回动作类别名。
    ///
    /// 动作内部可能生成多个操作（一次删除多个字符、一次包裹两个操作），
    /// 但计数按用户动作算。
    ///
    /// # Errors
    ///
    /// 底层 CRDT 操作失败时返回错误。
    pub(crate) fn apply_one(
        &mut self,
        replica: &mut Replica,
        fixture: &Fixture,
    ) -> Result<&'static str, CrdtError> {
        let roll = self.rng.below(100);
        if roll < INSERT_PERCENT {
            self.insert(replica, fixture)
        } else if roll < INSERT_PERCENT + DELETE_PERCENT {
            self.delete(replica, fixture)
        } else if roll < INSERT_PERCENT + DELETE_PERCENT + ATTR_PERCENT {
            self.attribute(replica, fixture)
        } else if roll < INSERT_PERCENT + DELETE_PERCENT + ATTR_PERCENT + UNDO_PERCENT {
            self.undo(replica, fixture)
        } else {
            self.structure(replica, fixture)
        }
    }

    fn insert(&mut self, replica: &mut Replica, fixture: &Fixture) -> Result<&'static str, CrdtError> {
        let len = replica
            .doc()
            .text(fixture.body)
            .map_or(0, |text| text.visible_len());
        let offset = self.rng.below(len as u64 + 1) as usize;
        let ch = (b'a' + self.rng.below(ALPHABET) as u8) as char;
        replica.insert_char(fixture.body, offset, ch)?;
        self.insertions += 1;
        Ok("insert")
    }

    fn delete(&mut self, replica: &mut Replica, fixture: &Fixture) -> Result<&'static str, CrdtError> {
        let len = replica
            .doc()
            .text(fixture.body)
            .map_or(0, |text| text.visible_len());
        if len == 0 {
            // 没有可删内容时退化为插入，保持动作总数与判据一致。
            return self.insert(replica, fixture);
        }
        let start = self.rng.below(len as u64) as usize;
        let span = len - start;
        let take = (self.rng.below(MAX_DELETE_LEN as u64) as usize + 1).min(span);
        replica.delete_text(fixture.body, start, start + take)?;
        self.deletions += 1;
        Ok("delete")
    }

    fn attribute(
        &mut self,
        replica: &mut Replica,
        fixture: &Fixture,
    ) -> Result<&'static str, CrdtError> {
        let value = ATTR_VALUES[self.attr_cursor % ATTR_VALUES.len()];
        self.attr_cursor += 1;
        replica.set_attr(fixture.para, "align", value)?;
        self.attrs += 1;
        Ok("attr")
    }

    fn undo(
        &mut self,
        replica: &mut Replica,
        fixture: &Fixture,
    ) -> Result<&'static str, CrdtError> {
        if replica.undo_depth() == 0 {
            // 没有可撤销的本地动作时退化为插入，保持动作总数与判据一致。
            return self.insert(replica, fixture);
        }
        replica.undo()?;
        self.undos += 1;
        Ok("undo")
    }

    fn structure(
        &mut self,
        replica: &mut Replica,
        fixture: &Fixture,
    ) -> Result<&'static str, CrdtError> {
        if !self.wrappers.is_empty() && !self.rng.chance(WRAP_PERCENT) {
            // 随机 undo 可能已经删掉了栈顶包裹节点，向后找仍然存活的。
            while let Some(wrapper) = self.wrappers.pop() {
                if replica
                    .doc()
                    .node(wrapper)
                    .is_some_and(|record| record.alive)
                {
                    replica.unwrap_node(wrapper)?;
                    self.unwraps += 1;
                    return Ok("unwrap");
                }
            }
        }
        let wrapper = replica.wrap_node(fixture.bold, NodeKind::Strong)?;
        self.wrappers.push(wrapper);
        self.wraps += 1;
        Ok("wrap")
    }
}
