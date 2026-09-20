//! 语义文档图：节点种类、固定槽位与文档 arena。
//!
//! 节点集合刻意压到能覆盖 UI 验收项的最小规模，不是正式数据模型的子集契约。

use crate::error::EditError;
use crate::ids::{CharId, NodeId};
use crate::text::TextLeaf;

/// 节点种类。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeKind {
    /// 文档根。
    Document,
    /// 段落。
    Paragraph,
    /// 章节标题。
    Heading,
    /// 文本叶子。
    Text,
    /// 行内或独立数学。
    Math,
    /// 分数：分子 / 分母。
    Fraction,
    /// 根式：被开方数。
    Sqrt,
    /// 上下标：底 / 下 / 上。
    Script,
    /// 定界符：内容。
    Delimited,
    /// 矩阵：单元格。
    Matrix,
    /// 未理解但保留的原文。
    Raw,
}

impl NodeKind {
    /// 固定槽位名。槽位永远存在，可放空占位节点，避免编辑时结构塌陷。
    pub fn slot_names(self) -> &'static [&'static str] {
        match self {
            NodeKind::Document => &["blocks"],
            NodeKind::Paragraph | NodeKind::Heading => &["inline"],
            NodeKind::Math => &["body"],
            NodeKind::Fraction => &["numerator", "denominator"],
            NodeKind::Sqrt => &["radicand"],
            NodeKind::Script => &["base", "sub", "sup"],
            NodeKind::Delimited => &["body"],
            NodeKind::Matrix => &["cells"],
            NodeKind::Text | NodeKind::Raw => &[],
        }
    }

    /// 槽位数量。
    pub fn slot_count(self) -> usize {
        self.slot_names().len()
    }

    /// 是否为文本叶子。
    pub fn is_text(self) -> bool {
        matches!(self, NodeKind::Text | NodeKind::Raw)
    }

    /// 结构槽位是否需要立即填充占位节点。
    ///
    /// 数学结构一旦空槽就必须保留槽位，否则光标无处可去；容器槽位则由调用方决定内容。
    fn wants_placeholders(self) -> bool {
        matches!(
            self,
            NodeKind::Math
                | NodeKind::Fraction
                | NodeKind::Sqrt
                | NodeKind::Script
                | NodeKind::Delimited
                | NodeKind::Matrix
        )
    }

    /// 结构变体数量，用于符号/结构循环。
    pub fn variant_count(self) -> u8 {
        match self {
            NodeKind::Fraction | NodeKind::Sqrt | NodeKind::Delimited => 3,
            NodeKind::Script => 4,
            NodeKind::Matrix => 2,
            _ => 0,
        }
    }
}

/// 语义图中的一个节点。
#[derive(Clone, Debug)]
pub struct Node {
    /// 稳定身份，永不复用。
    pub id: NodeId,
    /// 节点种类。
    pub kind: NodeKind,
    /// 父节点。根节点为 `None`。
    pub parent: Option<NodeId>,
    /// 固定槽位，下标即槽位编号。
    pub slots: Vec<Vec<NodeId>>,
    /// 文本内容，仅文本叶子非空。
    pub text: TextLeaf,
    /// 结构变体下标，由 `CycleVariant` 改变。
    pub variant: u8,
}

/// 语义文档图。
#[derive(Clone, Debug)]
pub struct Document {
    nodes: Vec<Node>,
    root: NodeId,
    next_char: u64,
    revision: u64,
}

impl Default for Document {
    fn default() -> Self {
        Self::new()
    }
}

impl Document {
    /// 建立包含根、一个段落和一个空文本叶子的文档。
    pub fn new() -> Self {
        let mut doc = Self {
            nodes: Vec::new(),
            root: NodeId::from_index(0),
            next_char: 1,
            revision: 0,
        };
        let root = doc.create_raw(NodeKind::Document, None, 0, 0);
        let paragraph = doc.create_raw(NodeKind::Paragraph, Some(root), 0, 0);
        doc.create_raw(NodeKind::Text, Some(paragraph), 0, 0);
        doc.root = root;
        doc.revision = 1;
        doc
    }

    /// 根节点。
    pub fn root(&self) -> NodeId {
        self.root
    }

    /// 当前 revision。每次结构或内容变化递增。
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// 节点数量，用于断言。
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// 按身份取节点。
    pub fn node(&self, id: NodeId) -> Result<&Node, EditError> {
        self.nodes.get(id.index()).ok_or(EditError::UnknownNode(id))
    }

    /// 按身份取可变节点。
    pub fn node_mut(&mut self, id: NodeId) -> Result<&mut Node, EditError> {
        self.nodes
            .get_mut(id.index())
            .ok_or(EditError::UnknownNode(id))
    }

