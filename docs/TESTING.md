# 测试策略

## 1. 测试金字塔

- 纯领域单元测试：语义节点、TreeCursor、位置转换、patch 前置条件、DAG、权限规则。
- 属性测试：共享树/文本操作序列、源码 reconcile、解析 round-trip、随机 Unicode 编辑、日志截断恢复。
- 夹具/golden：真实 LaTeX、Markdown、BibTeX 项目和各格式导出。
- 组件测试：storage、构建沙箱、sync-server 持久化。
- 端到端：桌面 UI 到文件、两端/三端协作、断线与重启。

## 2. 夹具仓库

`fixtures/` 按能力而非来源组织，包含最小用例与真实匿名项目。每个夹具声明许可证、入口、编码、
预期诊断、依赖图、round-trip 等级和允许的导出降级。含私密论文的样本不得进入仓库。

最低覆盖：中文、emoji、组合字符、结构化数学、矩阵、CRLF、非 UTF-8、LaTeX/Typst 动态代码、宏嵌套、verbatim、损坏语法、循环 input、
重复 cite key、大图、缺失资源和恶意路径。

## 3. 收敛测试

生成多个 actor 的编辑动作，随机排列合法网络交付顺序，要求：

- 所有副本最终文本、state vector 和资源目录一致。
- update 重复应用不改变结果。
- 本地 undo 只改变目标 actor 的有效动作。
- 保存/加载/快照压缩前后状态一致。
- 分支 merge 在任意重放顺序下得到同一结构状态和同一冲突集合。
- 一端视觉编辑、另一端源码 reconcile 后，SDG/源码权威状态收敛且 Raw 不丢失。

失败用例必须收缩并作为固定回归种子保存。

## 4. 格式测试

- 未编辑打开—保存：字节级一致。
- 局部编辑：未触及范围字节一致。
- parse—project—generate：支持子集语义一致。
- error recovery：一个错误不吞掉文件余下部分。
- copy-as：纯文本与目标 MIME 内容一致，降级报告稳定。
- SDG → LaTeX/Typst → SDG：受支持节点语义一致，Raw 和降级逐节点记录。
- export：artifact hash 可变时比较规范化结构，不盲目比较二进制 hash。

跨格式转换使用真实 LaTeX/Typst 夹具，逐节点断言 Exact/Equivalent/RawPreserved/Degraded/Dropped，
报告覆盖未投影原文、宏、模板与资源。Raw 同方言无损保留，跨方言降级或阻止导出；保存原文不等于成功转换。
目标源码与 PDF 在声明的干净工具链环境验证，失败不覆盖原项目或上次成功产物。

### 项目语言与内核

- 创建/导入固定 ProjectLanguage；Typst 支持 SDG 与 .typ 源码形态，LaTeX 以 .tex 源码为权威。
- 同语言多人在线/离线编辑收敛；权限撤销、IME、草稿恢复与幂等回执保持既有要求。
- 「转换为…」建立新项目并返回逐节点报告，原项目与历史保留，运行时不切换项目语言。
- Typst 与 LaTeX 各自验证预览与最终输出同一引擎、旧 revision 淘汰、定位与诊断。
- LuaLaTeX + rtex 实时预览待评估中文、逐字形一致性、沙箱、绘制、定位与稳定性；pdfLaTeX/XeLaTeX 标明整编预览。

混合组件、跨引擎桥接、混合重建包与团队语言切换已按
[ADR 0041](adr/ADR-0041-separate-latex-typst-kernels.md) 推迟，阶段 0 报告仅作追溯。

## 5. 恢复测试

在日志写入、fsync、文件临时写、rename、快照和索引更新的每个边界注入崩溃。重启后结果只能是
动作前或动作后，不能出现半个 UTF-8 字符、半个多文件重构或无法解释的分叉。
另外在草稿落盘、转换目标生成、输入快照和发布目录边界注入崩溃；失败转换不改变原项目，
失败导出不覆盖原文和上次成功产物。私人草稿默认不随共享/发布包泄漏。

## 6. 性能预算

| 路径 | 预算 |
|---|---|
| 输入到本地确认 p95 | 16ms |
| update 追加日志 p95 | 8ms |
| 10 万行增量解析 p95 | 50ms，后台 |
| Typst 快速预览：逻辑侧完成 | 实测（20 页 / 257 段，release，复用编译 World）：单字符路径合计中位 19.2ms、**p95 21.3ms**；改动首段导致源码整体平移时 **p95 207.8ms**（[报告 0005](spikes/SPK-0005-typst-mapping.md)） |
| Typst 快速预览：帧内开销 | **不得同步编译**——帧内只入队与取用最新结果；只采纳比已落地更新的 revision（旧结果一律丢弃，含负对照测试） |
| 文件切换到可编辑 p95 | 100ms |
| presence 显示端到端 p95 | 250ms（正常网络） |
| 冷启动打开中型项目 | 2s 内出现可编辑首屏 |

CI 跑稳定微基准趋势；发布门禁在固定机器跑场景基准，不用共享 runner 的绝对值判定。
LaTeX 内核另测实时段落更新、首个可用预览、后台整编与定位延迟；rtex 的预算由专项评估确定。
阶段 0 已用真实夹具把预览预算定下来（见上表）。**16ms 输入预算指的是「输入到本地确认」这一步，
不是完整排版在 16ms 内完成的承诺**；预览刷新按百毫秒量级设计，可见页优先。

## 7. 兼容性

- UI/core 协议：当前版与前一版双向兼容测试。
- sync 协议：旧客户端加入、新字段忽略、拒绝不支持的破坏性版本。
- project.toml：旧 schema 迁移必须幂等并保留备份。
- snapshot/log：至少支持读取上一个公开版本。

