//! 夹具：注入共享核心的标准文档，作为两种引擎的**同一份**输入。
//!
//! 复用 `scholium-spike-core::fixture::build_standard`，而不是各自现造一个文档：
//! 判据 1 要证明的是"同一份文档经两条路径构建互不干扰"，如果两条路径吃的是
//! 两份不同的夹具，隔离就无从谈起。

use scholium_spike_core::{Editor, NodeId, fixture};

/// 注入标准夹具，返回编辑器与段落节点。
///
/// 段落文本含中文与 `_` / `$` / `%` / `&` 这些两门语言都敏感的字面量，
/// 用来暴露转义缺口（见报告"失败与不确定性"），而不是把缺口藏起来。
pub fn editor() -> (Editor, NodeId) {
    let mut editor = Editor::new();
    let paragraph = fixture::build_standard(&mut editor);
    (editor, paragraph)
}
