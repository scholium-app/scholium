# AI 与 MCP 扩展设计（计划）

本文定义 Scholium 接入 Pi agent 以及 MCP 的边界。它属于阶段 6a，依赖阶段 3 的源码合约、阶段 4 的历史及阶段 5 的权限与语言门禁，不改变阶段 0 的出口条件；在进入实现前必须以 ADR 固化协议、凭据存储和审计策略。

## 目标与边界

Scholium 允许用户选择本地或远程 AI 服务，也允许其他 agent 通过 MCP 调用受控的 Scholium 能力。AI 只能通过公开的 agent/tool 合约读取项目或提交语义 Action；它不能直接改写 CRDT、WAL、源码文件或绕过团队源码语言 epoch。默认不上传文档，网络、模型和数据范围由用户逐次或按项目策略授权。

Pi 指用户直接接入的 Pi agent，不是 Provider Interface。第一版优先通过可选的外部进程适配器接入 Pi；具体发行来源、版本、启动命令、RPC/事件协议和许可证必须在 spike 中核实并锁定，不假定它原生支持 MCP。模型选择及对模型服务的调用由 Pi 管理，Scholium 负责上下文范围、工具权限、会话映射、取消、超时和结果校验。

Pi adapter 归一化启动、流式事件、工具请求、完成、取消和错误。Agent Gateway 是适配层内部的任务与策略边界，初期作为模块存在；只有出现第二个实际 agent 实现时才提取通用 trait/crate。不为首个接入预建多厂商 provider 框架。

Pi 在独立工作目录运行，仅获得显式提供的快照/附件和受控工具。不得将权威项目目录作为可写工作目录。必须验证能否禁用或隔离默认文件、shell 工具、扩展和自动加载配置，防止绕过 session；无法做到时只能交付只读分析/建议模式。取消必须终止或隔离后续工具回调，不能让旧任务继续提交。

Pi 若需要 Node.js，由用户安装的可选外部 agent 自行提供；不成为 Scholium 原生应用、核心或常规测试的依赖。Pi 专项集成验证单独运行，参见仓库开发规范的有限例外。

## 数据流

```text
原生 UI → Agent Gateway（Pi adapter 内部）→ 外部 Pi agent → 模型服务
                         │ 工具请求 / 编辑建议
                         ▼
其他 agent → MCP server → session ← plugin-host
                         │ 校验 / proposal 预览 / 确认或授权策略
                         ▼
                   普通 Action → history / CRDT / storage
```


AI 返回的编辑建议先作为 proposal，带 snapshot revision、来源、模型、工具调用和预计影响；只有用户接受或匹配用户显式配置的项目授权策略，并通过 session 的 Action 校验后才写入。生成文本、引用、代码和转换结果必须能标记来源，并可在历史中撤销或审计。

## MCP 服务器

Scholium 提供一个可选的 MCP server。第一版只暴露只读资源和显式 mutation tools：读取当前选择/诊断/格式能力、渲染或构建预览、提出结构编辑、提交已确认的 Action、查询任务状态。工具名称、输入输出 schema、错误码和版本号由 Scholium 维护；MCP session 不获得文件系统、shell、任意网络或原始数据库权限。

mutation tool 默认 `preview`，必须携带项目 ID、base revision 和幂等 key；actor 由认证会话绑定，源码写入另需有效语言 epoch 与许可；提交时再次检查 revision、权限、写集和冲突。长任务使用 task handle、进度事件和取消，不让 MCP 请求阻塞 UI。远程 MCP client 通过明确配置的 HTTP 监听地址、认证和来源白名单接入；本地 stdio 也必须经过同一授权层。

首个 MCP 交付方向是 Scholium 作为 server，供其他 agent 调用。Scholium 作为通用 MCP client 不列入首个出口。Pi 接入通过其经验证的工具协议桥接 session；是否经由 MCP 取决于锁定版本实际能力，不增加强制依赖。

MCP 传输采用 stdio 优先，远程使用经验证的 Streamable HTTP；不另造裸 TCP 协议。协议版本、认证与重连行为须在专项 ADR 中确定。

## 安全、隐私与可观察性

- Scholium 管理的凭据放在宿主安全存储；Pi 管理的模型凭据遵循锁定版本经验证的凭据机制，不复制到项目文件、工具结果或日志。项目只保存 agent/provider 标识和非敏感配置。
- 发送前按选择范围构造 context，支持字段/附件脱敏；日志默认记录 hash、大小、耗时和 token 统计，不记录正文。
- 每次请求记录 consent、policy revision、provider/model、输入范围、工具调用、结果和用户决定；用户可导出或删除审计记录。
- Pi、MCP 和模型故障必须显示可恢复诊断；超时、取消、重复提交和网络重试不能产生重复 Action。
- 不可信的 MCP/AI 输出走与源码 reconcile 相同的诊断和拒绝路径，不能以 Raw 保留掩盖未验证执行。

## 分阶段交付与验证

1. **Pi integration spike**：锁定 Pi 发行来源、版本和协议，验证真实进程的启动、会话映射、流式事件、工具桥接、取消、崩溃和重新连接。用协议录制/替身覆盖离线回归，但替身不能替代真实 Pi 验收；验证默认工具和配置无法绕过授权。
2. **AI proposal spike**：对结构编辑、源码诊断和引用建议验证 snapshot/revision、预览、接受/拒绝、撤销和敏感内容遮蔽。
3. **MCP loopback spike**：stdio 与本地 Streamable HTTP 各跑一套 schema/认证/超时/取消/幂等/epoch 冲突夹具；验证只读与 mutation 的权限隔离。
4. **多 agent 并发与远程模型**：再验证并发调用、断线恢复、预算、速率限制、审计留痕和用户可见授权；通过后才进入产品化 UI。

出口要求：任何 AI/MCP 写入都可追溯到普通 Action；旧 revision、错误 epoch、无权限 actor 和超出 context 的请求均被拒绝；离线时核心编辑和已有源码工作流仍可用。