## 原生 UI 验证门禁

Iced → GPUI → C++ EUI-NEO → Slint 或 egui 使用 [同一验收集](plan/NATIVE_UI_VALIDATION.md)。
实测中文预编辑/提交/取消、数学光标、结构选区、大源码、远端编辑、原生可访问树和三平台差异。
无显示服务器的逻辑测试不能代替真实窗口/输入法测试；浏览器自动化不能作为原生 UI 验收证据。
检查正常构建与运行无需 Node.js/npm/WebView；原生 FFI 做生命周期和错误边界测试。

## 8. CI 门禁

每个 PR：格式化、lint、单元、属性测试短集、round-trip、协议兼容、许可证和漏洞扫描。
阶段 0 的验证可用一条命令复现：`bash spikes/verify-stage0.sh`
（分段可跑：`core` / `ui` / `typst` / `reconcile` / `items` / `security` / `license` / `web`）。
门禁注意两点：`cargo deny check` 必须**同时**跑 `licenses` / `bans` / `sources`，
且 `advisories` 需要联网获取 RustSec 数据库（离线环境跑不了，CI 必须跑）；
`cargo deny` 只能在**每个 crate** 运行，仓库根 workspace 没有成员。
每日：完整 fuzz corpus、三端收敛、崩溃矩阵、真实夹具、性能趋势。
发布前：三平台端到端、安全构建、更新/回滚、长时间离线重连和数据迁移演练。

### 构建与测试速度

- `Cargo.toml` 把依赖（含 Typst）在 dev/test profile 下设为 `opt-level = 2`，工作区自身 crate 保持 0：
  Typst 排版/光栅化在未优化下很慢，`cargo test --workspace` 暖缓存实测约 95 s → 约 22 s，冷重编一次约 150 s。
- CI 的准备步骤（固定工具链、mold、`Swatinem/rust-cache`、apt 依赖）统一在复合 action `.github/actions/setup`；
  缓存只在 `main` 保存，PR 只读取。`rust.yml` 只有两个 job：lint（fmt、clippy、doc、`cargo deny`、仓库门禁脚本）与
  test（一次编译后跑 `cargo test` 和两种语言的真实窗口检查）。fork 与 stock Typst 的像素对照在 `typst-fork.yml`，
  只在 fork、`scholium-typst`、参考实现、锁文件或工具链变动时运行。
  `typst-upstream.yml` 每周一把补丁套到上游 Typst main 上，检查补丁能否应用、adapter 能否编译、像素是否一致，
  只按计划或手动触发，不在 push/PR 上运行、不阻塞合并（见 [scholium-typst 上游跟进](modules/typst.md#上游跟进)）。
  `push` 只在 `main` 触发，PR 只由 `pull_request` 触发，同一 PR 的新推送取消旧运行。
- 所有 workflow 显式固定同一个 runner 镜像 `ubuntu-26.04`，不用 `ubuntu-latest`：镜像升级会同时换掉 Xvfb、xkb-data
  和 xdotool，真实窗口检查对此敏感，换镜像必须是一个单独审查的 PR（经过见
  [ubuntu-26.04 窗口检查记录](log/2026-10-30-ubuntu-2604-window.md)）。
- 真实窗口检查统一由 `python3 scripts/window-checks.py` 运行（自动发现、顺序执行、不重试、机器级锁），
  新增检查无需修改 `rust.yml`，见 `spikes/typst-main-editor/README.md`。
  功能键（F1–F12）组合键按键码发送（`chrome_ui.keycode`，如 `ctrl+67`），不要写 `ctrl+F1`：Ubuntu 的 xdotool 3.20160805
  在 xkeyboard-config ≥ 2.46 下会给功能键 keysym 自动加上 Alt。

## WASM 与跨宿主验证

按[WASM 计划](plan/WASM.md)对纯逻辑 crate 执行 `wasm32-unknown-unknown` 编译与实际运行测试（例如无 npm 的 wasm-bindgen-test 工具路径，具体版本待 spike 固定）。复用原生解析、编辑动作、序列化、Unicode 锚点与兼容报告语料，并测试原生/浏览器 CRDT 更新互通。浏览器另测中文 IME、导入/导出、配额耗尽、异常关闭、Worker 取消与权限撤销后的重连；不得把编译成功等同运行成功，或以浏览器脚本替代原生 UI 测试。当前没有 WASM 实测结果。

## 验证纪律（阶段 0 的教训）

阶段 0 反复出现同一类错误：**看着通过、其实没测**。以下几条已写进流程，不是建议：

1. **任何"框架/库不支持某能力"的判定都要两条独立证据**：静态证据（源码、依赖树、API 全量检索）
   加可复现的实测。只看总数会掩盖"个别用例静默失效"——本项目已经因此错判过三次
   （wgpu 后端、无障碍开关、输入法可用性）。
2. **自动注入不可作为否定证据**：`ydotool` 依赖窗口焦点，在共享桌面会话里会被抢焦点。
   输入契约改用无窗口确定性测试（egui / Iced 各一套 `tests/input.rs`）。
3. **测试自身必须先被对照检验**：至少留一个"必然失败"的负对照，
   证明用例有区分度（例如 result-gate 的朴素实现必须被同一用例抓出画面倒退）。
4. **坐标/时序类断言要验同一行/同一帧**：跨行区间、跨帧状态都曾给出过假数据。
5. **汇总数字必须与明细一致**：报告表头与逐条判定对不上，是最容易骗过人的错误。
