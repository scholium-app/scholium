# Spike 0050：Typst 数学身份与空槽内核补丁

- 结论：**Pass（分数身份与空槽限定子集）**；完整 K1、K2 和产品编辑路径尚未验收。
- 对应验证项：[路线图阶段 0 第 1/2 项](../plan/ROADMAP.md)：结构编辑与 Typst 映射。
- 日期：2026-09-30；执行者：Codex，用户指定深改 Typst，并授权评估旧分支后继续操作。
- 方向：[ADR 0032](../adr/ADR-0032-typst-edit-kernel.md)、[编辑内核计划](../plan/TYPST_EDIT_KERNEL.md)。
- 前置：[公开 Content 入口验证](SPK-0049-typst-content-layout.md)。

## 问题与动手前判据

不用源码 span 或可打印占位符，能否让 Typst 自身的数学布局输出节点/槽位身份与空槽几何，
并保持完整公式的原有排版？判据预先写入独立探针 README：

1. 分子/分母稳定身份、非零空槽与中心命中；填入后 Hole 状态消失。
2. 空槽无额外字形/图形；重复布局一致，修改分母与同文本不同 ID 不串缓存。
3. 嵌套分数、平移与缩放后的身份/坐标正确。
4. strict 布局拒绝未填 Hole，完整公式成功。
5. 完整及嵌套公式与未修改的 stock Typst 同夹具、同字体图像逐像素一致。

本报告不把这些判据扩展为完整 K1：装饰逐项归属、所有数学节点和完整结构映射还需验证。

## 环境与方法

Rust 1.96.0 release，Typst 0.15.1，comemo 0.5.1；Linux 7.2.7-arch1-1，
i7-13650HX，20 逻辑 CPU。字体来自 typst-kit embedded + system，默认 Library 样式。
段落为“中文与 English，行内分数：” + 公式 + “结束。”，Region 宽 420 pt。
图片带 10 pt 边距，按 renderer 默认 2 px/pt 输出。系统字体依赖使跨机器 golden 不受控；
像素对照始终在同一机器将 fork 与 stock 重新生成。

固定官方上游 `v0.15.1`，commit `9dfd3a08500b7896045f907433cf7b4b02434fad`；
只保存补丁与 bootstrap，实际 checkout 位于被忽略的 `.vendor/typst`。
上游 Apache-2.0 与仓库许可证政策相容。产品 workspace 未引用此 fork。

### 补丁边界

- `typst-library` 的实验性 `editor` feature：Content 附带 opaque node/slot ID，
  身份进入 Hash/相等判断；MathProperties 传递 origin，Frame 保存不打印的 EditBounds。
- 内部 EditHoleElem 不注册为源码语言语法。仅编辑模式允许，且必须带 hole/slot 身份。
  Library 默认 strict，模式进入 Library Hash，防止编辑模式缓存绕过 strict 检查。
- Hole 使用原有 boxed math 分支，尺寸来自当前数学字体和 scriptstyle 字号，产生非零宽高，
  没有 Text/Shape；未增加 FrameItem 或 MathKind 变体。
- `typst-layout` 将分数框与字形 origin 记录到 Frame。字形 identity 在缓存字形指标之后绑定，
  防止相同文本的不同节点复用错误身份。
- 原有 Frame 合并时平移并转移 EditBounds，包括没有可打印 item 的 Hole；
  transform 使用已有 Group 坐标树。绘制层不需要处理编辑元数据。

独立 `reference/Cargo.toml` 编译相同 World/Content 夹具，依赖来自未修改的 registry，
没有 path patch。World::source 每次计数并返回 NotFound；布局断言要求零读次数。
命中仅使用输出矩形的小面积优先查询，不是生产光标算法。

## 结果与证据

[总验证日志](evidence/SPK-0050/verification.log)、[内核断言](evidence/SPK-0050/kernel.log)。

| 判据 | 结果 |
|---|---|
| 空分母几何与命中 | Pass：slot 12 非零面积，位于分子下方；父分数框包含两槽中心 |
| 无可打印占位，填槽正确 | Pass：空槽 21 glyphs / 1 shape（分数线）；填入 `2` 后多 1 glyph，shape 不变，hole=false |
| 缓存身份隔离与修改 | Pass：重复 Frame hash 一致；相同文本 node 100/200 各有自身身份且坐标一致；`22` 分母区域扩大 |
| 嵌套结构与坐标变换 | Pass：内层 node 300 两槽可定位；外层 node 400 框包含内层中心；平移 (30,70) pt 后 2 倍缩放位置符合断言 |
| strict 模式 | Pass：同一进程编辑模式先缓存 Hole，再切 strict 仍拒绝；完整公式成功 |
| stock 完整公式对照 | Pass：880×55，差异像素 0 |
| stock 嵌套公式对照 | Pass：880×55，差异像素 0 |
| source 读取 | Pass：0 |

可视证据：[空槽](evidence/SPK-0050/empty-slot.png)、[完整公式](evidence/SPK-0050/fork-full.png)、
[嵌套公式](evidence/SPK-0050/fork-nested.png)；stock 文件与生成日志保存在同一目录。
已人工查看空槽和嵌套图片，中文、英文与分数可见，空分母没有打印占位符。

第一版为保留 origin 禁止 Frame inline，嵌套图出现 3 像素差异。修正为沿既有 inline
转移元数据后差异为零；这证明身份实现不能任意改变绘制分组。

格式、release Clippy（探针与 stock reference，`-D warnings`）、禁用 editor feature 的
编译和仓库许可证/来源/bans 检查通过，对应日志保存在同一证据目录。

## 局限与下一步

- 每个 Content 只带一个 origin；嵌套夹具验证内层身份与外层框，未验证外层分子 slot
  和内层节点同时标记。需要 slot wrapper 或多层 origin，不能称完整数学树映射完成。
- 分数线位于对应分数框，但没有逐项装饰 owner。正文、合字内部 caret、RTL、selection、
  IME、show rule 重写后的来源、裁剪几何、旋转命中与所有 Frame 修改组合未覆盖。
- strict 只验证当前数学 Hole 的布局拒绝，不是完整 PDF/HTML 导出管线验收。
- 仍为固定 Content 夹具，EmptyIntrospector 与 Locator::root；没有持久文档会话、
  delta adapter、局部分页、全局 state/query/counter 依赖、沙箱或资源上限测量。
- 没有测击键到显示时延，不能承诺消除现有 UI 的临时回显。下一步先补 K1 的组合身份和
  装饰归属，再推进 K2 同源光标几何；接入真实窗口前保留产品当前路径。

ADR 0032 保持 Proposed；本结果不升级阶段出口或废弃已接受的产品架构。

## 复现

```sh
python3 spikes/typst-edit-session/bootstrap.py
python3 spikes/typst-edit-session/check.py
cargo fmt --manifest-path spikes/typst-edit-session/Cargo.toml -- --check
cargo clippy --release --locked --manifest-path spikes/typst-edit-session/Cargo.toml --all-targets -- -D warnings
cargo check --release --locked --no-default-features --manifest-path spikes/typst-edit-session/Cargo.toml
cargo deny --manifest-path spikes/typst-edit-session/Cargo.toml --config deny.toml --locked check licenses bans sources
```

干净机器需要网络和 Pillow，已有依赖可离线编译。另以临时干净上游 checkout 验证
补丁可应用且所有修改文件字节相同；bootstrap 在已应用 checkout 上重复运行也通过。
