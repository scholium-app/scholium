# 2026-09-24：公式块输入失败与延迟复验

限定结论：Pass。不等于完整结构化公式编辑验收。
决策：[ADR 0030](../../../../adr/ADR-0030-incomplete-formula-feedback.md)。

环境：Linux Xvfb/X11，Rust 1.96.0，Typst 0.15.1，egui/eframe 0.36.2，根 Cargo.lock。
用户启动命令 `cargo run -p scholium-app`；复测使用同一 debug 构建，独立临时会话数据库。

修复前实际失败：`al` 返回 `unknown variable: al`，没有更新后的页面；
切源码再返回后输入 ` world`，正文仍为 `hello`；诊断面板显示“尚未运行检查”。
对应回归均先失败再修复。

复现：

```sh
cargo test -p scholium-typst -- --nocapture
cargo test -p scholium-app
cargo build --locked -p scholium-app
xvfb-run -a -s '-screen 0 1400x1000x24' python3 spikes/typst-mapping/review-formula-input.py
```

窗口脚本需要 xdotool、xclip、ImageMagick；点击实际独立公式按钮，逐字输入，截图人工复核。

- [formula-al.png](formula-al.png)：`al` 在公式原位显示并有光标。
- [formula-incomplete.png](formula-incomplete.png)：`alpha/` 不冻结页面。
- [formula-diagnostic.png](formula-diagnostic.png)：真实 `expected expression` 原因可见。
- [formula-completed.png](formula-completed.png)：补 `2` 后恢复 α/2 分式，光标落在末尾。
- [formula-source.png](formula-source.png)：语义公式源码仍为用户输入，无替代文本污染。

冷 worker 从提交到首编译返回约 2.4–2.6 秒，主要为系统字体扫描，编译计时不含扫描。
初次扫描现提前到应用启动，仍可能在立即新建时等待。
同进程暖态短公式 `al`、`alpha/`、`alpha/2`，主动回调测得从提交到栅格页事件约
11、11、12 ms；不含 20 ms 去抖、UI 帧与显示器刷新，也不代表长文档性能。
旧 25 ms 轮询测试仅能证明结果在一次轮询内到达，不能当作精确编译耗时。

正式生成源码仍严格；输入期替代不能用于正式输出。只替换诊断明确指向的公式，
其它数学字形和正文保持排版，源串与权威不改变。

最终工作区回归：67 项通过（app 36、document 11、storage 8、typst 12），日志见 [tests.log](tests.log)。
格式、clippy warnings 门禁、文档 warnings 门禁和文件规模检查通过。
