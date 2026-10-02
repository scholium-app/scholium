# SPK-0053：主程序接入 A 的结构身份、代际与文件迁移基础

- 日期：2026-10-01；结论：**Pass（限定本地候选基础）**。
- 方向：[主程序接入计划 A](../plan/TYPST_MAIN_APP_INTEGRATION.md)；裁决：[ADR 0033](../adr/ADR-0033-structured-local-session-migration.md)。
- 基线：`93f01c3a57c33f064d4830c6e7cbc5d85ea9fed8`，开发分支 `codex/feat/structured-session-migration`；本报告同提交的源码为验证对象，指纹见 evidence/SPK-0053/source-hashes.json。
- 环境：Linux x86_64；门禁 Rust/Cargo 1.96.0（与根 CI 配置一致）；系统 SQLite 3.53.4。覆盖率另用已安装 llvm-tools 的 Rust 1.98.1。
- 独立探针：`spikes/structured-session-migration`，锁定 Cargo.lock；调用正式 model/document/storage，没有独立 Editor/UI，也不执行 Typst。

## 交付与主程序边界

正式 crate 新增 identified 本地结构、同一 LocalSession 的结构编辑/恢复，以及独立 SQLite v1
迁移。legacy API/默认类型仍兼容。没有修改 `scholium-app`/`scholium-typst` 的编辑绘制路径：
**普通启动仍是 egui 回显后切换 Typst，尚未交付去回显的主程序候选**。下一批 B 才把这些
类型接到真实 session/worker/page_editor，不能把本探针当作窗口验收。

所有旧 Math 原串保留为 RawMath（含合法 `frac(x, 2)` 和未知代码），本批解析转换数量为 0。
RawMath 无槽位/叶编辑能力，也未执行原串。正文/粗体/强调/标题原值与块身份保留，空块新增
可编辑空文字叶。全局节点身份、有限树深/字节/节点数与显式格式/tag 在读入时验证。

新数学覆盖 Text/Symbol/Row/Fraction/Hole；核心支持 literal 叶替换、数学插入/包分数、拆合块、
整叶样式与标题种类。存活节点保持身份；分数 wrapper/新分母和拆出的右叶/块获得新身份。
删除数学最后文字恢复同 ID Hole；必填 Hole 可以保存，ensure_filled 明确拒绝它。
这不是严格 Typst/PDF 输出验收：RawMath 的执行兼容与 unresolved 检查仍需输出 adapter。

## 可重复验证

从仓库根运行（探针输出目录必须为空，且不要使用实际用户会话）：

```sh
cargo +1.96.0 fmt --all -- --check
cargo +1.96.0 clippy --locked --workspace --all-targets -- -D warnings
cargo +1.96.0 test --locked --workspace
RUSTDOCFLAGS='-D warnings' cargo +1.96.0 doc --locked --workspace --no-deps
python3 scripts/check-rust-size.py
cargo deny --manifest-path Cargo.toml --locked --config deny.toml check licenses bans sources
CARGO_TARGET_DIR="$PWD/target" cargo +1.96.0 run --locked --manifest-path spikes/structured-session-migration/Cargo.toml -- /tmp/scholium-structure-evidence
```

探针使用仓库双许可 fixture；输出原库与独立候选库，读取现有 LocalSession 后接受五次
结构动作（插入数学、写分子、包分数、写分母、清空分母），保存退出重开，逐字段比较结构
和请求日志。撤销/重做使用已有单机 snapshot restore 语义，每次启动不同 epoch；不冒充
协作历史，也未把单机栈接入新 UI。

## 限定结果

| 项目 | 观察 |
|---|---|
| 跨模块 fixture | 三个旧快照逐字归档、两个公式 Raw 保留、六个 inline ID 首次赋值 |
| 新数学保存 | 五次结构动作后 revision=7；空分母/身份/完整请求身份重开一致 |
| 原文件保护 | 探针迁移/编辑/重开后源库主文件字节与迁移前一致；源连接只读事务 |
| 旧 writer | 旧裸 JSON 被新 snapshot SQL CHECK 拒绝；旧事务的 head/log 修改回滚 |
| 未知格式 | 未知数据库版本在建表/改变 journal 前拒绝；未来 envelope/tag/字段及额外 legacy 字段/表/列/元数据拒绝 |
| 不覆盖发布 | 已有目标、源=目标、dangling symlink、目标父目录缺失、构建错误均拒绝；损坏源/日志不生成目标 |
| 身份/动作 | Unicode 字素中间/UTF-8 中间、Raw 穿透、错误目标/过期/重复请求拒绝；no-op 不增 revision |
| 代际 | 恢复/撤销/重做/显式 reset 的 epoch 不同，布局请求与 profile/resource 独立于语义 revision |
| 容量边界 | 最大数学树深 24 在默认 serde_json 递归保护及 envelope 内可读回；多一级/超长叶拒绝 |

自动门禁输出（只整理行尾空白）见 [evidence/SPK-0053](evidence/SPK-0053/)；根工作区 **143 passed / 0 failed / 32 ignored**，新增 32 项测试，原有忽略的审计项未改变。总数与命令退出码
见 checks.json，跨模块摘要见 probe.json。model 行覆盖率 **208/219 = 94.98%**，高于 90% 合约；命令为 `cargo llvm-cov --locked -p scholium-model -p scholium-document -p scholium-storage --json --summary-only`，单独使用 Rust 1.98.1，结果见 coverage.json。根测试同时覆盖既有 UI/Typst/legacy 保存回归和
原有 SIGKILL 恢复，不能据此宣称新候选已完成断电/全 I/O 注入验证。

完整 advisories 检查仍 **Fail**（既有 Typst 链：quick-xml 漏洞、bincode/paste/rustybuzz/
ttf-parser/yaml-rust 维护通告），详见 advisories.log；未新增 ignore、升级 Typst 或默认启用 fork。
许可证/bans/sources 通过不能代替安全门禁。当前默认 Rust 1.98.1 会触发既有 caret.rs 的
新 some_filter lint；本报告门禁显式使用 CI 锁定的 1.96.0，未为此改动既有编辑器。

## 未覆盖与后续

- 本地动作仍使用临时全快照规划副本；没有逐键复杂度/32 ms 性能结论。
- 跨叶/块选区替换、局部格式、剪贴板、结构光标/导航、主程序 IME 与 worker 采纳待 B/C。
- SceneStamp 只是完整身份 DTO；新 worker 必须实际全字段采纳，才能关闭窗口过期场景竞态。
- 旧历史只有不可写原 JSON 档案；没有猜历史叶身份、重建历史动作或提供共享 DAG/协作撤销。
- 新候选文件出版使用 Linux 同目录硬链接；fsync 目录失败时可能留下完整目标但返回错误。
  本轮未模拟断电、磁盘满和每个同步点失败；不支持的宿主返回错误，不覆盖式降级。
- 旧数学全部 Raw 的能力限制必须在主程序候选可见，不得把保留原串称为可编辑转换。
- 默认切换仍受 B–E/K1–K4 与安全门禁；阶段 0 当前判定不更新，仍见报告 0012。
