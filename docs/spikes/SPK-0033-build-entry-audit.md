# Spike 0033：构建入口审查与 PDF 白名单收紧

- 日期：2026-09-19；基线 `70fd27d` 加本轮修改。
- 结论：本轮沙箱回归通过，**条件 6 仍部分满足**；当前状态见 [0012](SPK-0012-exit-criteria.md)。
- 决策：[ADR 0017](../adr/ADR-0017-pdf-probe-runtime.md)。

## 入口清单（代码调用链审查）

| 入口 | 实际边界 | 证据/限制 |
|---|---|---|
| mixed-build LaTeX 宿主/组件/重建 | `latex/sandbox.rs` → 固定 XeLaTeX argv → full 沙箱 | 无 shell escape、只读输入；本轮真实入口 3 项回归 |
| mixed-build Typst 宿主/组件/重建 | `typst_host/worker.rs` → 受信 driver 的固定 worker → full 沙箱 | 编译/内省/PDF/SVG 在子进程；本轮良性、越界读、超时 3 项回归 |
| 导出包干净重建 | `packages/clean.rs` → full 沙箱，再进入上述 worker | 工具由 harness 提供，不能由包替换；本轮未重跑 28 包全套 |
| PDF 信息/文本/链接/图像探针 | `pdf_sandbox.rs` → 固定 argv → **pdf-probe 沙箱** | 本轮收紧清单并加入 security 门禁 |
| egui 左侧编辑 | `typst_editor/session.rs` → typst-editor 沙箱 → editor-stream | 受监督长驻 helper；前序报告 0028/0029 的超时/退出证据，本轮调用链复核 |
| egui 右侧预览 | `preview/worker.rs` → Typst CLI compile/query → full 沙箱 | 无进程内降级；仍使用较宽清单，本轮未重跑窗口场景 |
| typst-mapping 默认/性能/几何探针 | 进程内 compile | 受信固定夹具；不能作为不可信项目入口使用 |
| typst-mapping editor-export/editor-stream | helper 内进程内 compile，依赖调用方启动 OS 沙箱 | 直接手动运行 helper 不获得沙箱，不能宣传为独立安全编译 API |
| recovery 旧实验 | 进程内 Typst、直接 latexmk/pdfinfo | 仅受信夹具；命名为 sandbox 的模块只是暂存目录，**不是 OS 安全隔离** |
| untrusted-input 安全实验 | 外层 sandbox 对照 + 受限 World 测试 | 21 项夹具，不能用 World 文件检查替代 OS 计算预算 |

审查范围为当前阶段 0 编译与 PDF 产物处理调用链，不是全仓依赖漏洞审计。
内部 worker 路径与二进制是受信 harness 配置；不把手动绕开父进程的调用算作已隔离入口。

## 运行时可见资源

| 清单 | 可执行工具 | 只读资源 |
|---|---|---|
| full | 固定 TeX/Poppler、shell、嵌套沙箱及基础工具列表 | 动态库、locale、字体/fontconfig、TeX 配置/资源/缓存、纸张配置 |
| typst-editor | prlimit；受信 helper 挂 `/toolchain` | 动态库、字体/fontconfig |
| pdf-probe | sh、prlimit、四个 Poppler 探针 | 动态库、字体/fontconfig、Poppler 资源 |

精确列表以 `spikes/toolchain-sandbox.sh` 为准。可选 `/toolchain` 只接受受信调用方工具，
`ldd` 也只针对这些工具，不能用于项目上传的二进制。资源目录仍是整目录可见。
共同策略为隔离网络/进程命名空间、清空环境、无 capabilities、只读 `/project`、可写 `/work` 和临时目录。
这允许声明的根外运行时资源，不满足“根外所有文件不可读”的字面绝对要求。

## 本轮验证

- security **5/5**：原安全 21 项、LaTeX 入口 3 项、Typst 入口 3 项、PDF 探针 2 项及双清单挂载验证。
- 混合构建 **36/36**：LaTeX/Typst 各 18 次，覆盖正常产物探针和预期阻断。
- PDF 正对照由四个工具分别读取；拒绝畸形、符号链接与超大文件。挂载对照同时确认正常读写路径可用，
  TeX/curl/宿主 hostname/无关目录不可见，宿主环境不泄漏，输入写失败。
- 验收 runner 四场景自测、shell 语法、修改的 Rust 文件格式通过。
- Clippy 首次检查碰到全 crate 格式化引起的旧长函数诊断；撤回无关格式化后复测通过，保留两份日志。

原始证据在 [evidence/SPK-0033](evidence/SPK-0033/)。复现：

```sh
SCHOLIUM_TYPST_BIN=/tmp/scholium-typst-toolchain/bin/typst bash spikes/verify-stage0.sh security
bash spikes/tests/verify-stage0.sh
```

## 未关闭的风险

旧 recovery 与独立研究 helper 的受信使用边界、全资源树挂载、总磁盘/进程配额、解析器漏洞、
IPC/图片解码的宿主内存以及跨平台隔离仍需处理。没有将 scope 缩小后把原安全出口改判满足。
