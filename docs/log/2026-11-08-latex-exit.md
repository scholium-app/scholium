# T50：LaTeX L1 真实窗口验收

本次在 scholium-wt2、origin/main `38544dd` 上实现了专用真实 rtex 验收，而非复用缺工具链的窗口结果。
所用 rtex bundle 来自 PIN `531e2c6b32a09d33be58f6be1301fe8f793411cd`，二进制 SHA-256
`a46184931e71cd0e95babd924b6fa452e78708dfafbfeb3cc01fc0441d7088ca`，278 个文件。
所有窗口均经 scripts/window-checks.py 的机器锁与私有 Xvfb 执行，没有接触用户桌面。

## 发现与修复

- 首次真实三页混排论文验证中，正文段落更新后没有静止整编；worker 新增最后输入静止 750 ms 后的布局请求。
- LuaLaTeX 整页结果被标成实时；标签改为读取实际采纳的 paragraph/full 来源，XeLaTeX 保持整编及耗时标签。
- 真 rtex SIGKILL 后已经恢复，但 worker 丢弃生命周期，或 UI 同一轮排空 Recovering/Ready，重启标签没有显示；
  转发生命周期，并将 Recovering 留到一帧后继续消费队列。新增排队 Ready 回归。
- 无限 Lua 循环触发上游 live watchdog，并由后台 30 s pass 预算明确停止；app 保留错误但没有 `pass_timeout` code。
  适配器对该 pin 的精确无文件错误 envelope 加入 `pass_timeout` code。
  修复循环后 host 接受新 revision，但 pin 没有发布新 LayoutUpdate；新增先失败的假进程回归，
  让确认超时后的下一次编辑更换整个 sandboxed rtex generation 并重同步源码，沿用原重启预算，避免重复编译未修复的死循环。
- A4 100 页不能分配全部 300 dpi 栅格；Renderer 保留有界矢量，app 只栅格/上传当前页。
  原全页 API 的 9600 万像素限制、单页尺寸、256 页、PDF/协议字节预算均保留。新增 100 页/末页/中间页回归。
- 最初恢复比较在提交 audit 后立刻读取屏幕，读到了呈现前的帧；窗口检查改为读取稳定 200 ms 的实际前缓冲。
  短暂状态按完整 audit 事件序列判定，避免只读最新状态遗漏已经呈现的重启帧。

- 常规 source-preview-layout 检查使用了已不存在的 SCHOLIUM_RTEX_BINARY，装好真实 bundle 后无法进入它要检查的缺工具链状态；
  改为产品实际读取的 BIN/TEXDIR 两个开发覆盖，保持原有不可用状态、居中布局和页面模式断言。

## 逐条出口证据

| 条件 | 自动检查 | 结果 | 数字/证据 |
|---|---|---|---|
| 中英 LuaLaTeX、章节/行内及独立公式/图、至少三页 | lualatex-open/edit；host revision + paragraph/full + 像素改变 | 双语通过 | 两引擎均 3 页 |
| XeLaTeX 整编、来源与延迟 | xelatex-open/edit；FullCompilation、Full build、elapsed_ms | 双语通过 | 首次 Xe 整编：中文 890.1 ms、英文 906.0 ms |
| 两种引擎 Save—SIGKILL—重开 | source 字节、dirty、稳定前缓冲 SHA-256 | 双语通过 | 两引擎源码字节相等、稳定预览 SHA-256 相等 |
| 产品 Sandbox E2 | lua-read、absolute-input、lua-shell、write18、lua-write、tex-write、project-write | 双语通过 | 每种语言 7/7，匹配对应探针 error 诊断 |
| E6：10 页 | XGetImage 按键到预览；真实忙碌 TeX 下逐键接受/UI 帧 | 双语通过 | 中文 p50/p95 129.5/249.8 ms；英文 133.6/162.4 ms |
| E6：100 页 | 同上，独立样本与真实页数 | 双语通过 | 中文 p50/p95 253.1/365.1 ms；英文 195.2/224.7 ms |
| ADR 0043 app 故障可达 | rtex-kill-recovery、hung-background-pass 与修复 | 双语通过 | Recovering 可见，generation 增加；pass_timeout 后修复取得新整编 |

write18 的引擎行为是禁用 shell 后静默拒绝，因此负对照在同一文档用 Lua assert 检查 marker 未生成，
将拒绝明确变成 app 诊断；其它 Lua 探针同样检查真实 io/os 返回值。不是在宿主假造诊断或替换引擎。
所有探针同时要求 app 存活、外部 secret 不变、外部写目标及只读项目目标不存在。

E6 从原生按键分派开始，到独立 XGetImage 前缓冲首次预览像素改变为止，2 ms 轮询，且要求相应
host revision 的段落更新。它不使用 rtex 自报时间或 submitted_ms 作为呈现时间。
同一 revision 的上游 `timing.total_us` 另存为 `rtex_paragraph_ms`；布局的 `wall_ms` 经独立
`latex-engine-timing` audit 保留，不混入窗口呈现延迟。
另记录原生输入接受、latex-frame 的工作台 CPU 部分 work_ms 与帧起点间隔 interval_ms，以及真实 TeX 子进程 CPU tick 增量；无限 Lua 循环期间持续输入，
随后修复并重新取得整编。只以输入未被编译阻塞作为门禁，不设置性能优劣阈值。
CLOCK_BOOTTIME 与 MONOTONIC 差值变化超过 1 s 时丢弃测量。

每格均为 ms；每个组合 12 个独立按键，先等上一版整编与屏幕稳定，再开始下一次采样。

