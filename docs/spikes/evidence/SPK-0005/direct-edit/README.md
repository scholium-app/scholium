# 2026-09-24 产品页面直接编辑复验

报告 0005 的字形/源位置映射机制在本地产品块文档中的复用证据。
不修改阶段出口判定，不等于完整数学结构编辑或 P1 验收。
对应决策：[ADR 0029](../../../../adr/ADR-0029-direct-page-editing.md)。

环境：Linux / Xvfb X11，Rust 1.96.0，egui/eframe 0.36.2，Typst 0.15.1；依赖由根 Cargo.lock 锁定。
原生应用为 `target/debug/scholium-app`；缩放 1，窗口 1280×900 与 640×480。
会话数据库使用脚本新建的临时目录，不接触用户会话。

从仓库根复现：

```sh
cargo test --locked --workspace
cargo build --locked -p scholium-app
xvfb-run -a -s '-screen 0 1400x1000x24' python3 spikes/typst-mapping/review-direct-page.py
```

窗口脚本依赖 Xvfb、xdotool、xclip、ImageMagick (`magick`)；输出目录由脚本打印。
它粘贴中英/公式夹具，在已排版公式上拖选括号项，断言复制内容，输入 `x` 原位替换；
再拖选中文首段并断言复制内容。截图需人工核对高亮与光标，没有普通输入框覆盖排版。

- [tests.log](tests.log)：61 项测试全部通过（app 34、document 11、storage 8、typst 8）。
- [window.log](window.log)：真实窗口剪贴板断言结果。
- [direct.png](direct.png)：直接输入后的中文、行内与独立公式。
- [math-selection.png](math-selection.png)：公式内部的实际字形选区。
- [math-edited.png](math-edited.png)：输入 `x` 后重新排版及末尾光标。
- [text-selection.png](text-selection.png)：中文全文连续选择。
- [source.png](source.png)：替换后的只读生成源码与预览一致。
- [narrow.png](narrow.png)：640×480 适合宽度，无独立输入区。

格式、clippy（warnings deny）、文档（warnings deny）、文件规模、diff whitespace、许可证/来源检查均通过。
`cargo deny` 仅保留现有 Unicode-DFS-2016 许可白名单未遇到提示。
真实系统输入法候选/读屏、Windows/macOS、高 DPI、完整结构化数学导航不包含在本次窗口结论中。
