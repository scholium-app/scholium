# Scholium /「注疏」

本地优先、可协作、同时支持即时结构编辑与源码编辑的科学写作工作区。

> scholium：古典手稿页边的注疏。欧几里得《几何原本》的 scholia
> 是希腊数学史最重要的传世材料之一。

## 状态

阶段判定以 [阶段出口报告](docs/spikes/SPK-0012-exit-criteria.md) 为准。
根 workspace 提供独立的 egui 应用壳，启动显示界面示例；文件菜单“新建”可进入基础内存文本编辑，尚无保存或排版后端。
废弃实现已经删除，旧实验结果仍可在归档文档和 Git 历史中查阅。

新方案聚焦 Liii STEM 式结构编辑体验、LaTeX/Typst 源码模式、双后端预览、非线性历史、
local-first 多人协作和可解释格式转换。从 [设计文档索引](docs/README.md) 开始阅读。

## 原生技术栈

应用 UI、核心与服务端优先 Rust，必要时混用 C/C++/Zig；构建以 Cargo 为主，原生依赖可使用 CMake 等工具。
UI 验证顺序为 **Iced → GPUI → C++ EUI-NEO → Slint 或 egui**，已选定 egui / eframe 0.36.2（[ADR 0006](docs/adr/ADR-0006-native-ui-framework.md)）；不采用 npm/JavaScript/WebView 编辑器。
具体验收与 FFI 边界见 [原生 UI 验证计划](docs/plan/NATIVE_UI_VALIDATION.md)。

## 运行基础 UI

```bash
cargo run -p scholium-app
```

使用 Rust 1.96.0 和 egui / eframe 0.36.2；Linux 需要可用的 X11 或 Wayland 会话与 OpenGL/EGL 驱动。
中文字体随应用嵌入，无需额外安装。字体来源与许可见 [字体说明](crates/scholium-app/assets/fonts/README.md)。

- 默认是单一页面的所见即所得布局；`Ctrl+1` / `Ctrl+2` 切换页面和源码工作区。
- 源码模式默认等宽分屏；拖动分隔条调整，双击恢复等宽；分隔条获得焦点后支持左右方向键和 Home。
- `Ctrl+B` / `Ctrl+J` 开关文件导航与诊断，二者默认收起。
- 底部按钮调整页面缩放；`Ctrl+0` 恢复 100%，`Ctrl++` / `Ctrl+-` 放大和缩小。
- 窄窗口自动收起导航并提供源码 / 页面单窗格切换，保留可读字号。
- 正文和两种方言源码是只读布局示例。文件、格式和草稿写操作禁用，不产生项目文件或运行编译器。

主题颜色集中在 `crates/scholium-app/src/theme.rs` 的 `Palette`，正文、源码和 egui 控件共享语义颜色。
目前只提供默认深色主题；主题选择器、主题文件及偏好持久化留待后续接入。

基础 UI 的检查范围和原生窗口截图见 [应用壳验收记录](docs/log/ui-shell-review.md)。

基础检查：

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
python3 scripts/check-rust-size.py
cargo deny check licenses bans sources
```

## 基础接入（开发中）

文件菜单“新建”或 Ctrl+N 创建内存文档，支持中英文段落输入与选区编辑。
所有已接受修改经过本地会话校验，并记录请求和正文 revision。
**保存、撤销、恢复、数学结构、源码转换和排版尚未接入；关闭程序会丢失内存内容。**
再次新建或关闭窗口会要求确认丢弃。源码视图不把示例当作当前文档的输出。
详见 [ADR 0024](docs/adr/ADR-0024-local-paragraph-integration.md)。

## 设计目标

- **双工作形态。** 原生项目以语义图为真，外部 LaTeX/Typst/Markdown 项目以源码为真。
- **视觉与源码都是一等入口。** 默认即时结构编辑，Source Studio 可直接编辑 LaTeX 或 Typst。
- **双后端。** Typst 提供快速预览，LaTeX 提供兼容性验证和最终构建。
- **历史不是一条栈。** 撤销产生新的补偿变更，并支持 checkpoint、分支、合并、
  revert 和 cherry-pick。
- **本地优先协作。** 离线可编辑、保存和导出；联网后通过 CRDT 增量收敛。
- **转换必须可解释。** 保存原生格式无损；跨格式导出若有降级，必须提供报告。

## 已确认的兼容行为

- 同一共享项目分支在团队范围内只允许一种源码语言正在被编辑，同语言支持多人协作；两种编译器可并行。
- LaTeX/Typst 混用目标覆盖正文、公式、图表、宏、模板和跨片段引用；两种格式可打开保存、另存为与输出。
- 混合项目保留两种原文，标准目标包与双工具链重建包分别说明依赖和可编辑性。
- 共享分支的离线源码修改进入本地草稿/fork，重连后按团队活动语言显式合入。

具体规则及待验证边界见 [混合源码与团队编辑](docs/MIXED_SOURCE_EDITING.md)。

## 许可证

本项目以 **MIT OR Apache-2.0** 双许可发布，使用者可任选其一：[LICENSE-MIT](LICENSE-MIT) 或
[LICENSE-APACHE](LICENSE-APACHE)。决策依据见 [ADR 0004](docs/adr/ADR-0004-project-license.md)。

正式实现不移植 TeXmacs/Mogan 或废弃原型实现；新增依赖必须通过 `cargo deny` 许可证检查，
允许与禁止的许可类别见 [AGENT.md](AGENT.md) 的许可证政策一节。

## 全栈与 WASM

[全栈候选](docs/plan/TECH_STACK.md)记录编辑核心、解析/编译、渲染、协作、存储及服务端的验证方向；[WASM 计划](docs/plan/WASM.md)优先保证 Rust 核心可移植。[Mogan LaTeX 调研](docs/research/MOGAN_LATEX.md)记录源码依据与可借鉴边界。已验证的选型与仍待验证的范围见 [spike 状态索引](docs/spikes/README.md)。
