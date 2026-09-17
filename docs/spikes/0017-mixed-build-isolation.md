# Spike 0017：混合构建的 LaTeX 入口隔离

> **有效性（2026-09-17 登记）**：出口条件 6 的过程证据，只作追溯；后续 Typst 侧见 [报告 0018](0018-typst-worker-isolation.md)。阶段 0 当前判定见 [报告 0012](0012-exit-criteria.md)。

- 结论：**Pass（LaTeX 共用入口与相关回归）；阶段 0 条件 6 仍部分满足。**
- 日期：2026-09-16；基线 `ad38a0d` 加本轮修改。
- 平台与工具链沿用[报告 0016](0016-working-tree-status.md)：Linux、bubblewrap 0.12.0、TeX Live 2026、Typst 0.15.1。
- 对应：路线图第 7 项及出口条件 6；遵循 [ADR 0008](../adr/0008-reconcile-and-toolchain-isolation.md)，不新增产品架构或正式文件格式。

## 复现问题与修复

直接调用 `mixed-build` 的宿主/组件共用 `latex::compile` 时，旧实现只套 `timeout`，没有 OS 隔离。
新增回归在修复前确认：生成的测试 PDF 含项目外合成诱饵 `PRIVATECANARY`；失败编译仍留下旧 `main.pdf`。
这说明包重建外围沙箱通过，不能证明普通构建入口已受保护。

现在共用入口执行以下流程：

1. 验证入口名，清除该入口旧 PDF；任何错误都返回失败，没有无沙箱降级。
2. 创建权限 0700 的临时目录，快照平铺的 `.tex/.pdf/.aux/.out` 常规文件，拒绝符号链接输入。
3. 使用统一 `toolchain-sandbox.sh` 配方，只读输入映射为 `/project`，新输出为 `/work`；
   固定 `/usr/bin/xelatex -no-shell-escape`，清空外部环境，限制时间和资源。
4. 成功后复制 PDF 与引用辅助文件；失败只保留日志，不复制失败 PDF。临时目录退出时清理。

配方以 `include_str!` 编入 driver，混合重建包无需读取仓库脚本。
包外围已有沙箱时，内部入口仍运行嵌套沙箱；没有环境变量式的绕过开关。
统一 profile 设置 `TEXINPUTS`/`TEXPICTS` 为 `/project:`，保留末尾默认系统搜索路径，
让只读源码、矢量组件及多轮辅助文件正常解析。

## 验证结果

| 检查 | 结果 |
|---|---|
| 共用编译入口回归 | 3/3：良性 include + 越界读取拒绝 + shell 无副作用；失败移除旧 PDF；符号链接输入拒绝 |
| mixed-build 单元/集成测试（最终版本） | 7/7，含 3 个入口隔离回归 |
| `items` 分段 | 5/5；mixed-build 36/36 宿主运行通过 |
| `packages` 分段 | 28/28 包重建，含嵌套沙箱及两宿主缺组件拒绝对照 |
| `security` 分段 | 2/2：既有 21 条安全判据 + 新增共用入口 3 个回归 |
| `preview` 分段 | 通过；headless UI p95 2.62 ms，编译及栅格化 606 ms，不含真实窗口呈现 |
| `cargo clippy --release --offline --all-targets -- -D warnings` | 通过 |
| shell 门禁四场景、shell 语法与改动文件 rustfmt | 通过 |

关键原始日志：[入口回归](evidence/0017/compiler-isolation.log)、[包重建](evidence/0017/packages.log)。
本机其他日志：`/tmp/scholium-stage0.x6IBu6/`（items）、`/tmp/scholium-stage0.0g2ZoJ/`（security）。
`packages` 在后续仅临时目录权限/函数拆分及符号链接测试调整前通过；核心编译流程一致。
本轮按影响范围运行分段，没有声称重新获得全量 33/33；脚本因新增 security 检查，全部运行时现在有 33 个检查项。

## 入口覆盖与剩余边界

| 入口 | 当前边界 |
|---|---|
| mixed-build 普通 LaTeX 宿主/组件 | 本轮统一沙箱，共用入口回归覆盖 |
| mixed-build 混合包重建 | 外围沙箱 + 内部 LaTeX 嵌套沙箱，28 包回归覆盖 |
| 标准包验证 | 固定命令在外围沙箱；交付包离开本验证器后由用户工具链执行 |
| egui Typst 后台预览 | 原有 CLI 沙箱，沿用统一脚本 |
| mixed-build 进程内 Typst | `World` 显式资源表；普通运行仍无独立 OS 进程的时间/内存边界 |
| recovery 旧恢复实验 | 仍是受控生成夹具的直接 latexmk 子进程，不是可接受任意不可信项目的安全入口 |
| 安全测试负对照 | 有意在沙箱外编译固定合成文档；不可复用于用户输入 |

只读运行时仍包含 `/usr`、动态库、字体/TeX/纸张配置与 `/var/lib/texmf`，不是最小文件白名单。
没有验证解析器漏洞、seccomp、总输出配额、进程数耗尽、所有平台或宿主侧 PDF 探针隔离。
输入快照仅支持当前平铺夹具，不承诺任意目录资源树；辅助文件属于不可信源码，同样放进沙箱处理。
因此不把条件 6 升为“满足”，也不把该 spike 当生产沙箱。

## 复现与下一步

```bash
export CARGO_HOME="$PWD/spikes/native-ui/.cargo-home"
cargo test --release --offline --manifest-path spikes/mixed-build/Cargo.toml
cargo clippy --release --offline --all-targets --manifest-path spikes/mixed-build/Cargo.toml -- -D warnings
bash spikes/verify-stage0.sh security
bash spikes/verify-stage0.sh items
bash spikes/verify-stage0.sh packages
bash spikes/verify-stage0.sh preview
bash spikes/tests/verify-stage0.sh
```

下一步继续最小运行时白名单和 Typst 进程隔离；原生窗口验收独立推进。
本轮只检查了桌面验收前提：AT-SPI 的 `IsEnabled`、`ScreenReaderEnabled` 均为 false，未修改用户会话设置。
完整输入法/无障碍共同验收和实际窗口呈现性能未重跑，不能以 headless 回归替代。
