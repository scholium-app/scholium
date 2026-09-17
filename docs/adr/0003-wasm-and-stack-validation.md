# ADR 0003：全栈验证范围与 WASM 优先兼容

- 状态：Accepted（目标及验证方向；不是库或浏览器发行选型）
- 日期：2026-09-16
- 决策者：用户
- 影响模块：model、history、collab、format、storage、render、build、app、sync-server 与测试/构建流程

## 背景

[ADR 0002](0002-native-ui-validation-order.md) 把应用约束到原生 UI，但没有定义 GUI 以外各层（解析、编译、
渲染、协作、存储、服务端）的验证方向，也没有回答 Rust 核心能否复用到浏览器。用户在确认 GUI 顺序后要求
优先保证核心可移植，并允许在明确边界内使用浏览器宿主。若不在阶段 0 一并验证，后期替换 CRDT、存储或排版
后端会牵动持久化格式与协议。

## 候选方案

1. 只验证桌面，浏览器能力后置：核心容易过早绑定文件系统、进程和具体数据库，移植成本延后爆发。
2. 直接用 Web 技术实现浏览器版本：违反 ADR 0002 的语言约束。排除。
3. 核心可移植优先 + 声明式宿主适配 + WASM 作为优先兼容目标：作为待验证方案。采用。
4. 各层一次性锁定第三方库：与"候选不等于依赖"和可替换性冲突，改为逐项 spike 后另立 ADR。排除。

## 决策

在 ADR 0002 的原生语言约束与 GUI 顺序上，补充[全栈候选](../plan/TECH_STACK.md)和[WASM 计划](../plan/WASM.md)。
优先使核心可移植，再验证浏览器宿主。浏览器可使用必要的工具生成加载/绑定胶水，不引入 npm/Node.js 或
JS/TS 业务编辑器；桌面原生路线不变。这是对 ADR 0002 禁止浏览器编辑器路线的有限补充，不是恢复旧 Web 原型。

LaTeX 参考[Mogan 已阅读源码](../research/MOGAN_LATEX.md)的架构结论，由 Rust 独立实现。GUI 顺序保持不变；
其他明确比较顺序为 Loro → Yrs → Automerge、SQLite/rusqlite → redb、TeX Live → Tectonic。

## 验证方法

按 [WASM 计划](../plan/WASM.md) 与 [全栈候选](../plan/TECH_STACK.md) 执行：纯逻辑 crate 先在同一语料下通过原生与
`wasm32-unknown-unknown` 的编译**与运行**测试，再验证浏览器宿主、Typst Worker、PDF/SVG 与持久化适配。
每项记录锁定版本、平台、可复现命令与 Pass/Fail/Blocked，报告放 `docs/spikes/`。仅 `cargo check` 通过
不算运行通过，浏览器脚本也不能替代原生 UI 验收。

## 后果

核心与宿主 IO、调度、存储、构建分离，并增加跨端语义与协议测试。成本是核心 API 必须保持声明式、不泄漏
平台句柄，C/C++/Zig 依赖必须单独验证 WASM 产物。WASM 编译通过不代表浏览器完整可用。

## 待定项

完整浏览器发行范围、LaTeX 本地 WASM 后端与 WASI 目标、各层最终库版本仍须实验及后续 ADR。当前没有
新增运行验证结果。

## 替换方案

各层适配器（CRDT engine、存储后端、排版引擎、宿主调度）都定义在稳定 trait 之后，可按验证结果替换，
但必须保留：语义与协议测试跨端可运行、核心不依赖具体数据库或进程、浏览器缺能力时显式降级而不是静默上传。
持久化格式与协议破坏性变化需新 ADR、迁移和恢复测试。
