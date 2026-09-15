# ADR 0003：全栈验证范围与 WASM 优先兼容

- 状态：Accepted（目标及验证方向；不是库或浏览器发行选型）
- 日期：2026-09-16
- 决策者：用户

## 决策

在 ADR 0002 的原生语言约束与 GUI 顺序上，补充[全栈候选](../TECH_STACK.md)和[WASM 计划](../WASM.md)。优先使核心可移植，再验证浏览器宿主。浏览器可使用必要的工具生成加载/绑定胶水，不引入 npm/Node.js 或 JS/TS 业务编辑器；桌面原生路线不变。这是对 ADR 0002 禁止浏览器编辑器路线的有限补充，不是恢复旧 Web 原型。

LaTeX 参考[Mogan 已阅读源码](../research/MOGAN_LATEX.md)的架构结论，由 Rust 独立实现。GUI 顺序保持不变；其他明确比较顺序为 Loro → Yrs → Automerge、SQLite/rusqlite → redb、TeX Live → Tectonic。

## 后果与待定项

核心与宿主 IO/调度/存储/构建分离，并增加跨端语义与协议测试。WASM 编译通过不代表浏览器完整可用，完整浏览器发行、LaTeX WASM 后端与最终库版本仍须实验及后续 ADR。当前没有新增运行验证结果。
