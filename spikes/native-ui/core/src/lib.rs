//! Scholium 原生 UI 验证用的独立文档核心。
//!
//! 本 crate 只服务于阶段 0 第 1 项验证：给各 UI 候选提供同一份权威文档模型与同一组输入动作，
//! 使候选之间可比。它**不是**正式架构，也不实现 CRDT 收敛、真实 reconcile、持久化或语言切换协议。
//!
//! 位置量的单位约定（对照 `AGENT.md` 的注释规范）：
//!
//! - [`text::TextLeaf`] 的偏移一律是 UTF-8 **byte offset**，且必须落在字素边界上。
//! - 编辑器协议使用 UTF-16 code unit 的概念只出现在 UI 候选的适配层，核心不接触。
//! - 预览/窗口坐标不属于本 crate。

#![warn(missing_docs)]

pub mod action;
pub mod cursor;
pub mod doc;
pub mod edit;
pub mod error;
pub mod ids;
pub mod layout;
pub mod source;
pub mod text;

pub use action::{
    Action, ActionId, ActorId, Editor, History, Intent, InverseRecipe, RemoteEdit, UndoOutcome,
};
pub use cursor::{Cursor, Direction};
pub use doc::{Document, Node, NodeKind};
pub use edit::{EditError, SemanticEdit};
pub use ids::{CharId, NodeId};
pub use layout::{Item, Layout, Metrics, layout_document, layout_node};
pub use source::{Dialect, SourcePane};
pub use text::{Char, TextLeaf};
