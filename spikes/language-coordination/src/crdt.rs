//! 简单的确定性操作日志，用于同语言多人编辑的收敛断言。
//!
//! 这里**不是** CRDT 选型（那是阶段 0 第 4 项）。它只提供"同一操作集在不同投递顺序下
//! 渲染结果一致"的最小模型，使语言门禁的测试不被合并算法差异干扰。
//!
//! 同时提供 `render_arrival`（按到达顺序拼接），用来证明收敛断言不是恒真。

use crate::model::{ActorId, Dialect};

/// 一条已接受的逻辑写入。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct LoggedOp {
    /// 提交者。
    pub actor: ActorId,
    /// 提交者内序号。
    pub seq: u64,
    /// 语言。
    pub dialect: Dialect,
    /// 资源路径。
    pub path: String,
    /// 源码文本。
    pub text: String,
}

/// 操作日志副本。
#[derive(Clone, Debug, Default)]
pub(crate) struct Replica {
    ops: Vec<LoggedOp>,
}

impl Replica {
    /// 空副本。
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// 应用一条操作。
    pub(crate) fn apply(&mut self, op: LoggedOp) {
        self.ops.push(op);
    }

    /// 确定性渲染：按 `(path, actor, seq)` 排序后拼接，与投递顺序无关。
    pub(crate) fn render(&self, dialect: Dialect) -> String {
        let mut selected: Vec<&LoggedOp> =
            self.ops.iter().filter(|op| op.dialect == dialect).collect();
        selected.sort_by(|a, b| {
            a.path
                .cmp(&b.path)
                .then_with(|| a.actor.0.cmp(&b.actor.0))
                .then_with(|| a.seq.cmp(&b.seq))
        });
        selected.iter().map(|op| op.text.as_str()).collect()
    }

    /// 按到达顺序渲染。仅用于证明"收敛断言有牙齿"：不同到达顺序会得到不同结果。
    pub(crate) fn render_arrival(&self, dialect: Dialect) -> String {
        self.ops
            .iter()
            .filter(|op| op.dialect == dialect)
            .map(|op| op.text.as_str())
            .collect()
    }

    /// 文本中是否出现指定标记。
    pub(crate) fn contains(&self, dialect: Dialect, needle: &str) -> bool {
        self.count(dialect, needle) > 0
    }

    /// 文本中出现的次数，用于"不产生双写"断言。
    pub(crate) fn count(&self, dialect: Dialect, needle: &str) -> usize {
        self.ops
            .iter()
            .filter(|op| op.dialect == dialect)
            .map(|op| op.text.matches(needle).count())
            .sum()
    }

    /// 操作条数。
    pub(crate) fn len(&self) -> usize {
        self.ops.len()
    }
}
