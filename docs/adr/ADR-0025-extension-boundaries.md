# ADR 0025：会话复用与 Pi/MCP/WIT 扩展边界

- 状态：Proposed
- 日期：2026-09-23
- 影响模块：app、document、未来 session 与扩展适配器

## 背景

用户要求在路线中直接接入 Pi agent、提供 MCP server，并采用 WIT/WASM 插件。
当前 LocalSession 是 ADR 0024 限定的内存段落适配器；未来 ProjectSession 不宜绑定桌面 UI。

## 候选方案

1. 全部放入 app：初期简单，但 MCP/插件复用会携带 UI 依赖，权限规则容易重复。
2. 独立 session 与协议适配层：增加 DTO 转换成本，但统一写入校验，外部协议可替换。
3. 一次性建立全套空 crate：缺乏实际调用验证，增加维护负担。

## 拟议决策

选择方案 2，按实际功能逐步提取，具体结构见 [组织计划](../plan/EXTENSION_ORGANIZATION.md)。
Pi 是可选外部 agent，不是 Provider Interface；原生应用无需 Pi 即可构建、测试和编辑。
若 Pi 需要 Node.js，仅允许其可选外部运行与专项验证，不扩展到应用 UI、核心或常规验证依赖。
MCP 首版提供 server。WIT 共用于插件 SDK 与宿主绑定，运行时不渗入领域模型。

## 验证方法

提取 session 前用 UI/无界面同场景验证编辑和拒绝行为一致；Pi 验证真实进程、协议、权限隔离及取消；
MCP 验证认证、幂等和过期请求；WIT 验证至少两种语言、版本兼容、限额及故障回收。
专项 spike 需记录版本、许可证、复现命令和失败夹具；本提案不关闭任何阶段 0 门禁。

## 后果与替换方案

增加适配代码，但保留独立更新 Pi、MCP 协议和组件运行时的能力。
未来更换 agent 或运行时只改适配器；保留 session 命令语义和版本化 WIT 兼容性。
尚未锁定 Pi 发行版本、通信协议或 WASM 运行时，正式选型须由验证证据支持。
