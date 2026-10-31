# LaTeX 工作区计划

> 计划文档（将来时）：定义要做什么、什么算通过，不记录进度。依据
> [ADR 0041](../adr/ADR-0041-separate-latex-typst-kernels.md)；产品要求见 [产品定义](../PRODUCT.md#2-项目语言与工作形态)，
> 排版路径见 [排版与转换](../RENDERING_CONVERSION.md#latex-项目)。

## 目标

LaTeX 项目以 `.tex` 源码为权威，在 Scholium 内获得与命令行构建逐字形一致的实时预览：
LuaLaTeX 项目按段落毫秒级重排，其他引擎以后台整编作为预览。LaTeX 内核独立于 Typst 内核，只共享窗口、
命令注册表、界面文字目录、storage、history、collab、build 沙箱与诊断面板。

## 首发范围

- 打开单文件与多文件项目（`\input`、`\include`、`\subfile`），按 [项目语言规则](../MIXED_SOURCE_EDITING.md#3-项目语言的确定) 确定入口与引擎。
- 源码工作台：编辑、语法高亮、诊断、补全、保存；语法错误与缺工具链不阻止保存。
- 同引擎预览：LuaLaTeX 实时段落更新 + 后台整编；pdfLaTeX/XeLaTeX 整编预览。
- 源码 ↔ 页面双向定位；构建日志诊断回链到源码位置。
- PDF 导出与普通命令行构建一致。
- 实时引擎与整编全部运行在 OS 沙箱内。

不在首发：页面内直接编辑正文、LaTeX 的结构工具栏与结构数学编辑、pdfLaTeX/XeLaTeX 的实时段落路径。

## 结构

```text
latex 内核
├── session      项目文件集、入口与引擎识别、源码 revision
├── live         实时预览适配层：段落更新 / 页面更新 / 定位 / 诊断事件（rtex 候选，可替换）
├── build        整编与导出，复用 build 模块的沙箱与运行时白名单
└── view         display list → 纹理；无法用 display list 描述的页面以 PDF 栅格化回退
```

`live` 对上只暴露事件，不泄漏 rtex 类型；更换实现时其余部分不改。首选以 `rtex serve`（JSON lines）
子进程接入，使 rtex 与 LuaLaTeX 整体处于沙箱内，宿主只解析受限消息；进程内链接 `rtex-core` 仅在评估证明
沙箱可等价实现后考虑。

## L0：实时预览引擎评估（选型前置）

已完成，由 [ADR 0043](../adr/ADR-0043-rtex-live-preview.md) 裁决：rtex 作为沙箱内子进程采用，附带宿主兜底条件。
两轮证据见[第一轮](../log/2026-10-30-rtex-evaluation.md)与[第二轮](../log/2026-10-30-rtex-evaluation-2.md)日志。

rtex 是第三方核心选型，须先评估再立 ADR。证据写入 `docs/log/YYYY-MM-DD-rtex-evaluation.md`，
原始数据放 `docs/log/evidence/rtex-evaluation/`。逐项判定：

| # | 评估项 | 通过判据 |
|---|---|---|
| E1 | 许可与依赖 | 锁定版本通过 `cargo deny`；原生依赖登记；TeX Live 仍为用户安装的独立子进程 |
| E2 | 沙箱 | `rtex serve` + LuaLaTeX 在现有 OS 沙箱与固定运行时白名单内正常工作；负对照：Lua `io.open` 读白名单外文件、`\input` 绝对路径、shell escape、写出构建目录外均失败 |
| E3 | 中文 | ctex（LuaLaTeX）与 luatexja 夹具：实时单元占比、单段 p50/p95 时延、`rtex verify` 逐字形一致 |
| E4 | 绘制 | display list 按不低于 2 倍像素密度栅格化，与 PDF 栅格未匹配墨迹 ≤ 1%（150 dpi 下两个参考渲染器自身就相差 1.3–1.7%，不作为判据）；含 TikZ 的页面经 PDF 回退正确显示。PDF 栅格化候选为 hayro（MIT/Apache-2.0，纯 Rust）与 pdfium（原生依赖，须登记）；MuPDF 为 AGPL，排除 |
| E5 | 定位 | 页面点击 → 源码字节、源码光标 → 页面位置的粒度与误差；不足字形级时说明可支撑的交互 |
| E6 | 端到端时延 | 10 / 100 页夹具在真实窗口内从击键到画面更新的 p50/p95，与 rtex 自报数字分开记录；输入不被阻塞 |
| E7 | 非 LuaLaTeX 回退 | XeLaTeX 夹具的整编预览时延、旧 revision 门与“整编预览”标识 |
| E8 | 故障 | 引擎崩溃、挂起、超时、导言区错误时编辑不受影响，单元退回整编并给出原因 |

E2 不通过则不得接入，退回自研或纯整编预览；E3 不通过时中文 LaTeX 项目首发只提供整编预览并在产品文档说明。
全部判定后新立 ADR 裁决选型；评估不改变 ADR 0041 的拆分决定。

## L1：打开—编辑—预览—导出闭环

依赖 [ADR 0043](../adr/ADR-0043-rtex-live-preview.md)。交付 session、build、live、view 的最小实现与工作区装配，并实现 ADR 0043 的接入条件：

- 产品级沙箱：会话级 cgroup（内存、进程数）、常驻进程不设墙钟与 `RLIMIT_CPU`、单次编译由宿主计时；
- 每用户字体缓存的可信预热与每会话可写副本；
- 后台编译看门狗（超时即终止并标为失败）、会话心跳与重启、`EngineState Restarting/Failed` 后重新请求布局；
- 编辑转发前校验（UTF-8 字符边界、不越界、项目内规范化相对路径）；
- 二进制 display list 通道（上游未提供时经 fork，见 ADR 0043），按不低于 2 倍像素密度绘制，TikZ 等页面用 hayro 回退。

出口：

- 打开中英文混排的 LuaLaTeX 论文夹具，编辑正文与公式时预览按段落更新，停止输入后整编更新整页。
- XeLaTeX 夹具显示整编预览并标明来源。
- 保存、强杀恢复、重开后源码与预览一致；PDF 导出与命令行构建一致。
- 沙箱负对照在产品路径上复测通过；E6 在真实窗口内复测。
- 宿主兜底的每一条都有故障注入测试：引擎被杀、实时与后台挂起、导言区缺包、畸形偏移与路径、`rtex serve` 被杀。

## L2：定位、诊断与协作

出口：

- 页面点击跳转源码、源码光标在页面上高亮，误差在 E5 声明范围内。
- 构建错误与警告进入诊断面板并回链源码；实时路径退回整编的原因可见。
- 两位成员同时编辑同一 LaTeX 项目，CRDT 收敛，预览各自跟随本地 revision。

## L3：页面内编辑（视评估决定）

在 E5 证明定位粒度足够后，单独评估页面内直接编辑正文的可行性与范围，结论写入新 ADR。
不承诺与 Typst 原生结构项目同等的结构编辑能力。

## 与路线图的关系

L0 可与 Typst 内核的阶段 1 并行；L1–L2 归入[路线图](ROADMAP.md)阶段 3。团队协作产品化（阶段 5）同时覆盖两个内核。
