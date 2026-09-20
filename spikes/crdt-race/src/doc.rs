//! 文档：结构树 + 每个文本叶子的文本序列。
//!
//! 文本按节点分片（一个文本叶子一个 [`TextCrdt`]）。这样结构操作与文本操作互相独立：
//! 把节点搬到别的父级下不会改变该节点文本的位置标识，因为位置标识只在节点内部有语义。
//! 这既是收敛上的简化，也是 `docs/DATA_MODEL.md` 稳定身份的实际收益。

use std::collections::{BTreeMap, HashSet};

use crate::codec::{Reader, Writer};
use crate::ids::{Id, NodeId};
use crate::op::Op;
use crate::text::TextCrdt;
use crate::tree::TreeCrdt;

/// 渲染时的最大递归深度。并发移动可能让父指针成环，深度上限保证渲染不 panic。
const MAX_RENDER_DEPTH: usize = 64;

/// 一个文档副本的 CRDT 状态。
#[derive(Clone, Debug, Default)]
pub(crate) struct Doc {
    tree: TreeCrdt,
    texts: BTreeMap<NodeId, TextCrdt>,
}

impl Doc {
    /// 空文档（没有任何节点，连根都没有；根由引导操作创建）。
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// 应用一个操作。必须幂等：重复操作不改变状态。
    pub(crate) fn apply(&mut self, op: &Op) {
        match op {
            Op::NodeCreate {
                op,
                kind,
                parent,
                pos,
                ts,
            } => self
                .tree
                .apply_create(*op, *kind, *parent, pos.clone(), *ts),
            Op::NodePlace {
                node,
                parent,
                pos,
                ts,
                ..
            } => self.tree.apply_place(*node, *parent, pos.clone(), *ts),
            Op::NodeAlive {
                node, alive, ts, ..
            } => {
                self.tree.apply_alive(*node, *alive, *ts);
            }
            Op::NodeAttr {
                node,
                key,
                value,
                ts,
                ..
            } => self.tree.apply_attr(*node, key, value.as_deref(), *ts),
            Op::TextInsert { op, node, pos, ch } => {
                self.texts
                    .entry(*node)
                    .or_default()
                    .apply_insert(*op, pos.clone(), *ch);
            }
            Op::TextAlive {
                node,
                char,
                alive,
                ts,
                ..
            } => {
                self.texts
                    .entry(*node)
                    .or_default()
                    .apply_alive(*char, *alive, *ts);
            }
        }
    }

    /// 结构树。
    pub(crate) fn tree(&self) -> &TreeCrdt {
        &self.tree
    }

    /// 文本序列；该节点从未有过文本操作时返回 `None`。
    pub(crate) fn text(&self, node: NodeId) -> Option<&TextCrdt> {
        self.texts.get(&node)
    }

    /// 文本序列的可变访问；没有则建立空序列。
    ///
    /// 只在**已完成输入校验**的本地编辑路径上调用：凭空建立一个空文本序列会进入规范状态，
    /// 若两个副本因此产生不同的空序列集合，收敛判据就会失败。
    pub(crate) fn text_or_default(&mut self, node: NodeId) -> &mut TextCrdt {
        self.texts.entry(node).or_default()
    }

    /// 节点记录。
    pub(crate) fn node(&self, id: NodeId) -> Option<&crate::tree::NodeRecord> {
        self.tree.node(id)
    }

    /// 存活子节点，按兄弟顺序。
    pub(crate) fn children(&self, parent: NodeId) -> Vec<NodeId> {
        self.tree.children(parent)
    }

    /// 渲染整份文档，用于人眼核对。
    pub(crate) fn render(&self) -> String {
        let mut out = String::new();
        let mut visited = HashSet::new();
        self.render_node(Id::root(), &mut out, 0, &mut visited);
        out
    }

    /// 渲染指定节点子树。
    pub(crate) fn render_node(
        &self,
        id: NodeId,
        out: &mut String,
        depth: usize,
        visited: &mut HashSet<NodeId>,
    ) {
        if depth > MAX_RENDER_DEPTH || !visited.insert(id) {
            return;
        }
        let Some(record) = self.tree.node(id) else {
            return;
        };
        if !record.alive {
            return;
        }
        let Some(kind) = record.kind else {
            return;
        };
        if kind == crate::model::NodeKind::Text {
            if let Some(text) = self.texts.get(&id) {
                out.push_str(&text.render());
            }
            return;
        }
        let (open, close) = kind.markers().unwrap_or(("", ""));
        out.push_str(open);
        for child in self.children(id) {
            self.render_node(child, out, depth + 1, visited);
        }
        out.push_str(close);
    }

    /// 某个文本叶子的可见文本。
    pub(crate) fn render_text_node(&self, node: NodeId) -> String {
        self.texts
            .get(&node)
            .map(TextCrdt::render)
            .unwrap_or_default()
    }

    /// 规范状态：结构树 + 每个文本叶子的条目，顺序完全确定。
    pub(crate) fn write_canonical(&self, w: &mut Writer) {
        self.tree.write_canonical(w);
        w.u32(self.texts.len() as u32);
        for (node, text) in &self.texts {
            w.id(*node);
            text.write_canonical(w);
        }
    }

    /// 完整快照。
    pub(crate) fn write_snapshot(&self, w: &mut Writer) {
        self.tree.write_snapshot(w);
        w.u32(self.texts.len() as u32);
        for (node, text) in &self.texts {
            w.id(*node);
            text.write_snapshot(w);
        }
    }

    /// 从快照读回。
    ///
    /// # Errors
    ///
    /// 输入截断或字段非法时返回错误。
    pub(crate) fn read_snapshot(r: &mut Reader<'_>) -> Result<Self, crate::error::CrdtError> {
        let tree = TreeCrdt::read_snapshot(r)?;
        let count = r.u32()? as usize;
        let mut texts = BTreeMap::new();
        for _ in 0..count {
            let node = r.id()?;
            let text = TextCrdt::read_snapshot(r)?;
            texts.insert(node, text);
        }
        Ok(Self { tree, texts })
    }

    /// 全文档最长的位置标识位数。反复在同一间隙插入会让位置标识变长，是规模与存储成本的关键量。
    pub(crate) fn max_position_len(&self) -> usize {
        self.texts
            .values()
            .flat_map(|text| text.iter_positions())
            .map(<[u32]>::len)
            .max()
            .unwrap_or(0)
    }

    /// 规模证据用的文本统计：`(条目数, 可见数, 最大块数)`。
    pub(crate) fn text_totals(&self) -> (usize, usize, usize) {
        let mut entries = 0usize;
        let mut visible = 0usize;
        let mut chunks = 0usize;
        for text in self.texts.values() {
            entries += text.entry_count();
            visible += text.visible_len();
            chunks = chunks.max(text.chunk_count());
        }
        (entries, visible, chunks)
    }
}