| 语言/页数 | 窗口 p50/p95 | rtex 自报 p50/p95 | 忙时输入最大 | workspace work 最大 | 帧间隔最大 |
|---|---:|---:|---:|---:|---:|
| 中文 10 | 129.5 / 249.8 | 2.872 / 4.899 | 27.8 | 12.6 | 28.3 |
| 中文 100 | 253.1 / 365.1 | 4.017 / 6.388 | 90.7 | 48.4 | 62.2 |
| 英文 10 | 133.6 / 162.4 | 3.152 / 4.102 | 26.4 | 13.4 | 29.3 |
| 英文 100 | 195.2 / 224.7 | 2.504 / 3.833 | 100.2 | 50.5 | 63.2 |

忙时 TeX CPU tick 分别从 143→251、245→333、133→240、231→336，证明输入采样期间真实 TeX 在工作。
所有测量 BOOTTIME/MONOTONIC 差值变化均小于 1 s。这里是本机 debug app、优化依赖、软件 GL/private Xvfb
的窗口测量，不据此声称跨机器性能保证。原始样本和帧数组在双语 JSON；压缩 audit 保留 revision、来源、
generation、诊断及引擎自报耗时，不提交重复源码。重启的实际本地化 chrome 标签另存
[restarting-labels.json](evidence/2026-11-08-latex-exit/restarting-labels.json)。

截图：[Lua 中文](evidence/2026-11-08-latex-exit/lualatex-zh-Hans.png)、
[Xe 中文](evidence/2026-11-08-latex-exit/xelatex-zh-Hans.png)、
[Lua 英文](evidence/2026-11-08-latex-exit/lualatex-en.png)、
[Xe 英文](evidence/2026-11-08-latex-exit/xelatex-en.png)。
审计：[中文](evidence/2026-11-08-latex-exit/audit-zh-Hans.jsonl.gz)、
[英文](evidence/2026-11-08-latex-exit/audit-en.jsonl.gz)。


## 门禁与交付

Rust 与仓库门禁逐项通过：fmt、locked workspace/all-targets clippy（-D warnings）、locked workspace test
（747 passed / 16 native ignored）、RUSTDOCFLAGS=-D warnings 的 locked workspace doc、Rust size、doc links、
UI strings、layers、locked cargo-deny licenses/bans/sources、locked app/storage bins build、bundle 三项单测与 diff check。
命令与退出码见 [gates.json](evidence/2026-11-08-latex-exit/gates.json)。
Typst fork 与独立 stock：系统字体 31/31、bundled 字体 31/31 像素对照通过；upstream objects 来源重建验证通过，
见 [来源记录](evidence/2026-11-08-latex-exit/typst-fork-source.json)。没有修改 Typst adapter 或 reference。

常规双语窗口初次 63 pass / 1 fail / 2 skip：唯一失败为上面记录的 source-preview-layout 中文旧覆盖名；
英文在修复后通过。目标双语重跑 source-preview-layout 2/2 通过（中文 34.4 s、英文 34.6 s），
覆盖常规批次的唯一失败。两者合并覆盖 64 个实际常规窗口场景，2 个专项 skip 符合任务约定。
专用真实引擎双语 2/2 通过，每种语言 17 个验收断言组（中文 219.0 s、英文 220.1 s）。
见 [专项汇总](evidence/2026-11-08-latex-exit/window-exit.txt)、
[中文测量](evidence/2026-11-08-latex-exit/exit-zh-Hans.json)和[英文测量](evidence/2026-11-08-latex-exit/exit-en.json)。
原始汇总见 [window-main.txt](evidence/2026-11-08-latex-exit/window-main.txt)，早期回归见
[initial-failures.json](evidence/2026-11-08-latex-exit/initial-failures.json)。

真实内核定向回归 `native_capability_probe_and_supported_upstream_pass_timeout --ignored --exact` 通过，
确认 timeout 后下一次编辑启动 generation 1，revision 2 取得 Complete layout；
见 [native-timeout.txt](evidence/2026-11-08-latex-exit/native-timeout.txt)。

没有修改 T49 的 full.rs/command/file.rs，或 T51 的 build launch/sandbox 与 source_editor。
没有新增 ADR：使用既有内核、Sandbox 和公开诊断边界，未更换第三方引擎或持久格式。
专用 latex-exit.yml 固定 ubuntu-26.04，workflow_dispatch / 相关路径 PR 触发；常规窗口批次明确跳过，
显式 --only latex-exit 会运行真实验收。没有为发现式窗口检查添加常规 workflow 的逐项步骤。

全部六条出口均完成本地自动验收；没有剩余实现项。专用 CI workflow 尚未运行，不轮询 CI；不创建 PR，由协调者起草。
没有偏离验收要求；额外修改 source-preview-layout 仅为修复真实 bundle 暴露的过期工具链覆盖名。

## 合并后 CI 的间歇失败（2026-11-09）

合并 main 后专用 workflow 一次在英文 `formula edit did not alter preview pixels` 失败，同一提交重跑双语通过。
CI 日志中 revision 2 的段落更新与整编均已呈现（`shown_revision` = 2），截图中公式位于采样区域内；
失败原因是采样在新纹理所在帧呈现前就取到了 200 ms 稳定的旧帧。
`settled_pixels(unlike=…)` 对“应当改变”的两处断言等待与之前不同的稳定帧（最长 15 s），超时仍返回稳定帧并按原断言失败；
断言与阈值不变。本机真实 rtex 双语 2/2 通过。
