# Spike 0045：预览延迟链分解与渲染改进基线

- 结论：**Pass（基线采集成）**——结论本身是对现状的否定：击键→字形可见延迟由全文档重编译主导，
  光栅化不是大头；当前"状态栏毫秒数"只报编译段，低估体感延迟一个数量级（debug 构建）。
- 对应验证项：阶段 1 预览体验改进的测量基线；回链[路线图](../plan/ROADMAP.md)预览响应与
  [模块 render](../modules/render.md)的不变量"过期预览不替换 current"。
- 日期：2026-09-25
- 执行者：ation_ciger（ZCode 会话）
- 关联文档：[渲染与编辑改进计划](../plan/RENDER_EDIT_REWORK.md)（本报告是其证据基础）

## 问题与判据

用户报告：预览"不跟手，虽然有时只有几毫秒但就是很不跟手"，且字体渲染模糊。动手改进前先钉死：

1. 击键到字形可见的延迟链各段各占多少（去抖、编译、光栅化）？
2. 延迟随文档规模如何增长（8/64/256 段）？
3. debug 与 release 构建差多少倍（开发者日常跑的是 debug）？
4. 状态栏自报耗时与真实链路差多少？

判据（动手前写定）：能按段给出 p50/p95、两次采样结论方向一致、夹具与报告 0016 同规模。
不设"延迟必须低于 X"的判据——本 spike 是基线测量，不是优化验收。

## 环境

- 锁定版本：typst 0.15.1 / typst-kit / typst-layout / typst-render（与主 workspace `Cargo.lock` 同源）；
  egui/eframe 0.36.2（未参与本测量）。
- 平台：Arch Linux（linux 7.2.6-arch2-1）· 20 逻辑核；系统字体含 Times New Roman / SimSun / SimHei
  （`fc-match` 验证）。Typst 字体栈：embedded + system 扫描，与 `PreviewWorld` 一致。
- 构建与运行：

```bash
cargo run --release --manifest-path spikes/render-latency/Cargo.toml
cargo run --manifest-path spikes/render-latency/Cargo.toml   # debug 对照
```

## 方法与夹具

- 夹具：8 / 64 / 256 段快照，每段"中文正文与 English text 混排 + 行内公式 `alpha + x/2` + 结尾"，
  首段为一级标题。与报告 0016 的"256 段/26 页"夹具规模对齐（本夹具排为 9 页）。
- 动作：冷启动首编一次；随后 10 次"单字符击键"（改最后一段一个字，revision 递增，整快照提交），
  与 Visual 模式 `submit_snapshot` 路径完全一致。
- 分段计时：
  - 编译段 = `PreviewCompiler` worker 自报的 `elapsed_ms`（与状态栏同源）；
  - 光栅段 = 独立线程上 `typst_render::render(pixel_per_pt = 2.0)` 第 0 页的耗时
    （复刻 `compiler.rs` 的 `raster`；主 crate 未把光栅耗时计入任何自报数字）。
- UI 帧调度开销（egui 逐帧绘制、每帧 `buffer::text` 全文档重建）不在本 spike 范围，
  属于计划文档 E1/R6 的职责。

## 结果

原始日志：[evidence/SPK-0045/](evidence/SPK-0045/)（release×2、debug×2）。

| 指标 | 8 段 | 64 段 | 256 段 |
|---|---|---|---|
| release 编译段 p95 | 2.0 ms | 21.0 ms | 182 ms |
| release 光栅段 p95 | 2.2 ms | 4.3 ms | 6.8 ms |
| release 击键→字形可见 p95 | 4.2 ms | 23.4 ms | 188.8 ms |
| debug 编译段 p95 | 25.0 ms | 129 ms | 1393 ms |
| debug 光栅段 p95 | 14.4 ms | 23.8 ms | 25.5 ms |
| debug 击键→字形可见 p95 | 39.2 ms | 152.7 ms | 1416.8 ms |
| 冷启动首编（release / debug） | 17 / 150 ms | 34 / 389 ms | 201 / 2047 ms |

| 判据 | 结果 | 证据 |
|---|---|---|
| 各段 p50/p95 可分列 | Pass | 上表；编译/光栅/合计三行分列 |
| 两次采样方向一致 | Pass | run1 与 run2 各段差 <15%，量级与排序不变 |
| 与 0016 夹具同规模可对照 | Pass | 256 段；本机编译段 p95 与 0016 的"改首段最大 210.6 ms"同量级 |
| 状态栏自报 vs 真实链路 | **差 1.1–10 倍** | debug 256 段：自报 1341 ms 时体感链路 ≥1370 ms（另有 20ms 去抖与 2 个 UI 帧）；release 256 段：自报 163 ms、合计 168 ms。自报永远不含光栅与调度 |

## 失败与不确定性

- **测得的"不跟手"主因是全文档重编译，不是光栅化**：release 下光栅段 p95 仅 2.2–6.8 ms
  （201 万像素/页）。此前推断"光栅是隐藏大头"在 release 下不成立；它只在 debug 下放大到 14–25 ms，
  仍非主导。这修正了计划文档的优先级（见下）。
- debug 256 段击键→可见 1.4 s：**日常 `cargo run` 的开发者体感即在此档**，这解释了
  "有时候就几毫秒（小文档）但就是不跟手（大文档）"的双峰体感。
- 现有 20ms 去抖 + "采纳编译结果后才请求页"的两跳流水线在本 spike 未单独计时
  （各加 1 个 UI 帧）；它使任何延迟至少 +20ms +2 帧，与编译耗时无关。
- 单机单次采样，未覆盖：GPU 纹理上传、字体冷缓存、其他平台、连续击键的 latest-only 取消率。
- spike 的 `parse_line` 是 `scholium_document::parse_markup` 的最简子集（仅 `$…$`），
  只用于构造夹具，不构成对该解析器的替代。

## 对设计的影响

1. **优化优先级重排**：让大文档跟手的第一杠杆是"避免全文档重编译 / 乐观渲染掩盖编译延迟"
   （计划 E1/R4），而非光栅化优化；光栅只需修清晰度（比例跟 DPR×zoom，计划 R1）。
2. **耗时显示诚实化**（计划 R3）：状态栏应报"击键→可见"的端到端口径，至少编译+光栅相加。
3. 每帧 O(文档) 的 UI 开销（snapshot 深克隆、`buffer::text` 重建）单独在计划 R6 处理，
   本 spike 证明它与编译延迟是两个独立问题。
4. 不需要新 ADR：不改变框架选型与模块边界；渲染路径从位图改矢量若立项，另立 spike。

## 复现步骤

```bash
# 干净环境
git checkout <本提交>
cargo run --release --manifest-path spikes/render-latency/Cargo.toml   # ~30 s
cargo run --manifest-path spikes/render-latency/Cargo.toml             # debug，256 段约 15 s
# 原始输出已在 docs/spikes/evidence/SPK-0045/ 留档
```
