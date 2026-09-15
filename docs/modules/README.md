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
| `scholium-latex` | [latex.md](latex.md) | LaTeX CST、项目图、语义操作 |
| `scholium-typst` | [typst.md](typst.md) | Typst CST、生成、reconcile 与编译桥接 |
| `scholium-markdown` | [markdown.md](markdown.md) | Markdown CST、方言与 HTML 投影 |
| `scholium-bib` | [bib.md](bib.md) | BibTeX CST、文献索引与 CSL |
| `scholium-build` | [build.md](build.md) | 安全构建、预览、产物与日志映射 |
| `scholium-render` | [render.md](render.md) | 语义图快速预览、SourceMap 与双后端状态 |
| `scholium-app` | [app.md](app.md) | 桌面 UI、项目 session 和模块编排 |
| `scholium-sync-server` | [sync-server.md](sync-server.md) | 鉴权、中继、快照、presence、团队源码语言协调与写集校验 |

模块间只能通过公共 DTO、trait 或事件交互。若两个模块需要互相依赖，应把共享概念下沉到 model，
或引入由上层 app 实现的端口，不能形成 crate 环。

团队语言控制、ForeignSource、文件交付和构建桥接横跨上述模块，公共合约统一见
[混合源码与团队编辑](../MIXED_SOURCE_EDITING.md)，技术选择见 [ADR 0001](../adr/0001-mixed-source-team-editing.md)。
