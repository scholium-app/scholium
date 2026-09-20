# Spike 0043：宿主产物读取与几何校验

- 日期：2026-09-20；基线：4d2c935。
- 结论：修复原生 UI 产物读取边界中的具体缺陷；条件 6 不升级。

## 复现与修复

构造 worker 几何响应，`offset=u64::MAX,length=2` 在宿主产生加法溢出；
`x=1e100` 转为 f32 无穷值后仍进入绘制几何。[失败对照](evidence/SPK-0043/range-before.log)
在修改解析逻辑前运行，两个用例均失败。

当前场景接收校验源码范围、有限且有界的坐标、墨迹矩形顺序、UTF-8 文本范围与 checked_add。
原范围夹取删除，保留明确的 Typst 单字形变换例外：一字节数学字母可能排为四字节样式 Unicode；
此时实际 glyph 字节数须等于声明长度、源尾部和 glyph 均为单 scalar，映射回原字符范围。
结构装饰符号没有可编辑 text cursor，按结构墨迹处理。过度严格的初版令 5 个真实编译测试失败；
修正这两类合法产物后真实编译集恢复通过，没有降低恶意 offset/长度测试的拒绝要求。

新增宿主读取入口：以 O_NOFOLLOW/O_NONBLOCK 打开产物，用同一 fd 确认普通文件和大小，
实际读取用 limit+1 防并发增长。场景 JSON 上限 32 MiB；每块原始 RGBA 以期望长度校验。
PNG 只接受标准 page-N.png、正确魔数和 64 MiB 文件上限，保留 4096 边长/32M 总像素的解码预算。
最终路径的 symlink、目录、FIFO 均拒绝；不宣称防同 UID 恶意程序替换整个应用私有目录。

预览编译/query 改为写入宿主私有日志文件后有界接收，stdout/stderr 各 4 MiB；
超限显式失败，不静默截断或继续发布。文件不挂载给 worker，子进程写入仍受 fsize 限制。
长驻 renderer 响应队列容量 1，每行保留原有 1024 字节上限，避免噪声输出导致无限队列。

## 验证

- 默认 UI 测试 [62 passed / 11 ignored](evidence/SPK-0043/tests.log)。
- 打开 ignored 的串行库测试 [55 passed / 0 failed](evidence/SPK-0043/integration.log)，
  含真实预览、完整夹具映射、转义文字/空格、空槽透明背景、增量瓦片/多页缩减、worker 超时和父进程强杀。
  `parent_death_fixture` 是测试辅助入口，不算单独的外部故障验证。
- [Clippy](evidence/SPK-0043/clippy.log) all-targets / -D warnings 与修改文件 rustfmt 通过。
- [窗口回归](evidence/SPK-0043/desktop.json) 6/7：中文 IME、Unicode、空槽、多页、帧采样、连续输入通过；
  数学读屏因失焦保护中止注入，保持 Fail 记录；[独立补跑](evidence/SPK-0043/reader-recheck.json)通过。
- 新负对照包括普通文件限长、FIFO/目录/symlink、超限命令输出、PNG 宽度/魔数/缺页、范围溢出和无穷坐标。

```bash
CARGO_HOME="$PWD/spikes/native-ui/.cargo-home" cargo test --offline --manifest-path spikes/native-ui/candidate-egui/Cargo.toml
CARGO_HOME="$PWD/spikes/native-ui/.cargo-home" SCHOLIUM_TYPST_BIN=/tmp/scholium-typst-toolchain/bin/typst \
cargo test --offline --manifest-path spikes/native-ui/candidate-egui/Cargo.toml --lib -- --include-ignored --test-threads=1
```

## 未关闭边界

这些是 UI 接收端的格式/资源校验，不是解码器进程隔离或漏洞免疫声明。
32 MiB JSON 反序列化为 Value 的实际内存放大仍需单独计量；PNG 解码仍在宿主工作线程执行。
其他研究入口、根外可信运行时例外和 /work 运行时总配额仍未关闭。
命令接收上限不代替构建磁盘总预算，不能凭这一项把条件 6 判定为满足。
