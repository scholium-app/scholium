# Spike 0052：Typst 持久 Content 会话与增量段落验证

- 结论：**Pass（可信正文/分数子集的持久 Content 与失效正确性）**；**Fail（本机接受到画面观测 p95 ≤ 32 ms）**。完整 K3/K4 未验收。
- 对应验证项：[路线图阶段 0 第 1/2 项](../plan/ROADMAP.md)：结构编辑与 Typst 映射。
- 日期：2026-10-01；方向：[ADR 0032](../adr/ADR-0032-typst-edit-kernel.md)、[编辑内核计划](../plan/TYPST_EDIT_KERNEL.md)。
- 前置：[报告 0051](SPK-0051-typst-caret-native-editor.md)。

## 范围与版本

本批把独立窗口的逐键完整 Document 快照、完整 Content 投影替换为已接受语义操作驱动的
只读节点更新及持久 ContentSession。正文、段落、Math、Fraction、Text 保持既有 origin、
包含槽位、Hole 和同源 caret。未修改产品 `crates/`，也没有 flow 分页检查点或 SceneDelta。

Typst 固定 0.15.1 / `9dfd3a08500b7896045f907433cf7b4b02434fad`，基线 Scholium
`8f18f47`；Rust 1.96.0 release、Linux 7.2.7-arch1-1、Intel i7-13650HX，
eframe/egui 0.36.2。字体为 typst-kit embedded + 同机 system 集合，无 CPU 绑核。
源码及补丁 SHA-256 见[测量清单](evidence/SPK-0052/source-manifest.json)。
上游代码/补丁保持 Apache-2.0，新探针代码 MIT OR Apache-2.0；仅本 spike 使用 path patch。

## 会话不变量

唯一可写权威仍是 `spikes/native-ui/core` 的语义图。RenderNode 是已接受节点的只读描述，
不能反向修改模型，也不写入持久化/协作协议。初始/重同步遍历可信可达树；文字编辑仅捕获
一个文本叶，分数包裹捕获旧父节点、存活叶和新建子树。文字撤销从 actor 动作记录取得原叶，
不会因光标已移动而更新错叶。当前窗口没有结构撤销或任意节点删除协议。

Update 携带 base/revision。容量 1 的请求通道满时，UI 合并连续更新的最终节点描述并累计
已接受动作数，保留最早 base 与最新 revision；模型动作不丢失，中间画面可以跳过。
worker 必须顺序应用各包，不能先跳到最后一个增量包。base 不连续时返回类型化错误；UI
收到当前 revision 的 base 错误后从权威模型重新捕获整树。重同步不回退语义版本。

Content 缓存以 `(NodeId, text/math context)` 和节点局部版本为键。修改使新旧祖先链失效；
相同未变节点不失效。填空槽/恢复空槽、结构包裹保留存活叶身份。更新前校验支持种类、
槽位数、variant、引用、父子所有权、重复子节点和祖先环；拒绝时节点/版本/cache 不变。
Content 只负责投影缓存；行宽、字号和 World/style 仍交由 Typst 自身 memoize 参数处理。

像素和整个 Frame 几何仍作为一个 Scene 返回，仅当前语义 revision 能同时采纳纹理和几何。
pending 期间旧图不可命中、不可绘制旧 caret，已提交正文没有 egui 回显。键盘/IME 操作在
egui raw_input_hook 中接受并送 worker，UI 在绘制前再次非阻塞接收；没有 UI 线程布局或等待。

第四个补丁仅在 `editor` feature 下统计 memoized paragraph 实际函数体执行次数，使用
线程局部计数；不参与缓存/失效决策，也不用回调汇集几何。editor 关闭编译仍通过。

## 1/64/256 段首中尾修改

[原始数据](evidence/SPK-0052/benchmark.json)、[断言日志](evidence/SPK-0052/benchmark.log)。
每组 60 次，共 540 次；每次在该叶 UTF-8 byte 0 插入唯一 `x{sample}`，可触发断行变化。
各组前缀包含段落数和位置，避免不同组复用同一 memoize 请求。初始试跑发现跨组相同输入
会产生 0 次实际 paragraph 执行，已改夹具并要求每次实际执行数**恰为 1**。

宽度 420 pt，高度 40000 pt，为单个不分页 flow；时间不含 core.apply、正确性 oracle 或栅格化。
Content 列含更新捕获、验证/失效与 Content 构建；布局/几何列含整个 flow 布局和全 Frame
Geometry 遍历。p95 采用 nearest rank，各列分别计算，不能直接相加成另一列的 p95。

| 段落数 | 修改位置 | Content p95 ms | 布局/几何 p95 ms | 后台合计 p95 ms |
|---|---|---:|---:|---:|
| 1 | 首 | 0.004 | 1.190 | 1.194 |
| 1 | 中 | 0.003 | 1.102 | 1.106 |
| 1 | 尾 | 0.002 | 1.094 | 1.096 |
| 64 | 首 | 0.011 | 1.877 | 1.883 |
| 64 | 中 | 0.021 | 2.098 | 2.108 |
| 64 | 尾 | 0.022 | 2.228 | 2.248 |
| 256 | 首 | 0.040 | 4.227 | 4.261 |
| 256 | 中 | 0.055 | 4.678 | 4.736 |
| 256 | 尾 | 0.050 | 4.520 | 4.564 |

全部 540 次仅收到 1 个节点、构建 3 个 Content（叶/段落/根），实际执行 1 个段落布局；
64/256 段分别复用 63/255 个兄弟段落 Content。支持夹具的布局/几何 p95 ≤ 8 ms **Pass**。
但根 sequence 重建、flow 和全几何仍随全文规模增长，不能据此宣称输入布局与全文解耦。

