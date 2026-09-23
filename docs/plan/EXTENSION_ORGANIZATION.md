# 会话与扩展代码组织计划

本计划定义后续拆分方向，不声明以下 crate 已实现。边界提案见 [ADR 0025](../adr/ADR-0025-extension-boundaries.md)。

## 组织与依赖

```text
crates/
  scholium-model/             稳定领域身份、位置、诊断；不放外部协议
  scholium-document/          文档结构与编辑校验
  scholium-session/           项目命令、权限与事务编排（按需提取）
  scholium-app/               原生 UI、投影和平台交互
  scholium-agent-pi/          Pi 外部进程、协议映射与任务策略
  scholium-mcp/               MCP server、schema 和传输适配
  scholium-plugin-api/        面向 Rust 插件作者的 SDK
  scholium-plugin-host/       WIT 宿主绑定、资源映射和 session 调用
  scholium-plugin-runtime/    组件执行、限额、取消与实例生命周期
wit/scholium-plugin/          版本化 WIT，宿主绑定与 SDK 共用
examples/plugins/            Rust 与非 Rust 的最小组件
```

箭头表示调用/依赖方向：app、Pi adapter、MCP adapter、plugin-host → session → document/history/collab/storage/build。
plugin-host 另依赖 plugin-runtime；plugin-api 面向 guest，仅依赖生成绑定及 SDK 必要库，不依赖 session、UI 或宿主运行时。
生成的 WIT 类型和 MCP/Pi schema 在适配层转换，不进入 model；session 不依赖外部协议和 egui。
运行时如需调用宿主，由宿主实现回调端口，避免 runtime 与 host 相互依赖。

session 统一处理项目/actor、revision、权限、源码 epoch、幂等、Action 与持久化确认。
UI、Pi、MCP 和插件提供的身份与许可不能直接信任；适配层建立调用身份，session 复核授权。
桌面关闭弹窗、选择范围和 proposal 展示留在 app。无界面调用使用显式策略或返回待确认状态，不能依赖 egui 弹窗才能完成。

## 提取顺序

1. 保留现有三个 crate 与 ADR 0024 的有限 LocalSession；不把临时段落 Action 当作永久协议。
2. 正式项目编排出现，或第二个调用入口接入时，提取不依赖 UI 的 session。
   用同一编辑场景验证 UI 与无界面入口的 revision、拒绝行为和 Action 结果一致。
3. Pi 与 MCP 分别作为外部适配器实现。Agent Gateway 初期是 Pi 适配层的内部模块，第二个实际 agent 出现后再抽公共接口。
4. 插件先建立最小 WIT world 与两种语言夹具，再分离 guest SDK、host 与 runtime。
   host bindings 初期放在 host 内；只有编译成本或复用需求明确时才独立 crate。
5. 每次正式提取同步补齐模块文档七节、依赖门禁及专项 ADR；未实现的候选目录不创建占位 crate。

## 可移植性与验收

- session/领域测试不链接 egui、Pi 运行时或 Wasmtime；插件 guest SDK 不传递宿主依赖。
- 真实 Pi 接入、MCP 协议和插件运行测试各自独立，基础编辑离线可用。
- WIT 兼容性以 world 版本和夹具验证，内部 Rust struct 不直接成为长期 ABI。
- 桌面优先验证 Windows、macOS、Linux；WASI guest 能力与浏览器宿主能力分别列矩阵。
  浏览器 Worker 不是组件模型运行时，也不自动支持 Wasmtime；绑定/加载工具链须另行验证。
- 跨语言目标先验证 Rust 与至少一种非 Rust 语言。其他语言只有构建、运行、错误及资源回收夹具通过后才列为支持。

## Pumpkin 参考

参考 [Pumpkin 固定提交](https://github.com/Pumpkin-MC/Pumpkin/tree/43bcd7b108abfe2d10fda28002efa87d48babda3)：
`pumpkin-plugin-wit/v0.1` 作为共享契约，`pumpkin-plugin-api` 使用 wit-bindgen，
`pumpkin-host-bindings` 使用 Wasmtime bindgen，`pumpkin-plugin-runtime` 分离执行与生命周期。
借鉴这些职责边界；Scholium 的文档事务、协作语言门禁和浏览器支持必须独立设计与验证。
