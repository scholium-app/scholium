# SPK-0056：结构正文范围的单次原子替换

- 日期：2026-10-02；结论：**Pass（限定正文端点的原子替换内核）**。
- 对应：[主程序接入计划 C](../plan/TYPST_MAIN_APP_INTEGRATION.md)、[路线图结构编辑](../plan/ROADMAP.md)。
- main 基线：`510551d44947677e73df86e36fa70d73b26aeaaa`，已合入增量 Content 与结构会话基础。
- 版本：Rust/Cargo 1.96.0、Linux x86_64 / 7.2.7-arch1-1，既有 Cargo.lock；无新增依赖或存储 schema 变更。
- 指纹及原始日志：[evidence/SPK-0056](evidence/SPK-0056/)。本批独立于未合入的 Typst 主程序候选，不启用 fork。

## 合约与支持范围

正式 `LocalSession<StructuredDocument>` 接受 `ReplaceBodyRange`。两端都是稳定正文 Text
叶身份与叶内 UTF-8 扩展字素边界，要求按文档顺序传入。范围内完整数学/Raw inline 可以
删除，但它们的内部不能作为端点；不会把公式或样式展开成可写 markup。

首块与左叶身份保留，右叶的后缀、身份、样式及范围外节点保留。跨块合并移除覆盖块；
首块 kind 保留。多行插入创建新块并继承首块 kind；同叶拆行的右半叶获得新 ID。
CRLF/CR/LF 都作段落分隔，连续及末尾空行不丢失。`$`、`*`、`_`、反斜线仍为 literal。

端点、容量与最终结构在临时规划副本中校验，一次成功最多一条动作、一次 revision。
拒绝不改变快照、epoch 和日志，拒绝请求可修正后以原身份重试；等值替换不消耗请求身份。

## 复现与结果

```sh
cargo +1.96.0 fmt --all -- --check
cargo +1.96.0 clippy --locked --workspace --all-targets -- -D warnings
cargo +1.96.0 test --locked --workspace
RUSTDOCFLAGS='-D warnings' cargo +1.96.0 doc --locked --workspace --no-deps
python3 scripts/check-rust-size.py
cargo deny --manifest-path Cargo.toml --locked --config deny.toml check licenses bans sources
cargo deny --manifest-path Cargo.toml --locked --config deny.toml check advisories
```

10 项新增测试覆盖同叶 literal 替换、跨 inline 完整数学/Raw 删除、跨块删除、多行插入、
CRLF/CR/空行、存活身份和样式、Unicode/反向/错误目标拒绝、规划中途容量失败回滚、
同请求重试、no-op、过期/重复请求及恢复后完整结果和日志。main 分支的根工作区
**153 passed / 0 failed / 32 ignored**；fmt、clippy、doc、规模、licenses/bans/sources 通过。

额外兼容检查在本地叠加树 `326da2b`（完整 ID 见指纹文件）执行：它包含已推送的候选
基线 `8cc23f5` 与本批同一范围内核。隔离候选的 clippy 通过，**91 passed / 0 failed /
32 ignored**。这只证明现有候选能编译和运行原回归，不是页面已调用新 API 或新的窗口验收。

完整 advisories 仍 **Fail**：既有 bincode、paste、quick-xml 两项、rustybuzz、ttf-parser、
yaml-rust 共七项错误；本次另报告 yoke-derive 的 yanked 警告。未改变锁文件、增加 ignore，
也未把 licenses/bans/sources 成功称作安全审计成功。

## 未覆盖与下一步

主程序未接入该范围 API；普通启动仍先 egui 回显再 Typst。拖选、Shift 选区、复制剪切、
多行粘贴的产品入口、数学内部跨槽位替换和结构化剪贴板仍待实现。下一步用同源 Scene
的选区几何与稳定语义端点接入此请求，同时明确拒绝未知能力和旧 Scene。

本批仍复制全快照规划，未证明逐键复杂度、物理呈现时延、分页、实机 IME 或无障碍。
阶段 0 判定不升级，完整 C 与默认替换未验收。
