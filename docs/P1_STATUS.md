# P1 执行情况

更新日期：2026-08-08。

## 结论

P1 已不是目录和 trait 空壳，核心数据链与桌面程序能够运行；此前真正缺失的是
编辑器交互闭环：没有鼠标命中测试，caret 与 AST 光标无关，IME preedit 不上屏，
CI 也没有三平台构建。这些缺口现已补齐代码与自动测试。

当前状态定义为「可验收」，而不是「已完成」：Linux、Windows、macOS 的人工 IME
验收尚未全部留下记录，尤其 Windows/macOS 不能用“编译成功”代替输入法实测。

## 计划对照

| P1 项目 | 状态 | 证据 / 剩余工作 |
|---|---|---|
| winit + wgpu + vello 窗口 | 已实现 | `scholium-shell` 可执行程序与 vello surface |
| 最小 AST | 已实现 | document / paragraph / heading / text |
| EditOp / Transaction / Origin | 已实现 | 事务原子应用；SetAttr 不再是空操作 |
| serialize + SourceMap | 已实现 | Typst 保留字符转义后仍映射到原始 UTF-8 偏移 |
| Typst World / 字体 / 编译 | 已实现 | 内置 Typst 字体、三平台系统 CJK 路径与缺字自检 |
| Frame → vello | 已实现 | 文本、页面与基础 shape 绘制 |
| 光标与命中测试 | 已实现 | Glyph span → NodeId → Cursor；点击与 caret 共用映射 |
| 键盘输入 | 已实现 | 插入、左右/Home/End、Backspace/Delete |
| IME 中文输入 | 可验收 | preedit 临时编译、下划线、commit 入事务、候选框跟随 |
| undo / redo | 已实现 | 文档与光标一起恢复 |
| MockAgent 链路 | 已实现 | 异步 trait、提案应用与撤销测试 |
| CI | 已配置 | fmt / clippy / test / deny / audit / doc + 三平台 build matrix |

## 验收步骤

在三个系统分别执行 `cargo run -p scholium-shell`，记录以下结果：

1. 输入 `中英 mix # * [ ]`，确认符号按正文显示、中文无方框。
2. 点击每个汉字左右两侧，确认 caret 落点与后续插入位置一致。
3. 用系统中文输入法观察 preedit 下划线与候选框位置，选词后只提交一次。
4. 在混排文本中执行左右移动、Backspace/Delete，再连续 undo/redo。
5. 缩放窗口后重复点击和 IME 候选框定位。

人工结果应按操作系统、窗口系统和输入法版本记录。三平台全部通过后，才把 P1 标为完成。

## 已知边界

- P1 只编辑单个基础文本流；跨段落选择、上下方向导航和多页滚动属于后续编辑器工作。
- 当前优先使用系统 CJK 字体，最终发行包仍应捆绑 OFL 字体以保证跨机器一致性。
- IME preedit 保留了输入法给出的字节范围，P1 只绘制整段下划线；日文分段样式可后续细化。
