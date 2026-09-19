# Spike 0036：恢复实验编译入口隔离

- 日期：2026-09-19；基线 `a0f9dc6` 加本轮修改。
- 决策：[ADR 0019](../adr/ADR-0019-recovery-worker-isolation.md)。
- 本轮关闭旧 recovery 公开 build 路径的无 OS 隔离缺口；条件 6 仍部分满足，当前状态见 [0012](SPK-0012-exit-criteria.md)。

## 实现与证据

LaTeX、Typst、PDF 页数/文本探针全部移入受限 worker，保留 latexmk 多轮行为与输出排他锁。
禁用 shell escape、latexmk 项目配置和环境继承；只读源码快照、单独输出目录、无网络与进程隔离。
父进程拒绝非法 job/path、过大源码和非普通/超量输出；失败不返回成功结果，超时删除当前入口产物。

| 验证 | 结果 | 证据 |
|---|---|---|
| 原构建/WAL/冲突/随机截断 | 两轮各 15/15，逐用例结果一致 | [recovery.log](evidence/SPK-0036/recovery.log) |
| 真实构建安全正负对照 | 7/7：两引擎良性、两引擎根外读取、shell/配置、两引擎超时 | [security.log](evidence/SPK-0036/security.log) |
| 输入/输出护栏单测 | 3/3：路径/源码、非普通/大小、消息/文件数 | [unit.log](evidence/SPK-0036/unit.log) |
| 运行时清单 | full、pdf-probe、recovery 正负对照 | [mounts.log](evidence/SPK-0036/mounts.log) |
| 共用安全门禁 | 6/6，包含原恶意输入、混合构建双引擎与 PDF 探针 | [security-suite.log](evidence/SPK-0036/security-suite.log) |
| 质量门禁 | recovery 格式、all-target Clippy、licenses/bans/sources；runner 自测 | [clippy.log](evidence/SPK-0036/clippy.log)、[license.log](evidence/SPK-0036/license.log)、[runner.log](evidence/SPK-0036/runner.log) |

两条超时测试先成功构建，再输入高成本源码，要求 worker 返回 124、耗时小于 20 秒且旧产物不存在。
LaTeX 越界测试用宿主真实文件与 EOF 分支成功编译，避免把任意编译失败当成安全通过。
Typst 仍为单文件 World，明确返回 file not found；没有宣称可导入任意项目资源。

## 失败与修正

- 旧子命令白名单漏掉 worker，导致递归进入完整夹具；改为 argv[1] 分发并拒绝未知命令。
- 原 Typst 内部 Hash 跨 worker 不稳定，导致并发/锁释放后的签名比较失败；改为实际逐页 SVG 内容签名，双轮复测通过。
- `#while true {}` 被 Typst 主动拒绝，不能验证墙钟超时；改用已有安全套件的高成本递归夹具。
- 编译已禁用 latexmkrc，但版本查询 `latexmk -v` 仍执行配置，真实生成 RC-LEAK；版本查询也加 `-norc`。
- 全 crate 格式检查暴露旧长函数，抽出基线证据模块；不修改共享 core 的既有格式。

失败日志保留在 [evidence/SPK-0036](evidence/SPK-0036/)，不声称首次运行全通过。

## 复现与剩余范围

```sh
CARGO_HOME="$PWD/spikes/native-ui/.cargo-home" cargo build --offline --release --manifest-path spikes/recovery/Cargo.toml
SCHOLIUM_SPIKE_REPEAT=2 spikes/recovery/target/release/scholium-spike-recovery
spikes/recovery/target/release/scholium-spike-recovery security
bash spikes/tests/runtime-mounts.sh
```

运行时总磁盘/进程配额、可信资源树最小化、独立研究 helper、原生解析与宿主解码仍未关闭。
输出 64 MiB 合计/256 文件是运行结束后的接收检查，不能限制执行期间的总磁盘占用。
当前只有 Linux 的既有环境验证；没有以缩小安全要求的方式升级阶段状态。
