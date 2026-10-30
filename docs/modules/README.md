# 模块设计索引

每个模块文档固定描述职责、非职责、内部结构、公共接口、不变量、失败处理和测试门禁；缺任何一节视为文档未完成，
不得作为实现依据。

| 模块 | 设计 | 主要责任 |
|---|---|---|
| `scholium-model` | [model.md](model.md) | 稳定领域类型、IR、位置、诊断 |
| `scholium-document` | [document.md](document.md) | 语义文档图、焦点、结构编辑、源码绑定 |
| `scholium-storage` | [storage.md](storage.md) | 项目、WAL、快照、原子物化、恢复 |
| `scholium-history` | [history.md](history.md) | Action、checkpoint DAG、撤回与合并 |
| `scholium-collab` | [collab.md](collab.md) | CRDT 封装、相对位置、update 协议 |
| `scholium-format` | [format.md](format.md) | 适配器合约、IR 转换、导出报告 |
| `scholium-latex` | [latex.md](latex.md) | LaTeX 内核、源码权威、预览适配与转换 |
| `scholium-typst` | [typst.md](typst.md) | Typst CST、生成、reconcile 与编译桥接 |
| `scholium-markdown` | [markdown.md](markdown.md) | Markdown CST、方言与 HTML 投影 |
| `scholium-bib` | [bib.md](bib.md) | BibTeX CST、文献索引与 CSL |
| `scholium-build` | [build.md](build.md) | 安全构建、预览、产物与日志映射 |
| `scholium-render` | [render.md](render.md) | 按项目引擎预览、SourceMap 与实时/整编状态 |
| `scholium-session` | [session.md](session.md) | 无 UI 的本地编辑会话、原子命令、历史与保存 |
| `scholium-app` | [app.md](app.md) | 桌面 UI、session 适配和模块编排 |
| `scholium-sync-server` | [sync-server.md](sync-server.md) | 鉴权、中继、快照、presence 与权限撤销 |

模块间只能通过公共 DTO、trait 或事件交互。若两个模块需要互相依赖，应把共享概念下沉到 model，
或引入由上层 app 实现的端口，不能形成 crate 环。

Typst 内核包含 model 中的 SDG 类型、document 与 typst；LaTeX 内核以源码为权威，由 latex 提供解析与预览适配。
窗口壳、命令注册表、i18n、storage、history、collab、build 沙箱与诊断面板属于共享底座；内核不互相调用或共享文档模型。
项目语言在创建/导入时固定，跨语言只经转换建立新项目；混合与团队语言切换已推迟。公共合约见
[项目语言、跨格式转换与文件操作](../MIXED_SOURCE_EDITING.md) 与 [ADR 0041](../adr/ADR-0041-separate-latex-typst-kernels.md)。

## 后续扩展模块

候选 Pi adapter、MCP server、插件 SDK/host/runtime 的职责和提取顺序见 [组织计划](../plan/EXTENSION_ORGANIZATION.md)。这些尚非实现模块；正式提取时补齐七节模块规范。session 已有 Typst 本地编辑核心，见 ADR 0042。WIT、MCP、Pi 协议类型留在适配层，不进入 model。

## 已实现 crate 分层

| 层 | crate | 允许的 workspace 依赖 |
|---|---|---|
| 0 | scholium-model | 无 |
| 1 | scholium-document | model |
| 2 | scholium-typst | model、document |
| 2 | scholium-storage | model、document |
| 3 | scholium-session | model、document、typst、storage |
| 4 | scholium-app | 所有下层 |

`scripts/check-layers.py` 解析锁定 metadata 的完整依赖图：未登记 crate、向上依赖和 app 以下
直接/传递依赖 egui、eframe、winit、accesskit 均失败（包括开发与构建依赖）。候选模块不计入已实现表。
