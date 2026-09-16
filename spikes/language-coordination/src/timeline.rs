//! 证据时间线：许可可写区间、接受序列、屏障与拒绝记录，以及重叠扫描。
//!
//! 判据 2 的"不存在两种语言同时可写的窗口"在这里被形式化为两条可扫描的检查：
//! 1. 不同方言的许可可写区间 `[issued, revoked)` 互不重叠；
//! 2. 接受序列中语言发生变化的位置，必须存在一次匹配的屏障（epoch 递增）。

use crate::model::{ActorId, Dialect};

/// 一段写许可的可写区间 `[issued, revoked)`；`revoked` 为 `None` 表示仍开放。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PermitWindow {
    /// 许可 ID。
    pub permit: u64,
    /// 持证人。
    pub actor: ActorId,
    /// 语言。
    pub dialect: Dialect,
    /// epoch。
    pub epoch: u64,
    /// 发放时刻。
    pub issued: u64,
    /// 撤销时刻。
    pub revoked: Option<u64>,
}

/// 一次被接受的写入（接受时刻由协调者串行决定）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AcceptedWrite {
    /// 接受时刻。
    pub tick: u64,
    /// 提交者。
    pub actor: ActorId,
    /// 序号。
    pub seq: u64,
    /// 语言。
    pub dialect: Dialect,
    /// 接受时 epoch。
    pub epoch: u64,
    /// 便于阅读的首条文本。
    pub marker: String,
}

/// 一次完成的语言切换屏障。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Barrier {
    /// 提交时刻。
    pub tick: u64,
    /// 原语言。
    pub from: Dialect,
    /// 新语言。
    pub to: Dialect,
    /// 新 epoch。
    pub epoch: u64,
}

/// 一次被拒绝的提交。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Rejection {
    /// 时刻。
    pub tick: u64,
    /// 提交者。
    pub actor: ActorId,
    /// 序号。
    pub seq: u64,
    /// 原因。
    pub reason: String,
}

/// 证据时间线。
#[derive(Clone, Debug, Default)]
pub(crate) struct Timeline {
    /// 许可窗口。
    pub permits: Vec<PermitWindow>,
    /// 接受序列（按 tick 升序）。
    pub accepted: Vec<AcceptedWrite>,
    /// 完成的屏障。
    pub barriers: Vec<Barrier>,
    /// 拒绝记录。
    pub rejections: Vec<Rejection>,
    /// 已观察到的最大逻辑时刻。
    pub last_tick: u64,
}

impl Timeline {
    /// 记录许可发放。
    pub(crate) fn grant(
        &mut self,
        permit: u64,
        actor: ActorId,
        dialect: Dialect,
        epoch: u64,
        tick: u64,
    ) {
        self.permits.push(PermitWindow {
            permit,
            actor,
            dialect,
            epoch,
            issued: tick,
            revoked: None,
        });
        self.touch(tick);
    }

    /// 记录许可撤销。
    pub(crate) fn revoke(&mut self, permit: u64, tick: u64) {
        for window in &mut self.permits {
            if window.permit == permit {
                window.revoked = Some(tick);
            }
        }
        self.touch(tick);
    }

    /// 记录一次接受。
    pub(crate) fn accept(&mut self, write: AcceptedWrite) {
        self.touch(write.tick);
        self.accepted.push(write);
    }

    /// 记录一次屏障完成。
    pub(crate) fn barrier(&mut self, barrier: Barrier) {
        self.touch(barrier.tick);
        self.barriers.push(barrier);
    }

    /// 记录一次拒绝。
    pub(crate) fn reject(&mut self, rejection: Rejection) {
        self.touch(rejection.tick);
        self.rejections.push(rejection);
    }

    /// 更新最大逻辑时刻。
    pub(crate) fn touch(&mut self, tick: u64) {
        self.last_tick = self.last_tick.max(tick);
    }

    /// 扫描不同方言的许可可写区间是否重叠。
    ///
    /// 这是"两种语言同时可写"的直接形式化检查：返回所有重叠的不同方言窗口对。
    pub(crate) fn cross_dialect_permit_overlaps(&self) -> Vec<(PermitWindow, PermitWindow)> {
        let horizon = self.last_tick + 1;
        let end = |window: &PermitWindow| window.revoked.unwrap_or(horizon);
        let mut overlaps = Vec::new();
        for (index, left) in self.permits.iter().enumerate() {
            for right in self.permits.iter().skip(index + 1) {
                if left.dialect == right.dialect {
                    continue;
                }
                if left.issued < end(right) && right.issued < end(left) {
                    overlaps.push((left.clone(), right.clone()));
                }
            }
        }
        overlaps
    }

    /// 扫描接受序列：相邻两次接受语言不同却缺少匹配屏障的位置。
    pub(crate) fn dialect_changes_without_barrier(&self) -> Vec<(AcceptedWrite, AcceptedWrite)> {
        let mut violations = Vec::new();
        for pair in self.accepted.windows(2) {
            let (first, second) = (&pair[0], &pair[1]);
            if first.dialect == second.dialect {
                continue;
            }
            let bridged = self.barriers.iter().any(|barrier| {
                barrier.tick > first.tick
                    && barrier.tick <= second.tick
                    && barrier.from == first.dialect
                    && barrier.to == second.dialect
            });
            if !bridged {
                violations.push((first.clone(), second.clone()));
            }
        }
        violations
    }

    /// 屏障提交后，旧许可撤销到新语言许可发放之间的间隔（逻辑 tick）。
    ///
    /// 屏障若正确，该间隔必须 `>= 1`，即不存在两种语言同时持有有效许可的瞬间。
    pub(crate) fn smallest_barrier_gap(&self) -> Option<u64> {
        let mut smallest: Option<u64> = None;
        for barrier in &self.barriers {
            let revoked = self
                .permits
                .iter()
                .filter(|w| w.dialect == barrier.from)
                .filter_map(|w| w.revoked)
                .max()?;
            let granted = self
                .permits
                .iter()
                .filter(|w| w.dialect == barrier.to && w.epoch == barrier.epoch)
                .map(|w| w.issued)
                .min()?;
            if granted < revoked {
                return Some(0);
            }
            let gap = granted - revoked;
            smallest = Some(smallest.map_or(gap, |current: u64| current.min(gap)));
        }
        smallest
    }

    /// 某语言被接受的写入条数。
    pub(crate) fn accepted_count(&self, dialect: Dialect) -> usize {
        self.accepted
            .iter()
            .filter(|write| write.dialect == dialect)
            .count()
    }
}
