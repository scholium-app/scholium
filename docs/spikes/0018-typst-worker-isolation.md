# Spike 0018：Typst worker 隔离与运行时白名单

- 日期：2026-09-16；基线 `3be0b0a` 加本轮更改。
- 结论：**Pass（本轮隔离与受影响回归）；阶段 0 条件 6 仍部分满足。**
- 范围：混合构建 Typst 宿主/组件、共享 Linux 沙箱；不涉及生产 workspace。
- 平台：沿用报告 0017 的 Arch Linux、TeX Live 2026、Typst 0.15.1、bubblewrap 0.12.0。
- 决策：[ADR 0012](../adr/0012-typst-worker-isolation.md)；前轮：[报告 0017](0017-mixed-build-isolation.md)。

## 实现

`typst_host::compile` 现在把主源码、显式虚拟文件、编号探针序列化为请求，启动当前受信 driver 的
`--typst-worker`。沙箱子进程完成 World 构造、求值、Frame/Introspector 取证和 PDF/首页 SVG 导出。
主进程只接收文本、编号、页码、链接、诊断和导出字节；不持有 `PagedDocument`。

请求版本为 1，输入/输出位置固定；读取上限 64 MiB，超大消息明确拒绝。
每次创建 0700 临时目录，结束清理；stdout/stderr 重定向文件。
沙箱使用 10 秒墙钟（终止后 2 秒强杀）、4 GiB 地址空间、30 秒 CPU、64 MiB 单文件、128 FD 护栏。
超时返回 `typst-worker-timeout`，其他退出异常返回 `typst-worker-failed`；没有进程内降级路径。

运行时不再整棵挂载 `/usr`、`/lib`：显式工具列表与 `ldd` 解析的共享库逐文件只读挂载。
保留 locale、字体、fontconfig、TeX 资源与配置、纸张配置、TeX 缓存目录。
嵌套沙箱需要的 bwrap、shell、探针和基础文件工具也显式列出；列表见 `spikes/toolchain-sandbox.sh`。

## 有区分度的夹具

- 真正的 `--rebuild` 入口成功产生良性 PDF，并拒绝未声明的 `/etc/hostname`。
- 简单 `while true` 会被 Typst 自己拒绝，普通 Fibonacci 会被缓存加速，均不能证明进程预算。
  最终使用每次递归携带不同 seed 的计算，确认 10 秒超时由内部 worker 上报，外层 20 秒兜底没有触发，
  `final.pdf` 不存在。
- 运行时探针先确认项目可读、输出可写、XeLaTeX 与字体/TeX 资源可用，再确认
  `/usr/share/doc`、`/usr/bin/curl`、`/usr/lib/chromium` 和 `/etc/hostname` 不可见。
  旧整目录挂载下该测试失败，新白名单下通过。
- 初次收紧挂载后良性 TeX 失败：系统 locale 数据缺失导致纸张规格解析异常；
  补入 `/usr/lib/locale` 后恢复。没有以良性启动失败冒充隔离成功。

## 验证

| 检查 | 结果 |
|---|---|
| mixed-build 单元/集成测试 | 10/10，通过 |
| security 分段 | 4/4：21 条原安全检查、3 个 LaTeX 入口测试、3 个 Typst 入口测试、挂载探针 |
| items 分段 | 5/5；混合构建 36/36 |
| packages 分段 | 28/28，含嵌套 worker 与缺组件拒绝对照 |
| preview 分段 | 通过，121 帧 headless p95 2.42 ms；编译及栅格化 994 ms，不含实际呈现 |
| Clippy（所有 targets，warnings 为错误） | 通过 |
| 改动 Rust 文件格式、shell 语法、门禁脚本四场景 | 通过 |

持久证据：[Typst worker](evidence/0018/typst-worker.log)、[挂载探针](evidence/0018/runtime-mounts.log)、
[包重建](evidence/0018/packages.log)。本机完整分段日志：`/tmp/scholium-stage0.nczf4w/`（security）、
`/tmp/scholium-stage0.FZki72/`（items）、`/tmp/scholium-stage0.mTT5Xa/`（packages）、
`/tmp/scholium-stage0.1Pwmlu/`（preview）。

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

`security` 新增 Typst 入口集成测试与挂载探针，共 4 项；完整脚本的检查总数变为 35。
本轮运行受影响分段，不把它写成已重跑全量 35/35。

## 剩余边界

- 10 秒预算和每次新进程适用于当前夹具，未作长文档性能承诺；增量 World 没有跨请求保留。
- 地址空间/单文件限制已配置，未对实际内存耗尽、总输出/进程数攻击逐项验证。
- 可信资源目录仍可读，不是“项目根外所有文件不可读”；其他 OS 未验证。
- 主进程中的输入 JSON/IPC 解析、请求序列化仍有内存开销；字节读取上限不等于整体堆内存上限。
- 宿主侧 PDF 探针、旧 recovery 实验、插件/字体/图片解析器安全审计仍未全部闭合。
- 新 worker 入口是内部固定路径模式，不接受外部任意路径；它不构成独立可供用户绕过沙箱的编译 API。

因此不提升阶段出口判定。原生窗口验收、实际呈现性能与混合内容保真仍按报告 0012 保留。
