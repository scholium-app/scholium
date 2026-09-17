# Spike 0016：当前工作区状态与补齐验证

> **有效性（2026-09-17 登记）**：时点工作区状态记录，只作追溯。阶段 0 当前判定见 [报告 0012](0012-exit-criteria.md)。

- 结论：**Pass（本轮自动检查）；阶段 0 尚未关闭。**
- 日期：2026-09-16。
- 基线：`62d82ba` 加本机已有未提交的 spike 修改（含新增文件）；不是该提交本身的结果。
- 范围：核对实现、运行全量脚本和新增事务/门禁回归、整理文档；本轮未修改实现代码。
- 平台：Linux 7.2.4-arch1-2；Rust 1.98.0-nightly（bd08c9e71）；bubblewrap 0.12.0；
  TeX Live 2026；Typst CLI 0.15.1；Rust 依赖以各目录 Cargo.lock 为准。
- 关联：[前轮复核](0015-verification-review.md)、[出口判定](0012-exit-criteria.md)、[路线图](../plan/ROADMAP.md)。

## 本次自动验证

| 分段 | 通过 / 失败 | 范围 |
|---|---|---|
| core | 1 / 0 | 37 个核心测试，含新增选区 3 个 |
| ui | 5 / 0 | egui 16 个非 ignored 测试、Iced 输入契约、三个候选构建；未重跑桌面 IME/无障碍 |
| typst | 4 / 0 | 主流程、字符映射、渲染比对、时延和结果门 |
| reconcile | 1 / 0 | 原有两方言程序夹具 |
| items | 5 / 0 | CRDT、引擎对比、恢复、语言协调、混合构建；混合构建 36/36 |
| packages | 1 / 0 | 28/28 包重建，另有两种宿主的缺组件拒绝对照 |
| preview | 1 / 0 | 真实 Typst 后台编译 + headless UI 帧采样 |
| security | 1 / 0 | 21 条安全检查，含良性编译及正负对照 |
| license | 12 / 0 | licenses / bans / sources；跳过 advisories |
| web | 1 / 0 | 脚本检查的 npm 工程标记未发现 |
| **合计** | **32 / 0** | 脚本退出码 0；不是出口条件计数 |

另跑：`spikes/tests/verify-stage0.sh` 四场景通过；source-reconcile 的 2 个事务集成测试通过；
mixed-build 的 4 个单元/集成测试通过。`all` 没有调用这两项独立 crate 的 `cargo test`，不能省略补测。

原始全量日志：本机 `/tmp/scholium-stage0.FNz0cI/`。为避免临时目录清理丢失关键证据，保留
[包重建日志](evidence/0016/packages.log)、[预览日志](evidence/0016/preview.log)、
[安全日志](evidence/0016/security.log)。日志中的绝对路径只定位本次运行。

## 已补齐的实现与边界

### 原生编辑与源码应用

- `native-ui/core/src/selection_edit.rs` 支持以文本为两端的跨节点删除，移除完整覆盖的中间结构，
  批量编辑原子提交；测试覆盖撤销保留远端插入和错误不产生部分修改。
- egui 已接入 Copy/Cut/Paste；剪贴板输出是**纯文本**。槽位端点仍拒绝；不能据此宣称完整结构剪贴板或任意 TreeSelection 已通过。
- `source-reconcile::Session` 捕获 revision，先在候选副本应用，再核验整份草稿 round-trip，最后提交。
  过期草稿、多行不可归因修改均拒绝且不改变正文/历史。egui 已接入可写源码、显式应用、草稿保留和重新生成。
- 范围仍是单处可归因编辑；源码多处编辑、完整 CST、文件保存/重开、协作语言协议未由这个界面实现。
  新路径只做了 headless 验证，没有重新执行真实桌面共同验收。

### 后台预览

80 ms 去抖，待处理请求合并，后台子进程在沙箱中执行 Typst CLI，生成首页 PNG；
只采纳与当前正文 revision 相等的结果，旧图保留并显示 revision。

本次真实编译测试：73 个采样帧，UI 逻辑 p95 **1.76 ms**，总等待 **1.35 s**，
采纳结果的编译及栅格化 **580 ms**。这是小夹具、headless `egui::Context::run_ui` 的测量，
不含 GPU 上传与呈现、合成器、真实输入到显示延迟，也不是 20 页压力下的原生窗口性能。

