# 全栈候选与验证顺序

日期：2026-09-16。用户已确认语言约束、候选验证方向以及优先兼容 WASM 的目标；下表不是已安装依赖或已通过验证的结论。最终版本、feature、许可证与性能结果随 spike 记录，并通过独立选型 ADR 固定。

## 阶段 0 已固定的选型

阶段 0 的七项否决性验证关闭了一部分候选。**已固定的部分以下列 ADR 为准**，本页候选清单只保留仍未定的层：

| 层 | 结论 | ADR |
|---|---|---|
| 桌面外壳 / UI 框架 | **egui / eframe 0.36.2**（唯一同时具备中文输入法与可访问性的候选；Iced 缺可访问性、GPUI 缺文本输入控件、EUI-NEO 不投入、Slint 被许可证阻断） | [ADR 0006](../adr/0006-native-ui-framework.md) |
| Typst 生成与预览集成 | 由 SDG 生成 Typst；锚点 + `Introspector::position` 双向定位；持久 World；异步编译 + 结果门；锁定 `typst 0.15.1` | [ADR 0007](../adr/0007-typst-integration.md) |
| 源码 reconcile 与构建隔离 | 行+范围归因（不确定即冲突）、`Raw` 逐字保留；**LaTeX 编译必须跑在 OS 级沙箱内** | [ADR 0008](../adr/0008-reconcile-and-toolchain-isolation.md) |
| CRDT 引擎 | **未定**（Proposed）：Loro / Yrs / Automerge 预筛均为 MIT 且无 C 依赖，对比夹具待跑 | [ADR 0009](../adr/0009-crdt-engine.md) |
| 依赖许可门禁 | `deny.toml` 已建并逐 workspace 通过；新增 `Apache-2.0 WITH LLVM-exception` | [ADR 0005](../adr/0005-llvm-exception-license.md) |

一键复现全部验证：`bash spikes/verify-stage0.sh`。

## 候选清单

| 层 | 首选验证方向与替代 | 关键验收 |
|---|---|---|
| GUI | **Iced → GPUI → C++ EUI-NEO → Slint 或 egui** | 遵守[原生 UI 验证计划](NATIVE_UI_VALIDATION.md)，中文 IME、数学结构、无障碍；另记浏览器可行性。Slint 为 GPLv3/商业/royalty-free 许可，非宽松，须先解决许可证问题（同计划第 3.1 节）；EUI-NEO 许可证待核实 |
| 权威文档与编辑动作 | 自研 Rust 语义图、数学树、Action、源码投影 | 独立于 GUI/CRDT/IO；两种权威模式不能形成双写副本 |
| 源码文本容器 | [Ropey](https://github.com/cessen/ropey) 候选 | 大文本、UTF-8/UTF-16 转换；协作模式中只能是可重建视图缓存，不能成为第二权威 |
| 文本 shaping/layout | 先复用 GUI 文本能力，缺口再比较 [cosmic-text](https://github.com/pop-os/cosmic-text) / [Parley](https://github.com/linebender/parley) | CJK、组合字符、双向文本、字体 fallback、光标命中；文本排版库不等于数学编辑器 |
| Typst 解析与排版 | [官方 Rust 引擎及 syntax](https://github.com/typst/typst) | 固定版本、增量编译、SourceMap、字体/包资源；浏览器编译另测 |
| LaTeX 语法与转换 | Rust 容错解析器；[Rowan](https://github.com/rust-analyzer/rowan) 候选 CST；[Tree-sitter](https://tree-sitter.github.io/tree-sitter/using-parsers/) 可选高亮/导航 | Rowan 不是解析器；Tree-sitter 不提供完整宏语义。无损原文、局部 patch、动态语法保守降级 |
| LaTeX 构建 | **[TeX Live](https://tug.org/texlive/) 工具链 → [Tectonic](https://tectonic-typesetting.github.io/en-US/)** | 先测 pdfLaTeX/XeLaTeX/LuaLaTeX 所需兼容基线，再测 Tectonic 的模板/宏包差异、离线包与部署；不能视为完全等价替换 |
| 矢量与 PDF 预览 | GUI 绘制优先；SVG 用 [resvg](https://github.com/linebender/resvg) 候选；PDF 用 [pdfium-render](https://github.com/ajrcarey/pdfium-render) 候选；[Vello](https://github.com/linebender/vello) 仅在绘制瓶颈时评估 | PDFium 是 C++ 依赖；不引入 Chromium UI。检查文字选择、命中映射、内存、跨平台与 WASM 构建 |
| 协作 CRDT | **[Loro](https://github.com/loro-dev/loro) → [Yrs](https://github.com/y-crdt/y-crdt) → [Automerge](https://github.com/automerge/automerge)** | 树移动、相对锚点、actor undo、乱序恢复、写集校验与语言 epoch；桌面/浏览器协议互通 |
| 产品历史 | 自研 Rust Action/checkpoint DAG，适配选定 CRDT | 撤销保留远端变更、可审计 revert/merge；不把引擎更新直接展示为版本史 |
| 本地存储 | **SQLite / [rusqlite](https://github.com/rusqlite/rusqlite) → [redb](https://github.com/cberner/redb)** | SQLite 含 C；redb 为 Rust 候选。断电恢复、迁移、跨文件物化、数据库与导出包边界；浏览器使用独立存储适配 |
| 调度与服务端 | 原生 Tokio；[Axum](https://github.com/tokio-rs/axum) + WebSocket 候选 | 会话串行提交、CPU 排版不阻塞异步执行器；重连、背压、身份/权限与 epoch。服务端数据库/blob 另选 |
| 原生互操作 | 纯 Rust 优先；C ABI 或 [CXX](https://cxx.rs/) 接 C/C++；Zig 按 C ABI 接入 | 生命周期、错误、取消、线程归属清晰；每个 WASM 原生依赖单独验证 |
| 隔离与发布 | Rust 编排，原生操作系统隔离，Cargo 构建 | 无 shell 拼接；资源限额、断网与取消进程树；发行格式待目标 OS 明确后验证 |
| 验证工具 | Rust 单元/属性/fuzz、恢复故障注入、原生 GUI 脚本与 WASM smoke | 同一语料验证跨端语义；无 npm/Node.js 依赖，具体测试工具按需要锁定 |

列表中的顺序是验证顺序，未明确排序的候选按实际缺口比较，不要求同时引入所有库。Tree-sitter 若采用生成解析器，应使用可复现的 C 产物或支持的 grammar.json 路径，避免 JS grammar 生成器成为项目必需依赖，见[生成器文档](https://tree-sitter.github.io/tree-sitter/cli/generate.html)。

## 存储与文件边界

SQLite/redb 用于项目持久化候选，不改变 `.tex` / `.typ` 源码项目的权威归属。数据库提交与源文件物化通过可恢复的 revision/manifest 协调；不能同时保留两个独立提交、互不协调的数据库 WAL 和自研权威 WAL。最终持久化格式须由恢复试验决定，当前 storage 布局仅是逻辑草案。

打开、保存两种标准源文件不依赖协作服务。混用原文包、标准目标包和 PDF 等最终产物是不同交付物；不因某候选能生成 PDF 就宣称支持双向源码转换。

## WASM 与实现参考

[WASM 兼容计划](WASM.md) 定义可移植核心、浏览器宿主和桌面专属能力。GUI 顺序保持不变，不能因某框架有 Web 演示就绕过编辑器验收。

LaTeX 参考 [Mogan 实现调研](../research/MOGAN_LATEX.md)：借鉴转换分层、导言区保留、模板适配和工具链探测，由 Rust 独立实现，不搬运其源码。
