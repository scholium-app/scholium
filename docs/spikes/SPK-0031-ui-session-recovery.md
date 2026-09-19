# Spike 0031：窗口实验会话与草稿恢复

- 日期：2026-09-19；基线 `8dd2749` 加本轮实现。
- 结论：**Pass（限定实验会话）**，阶段判定见 [0012](SPK-0012-exit-criteria.md)。
- Linux / Wayland / niri，egui 0.36.2，真实 AT-SPI、键盘输入。
- 依据：[UI 计划“文件恢复”](../plan/NATIVE_UI_VALIDATION.md)、[ADR 0015](../adr/ADR-0015-ui-session-recovery-spike.md)。

## 实现

启动打开指定实验文件，窗口保存、自动 checkpoint、重开、重启恢复。SDG 和源码草稿分别保留，
IO/重建在后台，同步成功才显示持久化。错误保留当前内容；晚到加载不能覆盖新编辑。
正常关闭等待保存，失败提供放弃入口。恢复重建渲染 worker 和本地撤销范围。
实验文件不含共享身份/历史，只保留当前语言草稿与基文，过期草稿仍拒绝提交。

## 真实窗口

| 场景 | 结果 | 证据 |
|---|---|---|
| 正文输入、显式保存、窗口重开、错误 LaTeX 草稿自动保存、SIGKILL、重启恢复 | Pass | [结果](evidence/SPK-0031/recheck/results.json) |
| 外部修改后继续编辑，冲突阻止覆盖；重启打开损坏文件仍能编辑且保留磁盘字节 | Pass | [状态](evidence/SPK-0031/recheck/session-recovered.json) |
| Typst 错误草稿、正文更新使基文过期、正常关闭、重启后提交拒绝且两份内容保留 | Pass | [结果](evidence/SPK-0031/typst-recheck/results.json) |

人工核对 [LaTeX 冲突截图](evidence/SPK-0031/recheck/session-recovered.png)与
[Typst 过期草稿截图](evidence/SPK-0031/typst-recheck/session-typst-stale-recovered.png)。
首轮 LaTeX 因窗口失焦被输入保护中止，保留 [first](evidence/SPK-0031/first/results.json)。
Typst 首轮取证时 AT-SPI 节点失效，保留 [typst-first](evidence/SPK-0031/typst-first/results.json)；
改为等待 checkpoint 稳定再取证，复测通过，没有移除焦点保护。

## 检查与边界

完整默认候选测试 **42 项通过、11 项 ignored**，新增恢复 5 项，见 [日志](evidence/SPK-0031/tests.log)。
覆盖内容/错误草稿往返、过期提交、锁冲突、外部修改/损坏不覆盖、临时文件冲突、无效 schema/
父节点/槽位/变体和晚到打开结果。Clippy 全 targets、格式、licenses/bans/sources 通过；
许可仅有未用白名单告警，不含 advisories。首次许可命令缓存路径错误，改绝对路径后通过。
ignored 为独立沙箱/编译器集成和十万行基准，本轮未重跑全阶段套件。

强杀在持久化确认之后，未确认输入不保证恢复。正常关闭场景也先等 checkpoint；关闭期间写失败、
慢盘、磁盘满、每个 fsync/rename 边界断电和跨平台未验收。外部写者最终比较到 rename 的竞态未解决。
文件选择器、标准源码导入、正式项目格式、多文件资源、历史身份恢复不在本轮范围。

## 运行

仓库根目录运行（父目录须存在）：

```sh
SCHOLIUM_SPIKE_SESSION=/tmp/scholium-demo.session.json \
  spikes/native-ui/candidate-egui/target/debug/scholium-spike-egui
```

再次使用同一路径恢复；顶栏显示持久化状态。不设置变量维持原候选行为。

```sh
SCHOLIUM_TYPST_BIN=/tmp/scholium-typst-toolchain/bin/typst \
  /usr/bin/python spikes/native-ui/candidate-egui/scripts/native-edit-acceptance.py \
  /tmp/scholium-session-acceptance session_recovery session_typst_recovery
```