另一个 20 页同步逻辑基准：单字符路径 p95 20.9 ms，改首段最大 210.6 ms。
两种测量不可混用；“帧不阻塞”已有机制证据，完整窗口的 16 ms 验收仍待补。

### 工具链沙箱

`spikes/toolchain-sandbox.sh` 统一提供 Linux profile：只读项目、独立可写输出、临时 HOME、
隔离网络 namespace、清空环境、超时与 prlimit。补齐纸张/TeX 配置挂载后，良性 PDF 编译及正文读回通过。
越界绝对路径、父目录、符号链接读取被拒；项目文件可读；shell escape 关闭，输入只读、越界写入、
超时子进程停止和 64 MiB 单文件限制均有对照证据。

**安全边界**：`/usr`、库目录、字体/TeX/纸张配置和 `/var/lib/texmf` 是项目根外的只读运行时例外。
因此不能写成“所有项目根外文件都不可读”。当前挂载较宽，尚未形成最小资源白名单；
未验证 seccomp、进程数/总磁盘配额、内存耗尽攻击及其他操作系统。
Typst `World` 的 read/plugin 拒绝不等于所有构建入口都已接入这一安全边界；混合构建普通运行也不自动由该脚本包裹。

### 标准包与混合快照包

`mixed-build --packages` 对 7 个成功夹具 × 2 宿主 × 2 包类型运行 **28 次重建**。
沙箱只挂生成包、系统运行时和声明工具链，不挂仓库、用户目录、Cargo 缓存或原构建目录。
逐件核验页数、规范化提取文本、链接目标以及无栅格图像；两种宿主均验证缺失矢量组件导致构建失败。

- 标准包：宿主源码 + 所需预编译矢量组件；重建只需目标编译器。外语组件重新生成仍需另一工具链。
- 混合包：以实验性 `snapshot.json` 为重建权威，依赖当前 spike driver（内含 Typst）和 XeLaTeX；
  附带源码是生成投影，手改投影不会被导入。这不是正式存储格式，也不证明完整编辑后重建工作流。
- “干净环境”指本机声明运行时的隔离挂载，不是全新系统安装或跨平台可移植性。
- 未验证行内矢量嵌入、组件内部符号/链接保真、复杂浮动体与超长文档；PDF 检查不等于逐像素视觉一致。

产物位于 `spikes/mixed-build/out/packages/`（可重新生成，不作为源码提交）。

## 复现

从仓库根执行；准备 TeX Live、Poppler、bubblewrap、prlimit、timeout，以及 Typst CLI 0.15.1。
依赖离线缓存位于 `spikes/native-ui/.cargo-home`；缺缓存须先准备，不能把环境缺失当通过。

```bash
export CARGO_HOME="$PWD/spikes/native-ui/.cargo-home"
# CLI 不在默认位置时，设置 SCHOLIUM_TYPST_BIN 为 typst 0.15.1 的绝对路径。
bash spikes/verify-stage0.sh all
bash spikes/tests/verify-stage0.sh
cargo test --release --offline --manifest-path spikes/source-reconcile/Cargo.toml
cargo test --release --offline --manifest-path spikes/mixed-build/Cargo.toml
```

可单独重跑 `packages`、`preview`、`security` 分段；`packages` 会重建其 `out/packages` 目录。
安全负对照仅对程序生成的固定文档启用 shell escape，不接受用户项目输入。

## 下一步

1. 在真实窗口重新验收跨节点选区、剪贴板、源码编辑与输入法焦点；明确槽位端点的支持范围。
2. 补实际呈现帧与大文档后台预览数据，分开定义 UI 帧预算和预览更新预算。
3. 明确可信运行时例外、收紧沙箱挂载，并核对每个不可信构建入口。
4. 解决混合内容行内嵌入与内部语义保真，明确桥接矩阵和实验包到产品交付包的边界。
5. 再复核阶段出口；Loro 性能对比、真实网络/磁盘集成仍按各原始报告保留，不能被本次汇总覆盖。
