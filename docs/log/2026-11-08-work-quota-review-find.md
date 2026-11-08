# T51：总输出配额、宿主边界与共享源码查找

## 问题与变更

构建输出此前只有单文件限制。实现增加了内核执行的 tmpfs byte/inode 双配额，
把常驻状态和字体缓存副本放入同一会话卷，宿主只读取或复制有界普通输出。
裁决见 [ADR 0058](../adr/ADR-0058-output-quota-decoder-boundary.md)。

## 验证记录

Part A 先加入零预算和拷贝预算/链接拒绝回归，确认失败后实现。
真实 tmpfs 超字节与超 inode 对照通过；原 Typst profile 的文件/网络隔离、rlimit/cgroup、
输出超限、整树取消和 wall timeout 原生回归通过。

## 所有权交叉

`full.rs` 只新增 quota 错误到统一诊断的映射；`supervisor.rs` 只新增终止 quota 事件分支。
`typst_source/worker.rs` 只新增基础设施 quota 诊断。整编/导出和 live 调度其它行为归各任务所有。

## 宿主边界盘点

| 边界 | 限制 | hostile-input 回归 |
|---|---|---|
| rtex JSON-lines stdout/stderr | line 16 MiB、generation 64 MiB、host revision 32 MiB/4096 消息，channel 8 | malformed/truncated/unknown/oversized protocol；原 supervision 输出预算对照 |
| display-list / fragment / raster | 256 页、20000 rows、100000 items、128 fonts、坐标绝对值 ≤ 1e12；fragment 范围和长度；单边 8192、单页 24M/总 96M pixels | huge dimensions/offsets/page counts；现有 malformed scene 测试 |
| rtex 字体 | 只读系统可信字体路径、文件大小限制、128 faces/32768 outlines；TFM 名白名单 | 现有字体路径拒绝与 raster 测试；项目字体不进入宿主解析 |
| LaTeX PDF → hayro | 私有子进程、1 GiB/16 tasks/10 CPU + wall 秒；输入 64 MiB、256 页、page/scene pixels | decoder hostile frames（巨尺寸、零尺寸、缺失/截断）；无效 PDF；native render 回归 |
| decoder → host RGBA | 元数据 16 KiB；精确帧长度、排序/唯一页码、page/pixel 总预算 | hostile_decoder_frames_fail_before_allocating_pixels |
| Typst worker → host | 请求 16 MiB、元数据 1 MiB、100 页、8192 单边、64 MiB RGBA、diagnostics revision/count | worker 的现有预算校验加宿主维度与诊断身份检查 |
| LaTeX log | 4 MiB input、64 KiB line/message、1024 diagnostics、128 stack | pathological_log_is_rejected_without_building_unbounded_diagnostics |

Part B 的 plain hostile-input tests 通过；没有为此增加 fuzz 工具链依赖。
