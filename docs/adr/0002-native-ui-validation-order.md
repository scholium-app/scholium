# ADR 0002：原生技术栈与 UI 验证顺序

- 状态：Accepted（技术栈约束与验证顺序；不是框架选型）
- 日期：2026-09-16
- 决策者：用户
- 影响模块：app、render、model、document、collab、build 与测试/构建流程

## 背景

旧文档建议 ProseMirror、CodeMirror/Monaco 与 Tauri/WebView。用户明确要求纯 Rust 或必要的 C/C++/Zig，
并指定候选顺序，因此原浏览器编辑器路线废止。原型结论不能直接证明原生编辑器可用。

## 决策

按 **Iced → GPUI → C++ EUI-NEO → Slint 或 egui** 依次验证。EUI-NEO 对应
[sudoevolve/EUI-NEO](https://github.com/sudoevolve/EUI-NEO)。末组候选根据前序缺口选择，不提前绑定。
应用核心优先 Rust，UI 采用原生窗口/文本/绘制；必要 C/C++/Zig 通过显式 FFI 或受控进程接口接入。
不使用 npm/Node.js、JS/TS 编辑器、WebView/Electron/Tauri 作为应用 UI 或验证依赖。

## 验证方法

按照 [原生 UI 验证计划](../NATIVE_UI_VALIDATION.md) 执行相同输入法、数学结构、源码、协作、性能、
无障碍和文件恢复脚本。每个候选记录锁定版本、平台、命令与 Pass/Fail/Blocked；目前没有原生候选实测结果。
最终框架及原生库选型另立有实验证据的 ADR，本记录只确认用户指定的范围和顺序。

## 后果

不能依靠现成 Web 富文本组件完成科学编辑；原生数学控件、源码状态适配和平台输入需要单独验证与估算。
EUI-NEO 路线包含实际 C++ UI 工作量。保留 Rust 权威文档边界，避免 UI 框架替换牵动持久化/协作模型。
HTML/MathML 导入导出与 LaTeX/Typst 文档能力保留，它们不改变应用语言约束。

## 替换方案

候选失败时按既定顺序继续并记录原因；均失败时重新评估原生方案。修改顺序、提前结束比较或放宽语言范围需用户选择，
不得以实现困难为由静默恢复浏览器路线。

## 后续补充

[ADR 0003](0003-wasm-and-stack-validation.md)补充 WASM 核心与可选浏览器宿主，有限允许工具生成加载/绑定胶水；原生桌面路线与 GUI 验证顺序不变。