    /// 新字符身份。只有文档能分配身份，避免调用方伪造。
    pub(crate) fn next_char_id(&mut self) -> CharId {
        let id = CharId::new(self.next_char);
        self.next_char += 1;
        id
    }

    /// 标记一次变更。
    pub(crate) fn bump_revision(&mut self) {
        self.revision += 1;
    }

    /// 创建一个节点并插入父节点槽位。数学结构会自动填充空占位子节点。
    pub fn create(
        &mut self,
        kind: NodeKind,
        parent: Option<NodeId>,
        slot: usize,
        index: usize,
    ) -> Result<NodeId, EditError> {
        let id = self.create_raw(kind, parent, slot, index);
        if kind.wants_placeholders() {
            for slot_index in 0..kind.slot_count() {
                self.create_raw(NodeKind::Text, Some(id), slot_index, 0);
            }
        }
        self.bump_revision();
        Ok(id)
    }

    /// 只入 arena 与父槽位，不递归填充占位节点。
    fn create_raw(
        &mut self,
        kind: NodeKind,
        parent: Option<NodeId>,
        slot: usize,
        index: usize,
    ) -> NodeId {
        let id = NodeId::from_index(self.nodes.len());
        let slots = vec![Vec::new(); kind.slot_count()];
        self.nodes.push(Node {
            id,
            kind,
            parent,
            slots,
            text: TextLeaf::new(),
            variant: 0,
        });
        if let Some(parent_id) = parent
            && let Some(node) = self.nodes.get_mut(parent_id.index())
            && let Some(list) = node.slots.get_mut(slot)
        {
            let at = index.min(list.len());
            list.insert(at, id);
        }
        id
    }

    /// 取槽位内容。
    pub fn slot(&self, node: NodeId, slot: usize) -> Result<&[NodeId], EditError> {
        let n = self.node(node)?;
        n.slots
            .get(slot)
            .map(Vec::as_slice)
            .ok_or(EditError::NoSuchSlot {
                node,
                kind: n.kind,
                slot,
            })
    }

    /// 取文本内容。非文本节点返回错误。
    pub fn text_of(&self, node: NodeId) -> Result<String, EditError> {
        let n = self.node(node)?;
        if !n.kind.is_text() {
            return Err(EditError::NotText {
                node,
                kind: n.kind,
            });
        }
        Ok(n.text.as_string())
    }

    /// 找到包含 `node` 的槽位下标与在槽内的位置。
    pub fn locate_in_parent(&self, node: NodeId) -> Option<(NodeId, usize, usize)> {
        let n = self.node(node).ok()?;
        let parent = n.parent?;
        let p = self.node(parent).ok()?;
        for (slot, list) in p.slots.iter().enumerate() {
            if let Some(index) = list.iter().position(|c| *c == node) {
                return Some((parent, slot, index));
            }
        }
        None
    }

    /// 第一个文本后代，用于把焦点放进一个结构。
    pub fn first_text_descendant(&self, node: NodeId) -> Option<NodeId> {
        let n = self.node(node).ok()?;
        if n.kind.is_text() {
            return Some(node);
        }
        for list in &n.slots {
            for child in list {
                if let Some(found) = self.first_text_descendant(*child) {
                    return Some(found);
                }
            }
        }
        None
    }

    /// 投影为纯文本，供预览面板与源码投影使用。
    pub fn to_plain_text(&self) -> String {
        let mut out = String::new();
        self.write_plain(self.root, &mut out);
        out
    }

    fn write_plain(&self, node: NodeId, out: &mut String) {
        let Ok(n) = self.node(node) else {
            return;
        };
        match n.kind {
            NodeKind::Text | NodeKind::Raw => out.push_str(&n.text.as_string()),
            NodeKind::Document => self.write_children(n, 0, out),
            NodeKind::Paragraph | NodeKind::Heading => {
                self.write_children(n, 0, out);
                out.push('\n');
            }
            NodeKind::Math => {
                out.push('$');
                self.write_children(n, 0, out);
                out.push('$');
            }
            NodeKind::Fraction => {
                self.write_children(n, 0, out);
                out.push('/');
                self.write_children(n, 1, out);
            }
            NodeKind::Sqrt => {
                out.push_str("sqrt(");
                self.write_children(n, 0, out);
                out.push(')');
            }
            NodeKind::Script => {
                self.write_children(n, 0, out);
                out.push('_');
                self.write_children(n, 1, out);
                out.push('^');
                self.write_children(n, 2, out);
            }
            NodeKind::Delimited => {
                out.push('(');
                self.write_children(n, 0, out);
                out.push(')');
            }
            NodeKind::Matrix => {
                out.push('[');
                self.write_children(n, 0, out);
                out.push(']');
            }
        }
    }

    fn write_children(&self, n: &Node, slot: usize, out: &mut String) {
        for child in &n.slots[slot] {
            self.write_plain(*child, out);
        }
    }
}
