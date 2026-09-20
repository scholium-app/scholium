# ADR 0002：原生技术栈与 UI 验证顺序

- 状态：Accepted（技术栈约束与验证顺序；不是框架选型）
- 日期：2026-09-16
- 决策者：用户
- 影响模块：app、render、model、document、collab、build 与测试/构建流程

## 背景

旧文档建议 ProseMirror、CodeMirror/Monaco 与 Tauri/WebView。用户明确要求纯 Rust 或必要的 C/C++/Zig，
并指定候选顺序，因此原浏览器编辑器路线废止。原型结论不能直接证明原生编辑器可用。

## 候选方案

1. ProseMirror / CodeMirror / Monaco + Tauri/WebView：生态成熟、迭代快，但把应用 UI 绑定到浏览器栈，
   与原生技术栈要求冲突，且中文 IME、数学结构编辑和无障碍仍无法由现成组件保证。排除。
2. 只做一个自研原生控件：可控性最高，但文本塑形、IME、无障碍和三平台窗口行为全部自行实现，风险集中且难以估算。
3. 多个原生候选按同一验收集依次验证后再选型：用可比较的证据替代一次性押注。采用。
4. 同时引入多个 UI 框架：维护成本和状态泄漏风险高，也违反"候选需要独立目录与锁定版本"。排除。

## 决策

按 **Iced → GPUI → C++ EUI-NEO → Slint 或 egui** 依次验证。EUI-NEO 对应
[sudoevolve/EUI-NEO](https://github.com/sudoevolve/EUI-NEO)。末组候选根据前序缺口选择，不提前绑定。
应用核心优先 Rust，UI 采用原生窗口/文本/绘制；必要 C/C++/Zig 通过显式 FFI 或受控进程接口接入。
不使用 npm/Node.js、JS/TS 编辑器、WebView/Electron/Tauri 作为应用 UI 或验证依赖。

## 验证方法

按照 [原生 UI 验证计划](../plan/NATIVE_UI_VALIDATION.md) 执行相同输入法、数学结构、源码、协作、性能、
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

[ADR 0003](ADR-0003-wasm-and-stack-validation.md)补充 WASM 核心与可选浏览器宿主，有限允许工具生成加载/绑定胶水；原生桌面路线与 GUI 验证顺序不变。