每次增量 Content 都与保留的无缓存整树投影相等；Frame hash 对照包含打印元素、变换、
编辑框、cluster 和 caret。各组首末两次共 18 次逐像素对照，另有换宽/字号两次像素对照。
这里的整树 oracle 使用同 fork；独立未改 Typst 的九组 stock 对照另行全部为 0 差异像素。
World::source 调用数为 0。宽度 420→260 pt、字号改为 17 pt 时，复用相同 Content 但至少
64 个段落重新实际执行，输出与整树投影一致；未测字体族切换或字体资源 generation 变化。

## 30 秒连续输入与实际窗口

[最终可见帧汇总](evidence/SPK-0052/present/summary.json)、
[300 次样本](evidence/SPK-0052/present/samples.json)、
[Scene 审计日志](evidence/SPK-0052/present/app.log)、
[画面](evidence/SPK-0052/present/last-frame.png)、
[性能命令状态](evidence/SPK-0052/performance-status.json)。

隔离 Xvfb X11，缩放 1、默认 vsync；最终 benchmark 与窗口测量顺序运行，没有并行验证任务。
连续 30.000 秒、300 次/约 10 Hz，交替插入 `q` 与 Backspace，避免文本增长改变负载。
每次语义 revision 前进 1，观测到全部 300 个当前 Scene。窗口负载为既有小正文/分数夹具，
不是 256 段窗口或复杂公式压力测试。

测试开关在同次 pixels/caret 绘制中加 32-bit revision 条码，Pillow 读取 X11 framebuffer
直到条码匹配当前 revision。接受时间在 core 接受后、捕获更新前记录；跨进程使用同机
Unix wall-clock 纳秒。测量含抓屏成本，属于“首次捕获到该帧”的上界，不是物理显示器
scanout 时间；没有确认真实 60 Hz 扫屏，也没有通过禁用 vsync 改变目标。

| 阶段 | 最终 p95 ms |
|---|---:|
| 语义接受 → worker Scene 完成 | 0.391 |
| worker 完成 → UI 采纳（含纹理登记） | 16.420 |
| UI 采纳 → 首次观测到同帧条码 | 23.293 |
| 语义接受 → 首次观测到同帧条码 | **36.411** |
| 自动注入开始 → 首次观测到同帧条码 | 42.088 |

阶段 p95 不可相加。Content/布局/栅格化阶段 p95 分别为 0.035/0.060/0.221 ms。
32 ms 判据 **Fail**，present-check 在写入全部证据后非零退出；功能断言通过不把性能失败
改为通过。尚不能区分所有 GPU 提交、交换缓冲和抓屏等待，需要后续显示链路测量。

原始派发路径 p95 为 [36.935 ms](evidence/SPK-0052/present-baseline/summary.json)，
输入提前后的第一次独立试跑为 [40.249 ms](evidence/SPK-0052/present-early/summary.json)。
另有并行运行探索结果 [38.720 ms](evidence/SPK-0052/present-overlap-summary.json)，不作最终性能证据。
这些数据不足以证明 raw_input_hook 优化降低了可见帧延迟；最终值也仍未达标。

200 ms 人工布局等待的[六组原生操作](evidence/SPK-0052/native/summary.json)全部通过：
空槽填入、嵌套分数、撤销恢复 Hole、pending 旧正文点击被拒绝、中文 byte 边界处插入/移动/
删除/撤销。图像与审计状态保存在同目录；人工延迟的运行不用于性能数据。

## 工程门禁与剩余边界

- [13 项窗口/会话回归](evidence/SPK-0052/native-tests.log)通过；bench 二进制复用的 6 项会话测试也通过。
- [stock/fork 九组图](evidence/SPK-0052/verification.log)、bootstrap 四补丁前缀一致性通过。
- 格式、release [Clippy](evidence/SPK-0052/clippy.log)、[stock Clippy](evidence/SPK-0052/stock-clippy.log)、
  [editor 关闭编译](evidence/SPK-0052/editor-off.log)、[rustdoc](evidence/SPK-0052/rustdoc.log)、Python 语法与 Rust 规模检查通过。
- [许可证/bans/来源](evidence/SPK-0052/deny-policy.log)通过；[完整 advisories](evidence/SPK-0052/deny-advisories.log)
  **Fail**：bincode、paste、rustybuzz、ttf-parser、yaml-rust 停维护，quick-xml RUSTSEC-2026-0194/0195。
  没有加 ignore 或改变政策。这是独立 workspace 检查，未重跑无修改的产品 workspace 或远端 CI。

未完成的 K3：flow 分页检查点、跨页增删和依赖传播、脚注/计数/query、字体资源失效、
复杂数学/超长单段、内存与取消预算；全几何和栅格图仍全量生成，结果通道尚不是有界 SceneDelta。
未完成的 K4：保存重开、模型迁移、选区、协作锚点、实机 IME/无障碍与产品接入。
下一批先定位显示链路与抓屏成本，补 60 Hz 实机证据，再增加 flow 检查点和增量几何。
ADR 0032 保持 Proposed，阶段出口判定与产品旧路径不变。

## 复现

见[探针 README](../../spikes/typst-edit-session/README.md)。`check.py` 默认输出现在为 SPK-0052，
不覆盖 0051 的历史证据。性能 observer 输出目录必须为空；本机 32 ms 门禁失败是可复现结果，
不能把它的非零退出当作未生成证据。
