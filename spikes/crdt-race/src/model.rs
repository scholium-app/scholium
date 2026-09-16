//! 最小语义文档图模型：节点种类。
//!
//! 种类集合压到能覆盖 "文本 + 行内包裹 + 块级容器 + 数学" 的最小规模，
//! 不是正式数据模型的子集契约。`docs/DATA_MODEL.md` 的固定槽位、Raw 保留等
//! 都不在本 spike 范围内。

/// 节点种类。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum NodeKind {
    /// 文档根。
    Document,
    /// 段落。
    Paragraph,
    /// 章节标题。
    Heading,
    /// 行内加粗包裹。
    Strong,
    /// 行内强调包裹。
    Emphasis,
    /// 行内公式容器。
    Math,
    /// 文本叶子，持有该节点自己的文本 CRDT 序列。
    Text,
}

impl NodeKind {
    /// 判别式，用于序列化。
    pub(crate) fn tag(self) -> u8 {
        match self {
            Self::Document => 1,
            Self::Paragraph => 2,
            Self::Heading => 3,
            Self::Strong => 4,
            Self::Emphasis => 5,
            Self::Math => 6,
            Self::Text => 7,
        }
    }

    /// 从判别式还原。
    pub(crate) fn from_tag(tag: u8) -> Option<Self> {
        match tag {
            1 => Some(Self::Document),
            2 => Some(Self::Paragraph),
            3 => Some(Self::Heading),
            4 => Some(Self::Strong),
            5 => Some(Self::Emphasis),
            6 => Some(Self::Math),
            7 => Some(Self::Text),
            _ => None,
        }
    }

    /// 渲染时的包裹标记。`None` 表示容器，直接拼接子节点。
    pub(crate) fn markers(self) -> Option<(&'static str, &'static str)> {
        match self {
            Self::Strong => Some(("**", "**")),
            Self::Emphasis => Some(("_", "_")),
            Self::Math => Some(("$", "$")),
            Self::Heading => Some(("# ", "\n")),
            Self::Paragraph => Some(("", "\n")),
            Self::Document | Self::Text => None,
        }
    }
}
