# 原生 UI 验证工程

阶段 0 第 1 项验证的共用工程，规格见 [原生 UI 验证计划](../../docs/NATIVE_UI_VALIDATION.md) 第 4 节。

## 结构

```text
spikes/native-ui/
├── core/            独立 Rust 文档核心（无 UI、无 IO），所有候选共用
├── candidate-iced/  第一个候选：Iced，自带 Cargo.lock
└── candidate-<next>/ 后续候选，互不共享 UI 代码与锁定版本
```

每个候选目录都是独立 workspace，通过 path 依赖引用 `core/`。这样候选之间不会发生 cargo
feature 合并，也不会有 UI 状态或类型泄漏进核心，符合"候选需要独立目录与锁定版本"。

## 核心边界

`core/` 只实现被 UI 验收项直接压到的模型能力，不是正式架构：

- 语义节点与固定槽位、树光标与结构导航
- 字符级稳定身份，使本地 undo 能精确定位自己插入的内容而不依赖"最近一次编辑"
- Action 分组、IME preedit 排除、actor 作用域的本地 undo
- 源码面板的方言与可写性模型（团队语言门禁在模型层的占位实现）
- 大文本缓冲区，用于源码视图性能采样

核心不实现 CRDT 收敛、真实 reconcile、持久化或语言切换协议——它们分别是阶段 0 第 3、4、5、6 项。
每个候选必须用同一组验收脚本跑核心，见各候选目录与 `docs/spikes/` 的报告。
