# Spike 0015：验证门禁复核

> **有效性（2026-09-17 登记）**：时点门禁复核记录，只作追溯。阶段 0 当前判定见 [报告 0012](SPK-0012-exit-criteria.md)。

> 历史记录：以下保留该轮验证结果。2026-09-16 工作区后续实现与复测见
> [报告 0016](SPK-0016-working-tree-status.md)，当前出口判定见[报告 0012](SPK-0012-exit-criteria.md)。

- 结论：**Pass（本次门禁修复）；阶段 0 尚未通过。** 全量自动检查 29/30，通过数不等于出口条件满足数。
- 日期：2026-09-16
- 范围：复核现有 spike 的自动验证可信度，重点检查混合构建与一键脚本；未进行全部模块的逐行审计。
- 关联：[混合构建](SPK-0010-mixed-build.md)、[出口条件](SPK-0012-exit-criteria.md)、[路线图](../plan/ROADMAP.md)。

## 发现与修复

| 问题 | 复现证据 | 处理 |
|---|---|---|
| 混合构建断言失败仍退出 0 | 清空子进程 PATH 后执行 `E1-equation`，输出 Fail，但旧版本进程成功 | 根据所有夹具的 Evidence 返回退出码；集成测试先红后绿 |
| 未知夹具或脚本分段空跑通过 | 不存在的夹具名、`verify-stage0.sh typo` 都曾退出 0 | 空运行/未知分段明确返回非零 |
| PDF 探针失败被算成零栅格图像 | 原实现忽略 `pdfimages` 非零退出码；启动失败直接返回 0 | 独立 `pdf_evidence` 模块返回 IO Result，工具/输入错误均不能通过矢量性断言 |
| 一键脚本漏报打印型断言失败 | `untrusted-input` 输出 FAIL，但程序退出 0，旧脚本显示 PASS | 同时检查退出码和显式 FAIL 标记；这是对遗留探索程序的过渡兼容 |
| 失败日志不显示、互相覆盖 | `record` 后读 `$?` 读到的是打印成功状态；固定日志每项覆盖 | 保存原始状态，每次运行使用独立目录、每项独立日志；失败输出尾部与完整路径 |
| 证据重复计数 | 混合构建连续两次追加同一个“构建诊断” | 删除重复断言；旧报告的断言条数保留为历史数据 |

安全检查目前仍是探索程序，尚未统一为结构化结果；日志 FAIL 扫描只是防漏报，不能替代逐条安全判据。
同时修正 README 的“只有文档”过期描述，以及报告 0012 第 8 项正文/汇总与页首判定不一致的问题。

## 本机复测

平台：Linux 7.2.4-arch1-2；Rust 1.98.0-nightly（bd08c9e71，2026-06-25）；
Cargo 1.98.0-nightly；XeTeX 0.999998 / TeX Live 2026；混合构建使用锁定的 Typst 0.15.1。
依赖沿用各 spike 的 Cargo.lock，没有新增依赖。

```text
core       1 PASS
ui         5 PASS  （headless 测试与构建，不含重新操作 IME/无障碍桌面）
typst      4 PASS
reconcile  1 PASS
items      5 PASS  （mixed-build 36/36，LaTeX 与 Typst 各 18 次）
security   1 FAIL
license   12 PASS  （licenses / bans / sources，不含 advisories）
web        1 PASS
合计      29 PASS / 1 FAIL；一键脚本退出码 1
```

安全失败与报告 0011 的已知缺口一致：限制配置下仍能读取项目外文件，
沙箱内良性编译报 `xdvipdfmx:fatal: Unrecognized paper format: a4`。
恶意编译失败本身不能证明隔离可用，必须先让相同沙箱中的良性编译通过。
本轮没有修复安全实现，也没有把该项改写成通过。

新增回归：shell 门禁四种场景通过；mixed-build 三个 Rust 回归测试通过；
`cargo clippy --release --offline --all-targets -- -D warnings` 通过。
修改的 Rust 文件经 rustfmt 检查；整个 mixed-build 的 `cargo fmt --check` 仍存在其他文件的既有格式差异，
本轮未作无关的整 crate 格式重排。

## 复现

```bash
export CARGO_HOME="$PWD/spikes/native-ui/.cargo-home"
bash spikes/tests/verify-stage0.sh
cargo test --release --offline --manifest-path spikes/mixed-build/Cargo.toml
cargo clippy --release --offline --all-targets --manifest-path spikes/mixed-build/Cargo.toml -- -D warnings
bash spikes/verify-stage0.sh all
```

最后一条在本报告记录的版本中**应失败**，并把完整日志目录打印出来。本次全量日志位于本机
`/tmp/scholium-stage0.275kTE/`，其中 `16.log` 是混合构建，`17.log` 是安全检查；
临时日志可能被系统清理，长期复核请使用上述命令重新生成。

## 下一步优先级

1. 修复 LaTeX 沙箱的良性编译；再验证项目外读取、越界写入、外部命令与资源护栏，不能靠编译器启动失败充当隔离成功。
2. 补标准目标包/混合重建包的干净环境构建，关闭出口条件 10 的可交付性缺口。
3. 补跨节点选区与源码工作台编辑闭环，以及异步预览的实际帧阻塞证据。

现有实验可以保留；支持范围仍以各项报告为准，本轮没有搭建正式 workspace。
