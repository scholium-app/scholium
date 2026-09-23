# WASM 插件与 WIT 组件计划

Scholium 的插件目标是跨平台、跨语言的能力扩展。插件以 WebAssembly Component Model 为载体，以 WIT 定义接口；宿主在桌面、WASI 和浏览器分别提供能力适配。WASM 插件不是脚本权限的替代品，所有资源、网络、文件和编辑写入都由宿主授权。

组织与提取顺序见 [会话与扩展组织计划](EXTENSION_ORGANIZATION.md)。桌面三平台为初始验证目标；浏览器组件运行时另行验证，Worker 本身不能直接运行组件。

## 组件边界

插件分为三类：无副作用的格式/诊断/转换组件；受限的渲染或任务组件；能提出编辑 Action 的交互组件。插件不能直接访问 Rust 内部类型、CRDT、WAL、源码路径或 GUI 控件。它通过稳定的 WIT 类型接收文档快照、选择、能力声明和取消信号，并返回诊断、预览数据或带 base revision 的 Action proposal。

初始 WIT 包应包含：`scholium:plugin/manifest`（名称、版本、所需能力、兼容 API）；`scholium:document/snapshot`（节点、文本、稳定 ID、位置）；`scholium:action/proposal`（操作、来源、revision、语言 epoch）；`scholium:task/runtime`（进度、日志、取消）；`scholium:host/resources`（显式的只读资源句柄）。WIT 类型使用 UTF-8、明确的字节/UTF-16 位置单位、稳定 ID 和结构化错误，不暴露指针或平台路径。

插件包包含组件二进制、WIT world/hash、签名和元数据。宿主先校验签名、来源、API/world 版本、依赖和声明权限，再实例化。跨语言 SDK/绑定候选包括 Rust、C/C++、Go、Python、Java 等，各自组件模型工具链成熟度须验证，不能预先承诺全部支持，生成绑定分别属于 guest SDK 和宿主适配层；核心和桌面 UI 仍遵守无 npm/Node.js/WebView 约束。

## 权限与隔离

权限采用默认拒绝的 capability manifest：文档选择、附件读取、网络域名、临时存储、渲染资源和 Action 提交分别授权。插件运行在独立 store/instance 中，设置 fuel、内存、表项、输出大小、墙钟时间和并发上限；宿主提供取消和回收。WASI 文件系统、socket、随机数和时钟能力按平台逐项映射，浏览器只授予 Worker/宿主可用能力。

插件返回的 Action 必须经过与 AI/MCP 相同的 revision、权限、写集、语言 epoch 和语义校验。插件崩溃、超限或恶意输出只影响当前任务；宿主保留诊断并保证项目 WAL、历史和协作状态可恢复。插件升级使用签名版本、world hash 和迁移声明；不兼容版本并存，不能静默替换运行中的实例。

## 兼容策略

- WIT world 使用语义版本；包括可选字段在内的类型修改都须验证组件兼容性，不能套用 JSON 的忽略字段规则。破坏兼容性的修改必须升 major。
- 宿主维护支持的 world/API 矩阵，并在安装和启动时报告缺失能力。
- 组件不得假设桌面、WASI 与浏览器能力一致；能力缺失时返回结构化 `unsupported`，不能伪造成功。
- 文档快照和 Action schema 与项目格式版本分离；插件只能声明支持的 schema 范围。
- 插件包不进入核心 crate 的依赖闭包；运行时和组件适配的许可证、ABI、内存所有权与线程约束登记在依赖清单和 ADR 中。

## 验证与路线入口

1. **WIT contract spike**：锁定工具链/组件运行时候选，编译 Rust 与至少一种非 Rust 示例组件；跑快照、诊断、错误和版本兼容 fixture。
2. **sandbox spike**：桌面宿主优先验证 WASI 能力与资源拒绝、fuel/内存/时间上限、取消、崩溃恢复和无网络默认值；浏览器组件运行时及 Worker 路径单独探索，不作为桌面首版插件出口。
3. **document/action spike**：验证插件 proposal 经预览后转成普通 Action，旧 revision、恶意写集、语言 epoch 和协作冲突均拒绝。
4. **compatibility matrix**：记录平台、运行时、WIT world hash、工具链、启动/调用延迟、内存预算和可用能力；通过后才开放第三方开发者预览。

插件系统属于路线图阶段 6b 的扩展能力，依赖阶段 3–5 的源码、历史与权限合约；WASM 共享核心和浏览器 smoke 仍按 [WASM 兼容计划](WASM.md) 提前验证。完整插件市场、远程下载、自动更新和任意网络能力不在第一版出口内，均需单独安全与许可证审查。
