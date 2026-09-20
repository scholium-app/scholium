# scholium-sync-server

## 职责

管理账号/项目成员与邀请，鉴权 WebSocket/HTTP，请求级授权，持久化和转发幂等 CRDT updates，保存可用
快照与 branch refs，转发 presence，执行配额、审计和数据生命周期策略，并协调团队源码语言、许可及结构级写集校验。

## 非职责

不解析 LaTeX/Markdown/BibTeX，不决定合并结果，不生成用户 undo，不构建文档，不执行项目代码。

## 内部结构

```text
src/
├── api/            versioned HTTP/WS envelopes
├── auth/           session/token/device identity
├── membership/     owner/editor/viewer、invite、epoch
├── updates/        validate、dedupe、append、range query
├── snapshots/      upload/verify/select/retention
├── refs/           branch/checkpoint CAS metadata
├── presence/       ephemeral rooms、TTL、backpressure
├── blobs/          attachment hash upload/download
├── quota/          size/rate/member limits
└── audit/          metadata-only security events
```

## 存储模型

- update 表按 project/branch/actor/sequence 唯一，保存 hash、协议版本、字节数和 payload。
- snapshot 关联覆盖 state vector；服务器可以验证 hash/大小，但不需要执行文档语义；源码写入另经最小结构写集校验。
- branch ref 更新使用 expected-old CAS，避免两个客户端静默覆盖 head。
- presence 只在内存/短 TTL 存储，服务重启后允许消失。
- blob 按内容 hash 去重，ACL 继承项目，删除遵守保留期。

## 连接流程

1. 验证 token、device、project membership 和 permission epoch。
2. 协商协议与最大 payload，接收 client state vector。
3. 返回 snapshot 建议和缺失 update 范围。
4. 持续接收 update：校验、事务持久化、ACK、广播。
5. 慢消费者先丢 presence，再断开内容通道并要求基于 state vector 重连。

## 权限与离线

写入每次校验当前 epoch。撤权后的离线 update 返回 `permission_revoked`，同时允许客户端下载其已知基线
以创建本地 fork。服务器不能接受后再隐藏，避免客户端误认为已共享。

## 运维

数据库迁移向前兼容并可滚动部署。update payload 不写应用日志。指标包含连接、ACK 延迟、更新字节、
拒绝原因、快照命中和 presence 丢弃，不含正文。备份与删除恢复期限形成公开策略。

## 公共接口

- 版本化 HTTP/WebSocket 信封与消息集合见 [协议设计](../PROTOCOLS.md) 第 6 节；本模块不新增未记录消息。
- 服务端能力：鉴权与成员/邀请管理、project/branch CAS refs、update 校验与幂等持久化、快照上传与选择、
  blob 上传下载、配额、审计、源码语言控制与结构级写集校验。
- 对外承诺：ACK 表示已持久化；重复包返回原回执；viewer 永远不能写；presence 可丢弃。
- 运维接口：数据库迁移、公开的保留期与备份策略、脱敏指标。

## 不变量

- ACK 只在 update 事务持久化后发送。
- 重复 update 返回相同语义结果，不再次广播为新动作。
- viewer 永远不能上传 update、移动 refs 或上传 blob。
- 服务器不能把 presence 当可靠消息。
- 未实现 E2EE 前不声称 payload 对服务器不可见。

## 失败处理

- 鉴权、成员或权限 epoch 校验失败返回 `PermissionDenied`，错误与日志不含正文、token 或绝对路径。
- 重复包返回原回执；序号空洞请求补包；序号回退且 hash 不同判为身份冲突。
- 旧 epoch 的源码包返回 `SourceEpochStale` 或 `SourcePermitRequired`，写集不符返回 `SourceWriteSetMismatch`，客户端保留草稿。
- 事务未提交成功不得发送 ACK；重启从持久化控制记录恢复，不能因 presence 清空重新开放两种语言。
- 快照 hash/大小校验失败时拒绝并保留已有快照；配额与限流超限返回 retry 标志正确的结构化错误。
- 慢消费者先丢 presence，超限后断开内容通道并要求按 state vector 重连。

## 测试门禁

鉴权、epoch 竞争、重复/乱序、服务器重启、数据库故障、慢消费者、超额、恶意 envelope、branch CAS、
快照选择、撤权离线回归。协议兼容测试至少覆盖当前和前一公开客户端版本。

## 团队源码语言与混合项目合约

新增 source-control/ 子模块，按共享项目分支持久化活动语言、phase、单调 epoch、许可和 drain 回执。
切换 CAS 进入冻结阶段，处理旧语言接受序列/隔离后原子启用新 epoch；服务重启不能重用旧许可。
源码许可校验和 update 持久化在同一事务边界。已接受重复包先返回原回执；旧 epoch 的新包拒绝并提示草稿合入。
不只信任客户端 dialect：通过 collab 的受限结构校验核对实际资源/绑定/源码写集，阻断普通 UpdateBatch 绕过门禁。
该扩展不执行宏、不解析排版语义；其验证成本与协议兼容性是阶段 0 必须关闭的风险，不能继续宣称服务端纯中继。
离线编辑者不能阻止切换永久完成，但未确认修改保留在其草稿/fork；UI 显示隔离成员。控制状态不得进入 presence。
新增竞争切换、故障事务、服务恢复、许可伪造、撤权竞态、旧包和隐藏写集测试。

共同要求见 [混合源码与团队编辑](../MIXED_SOURCE_EDITING.md) 与 [ADR 0001](../adr/ADR-0001-mixed-source-team-editing.md)。
