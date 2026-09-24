# scholium-model

## 职责

定义跨模块稳定语言：typed IDs、项目与权威模式、revision、Tree/Source 位置、诊断、Intent、SemanticEdit、
DocumentPatch、转换 IR、目标能力和协议无关错误码。

## 非职责

不读写文件，不启动任务，不解析具体语法，不知道具体 CRDT，不保存 UI 状态，不依赖桌面框架。

## 内部结构

```text
src/
├── ids.rs          typed IDs 与内容 hash
├── project.rs      Project/Resource/EntryPoint
├── position.rs     TreeAnchor/EditorRange/SourceRange/RelativeAnchor DTO
├── edit.rs         Intent/SemanticEdit/DocumentPatch/TextPatch/WorkspacePatch
├── ir/             block、inline、math、citation、raw nodes
├── diagnostic.rs   code、severity、location、fix
├── capability.rs   节点与格式能力
└── error.rs        稳定错误分类
```

## 公共接口

- 构造和校验相对项目路径、typed ID、revision-bound range。
- 校验 TextPatch 有序、不重叠、位于同一 revision。
- `WorkspacePatch::validate` 保证跨文件 patch 没有重复资源或重叠替换。
- IR visitor/folder trait，避免下游直接匹配所有内部字段。
- Diagnostic builder 强制 code、primary location 和 severity。

## 不变量

- 裸路径不能逃逸项目根；绝对路径不进入可同步模型。
- `SourceRange` 必须附带 `ResourceId + RevisionId`。
- 语义图/IR 的 Raw 节点必须携带原格式、作用域和原始内容。
- `TextPatch` 不允许通过排序“修复”重叠输入，发现即报错。
- 公共 enum 序列化时有稳定显式 tag，新增 variant 不复用旧编号。

## 失败处理

构造期尽早拒绝 invalid ID、路径、range、patch 和 schema。错误不包含正文；需要显示片段时由 app
在用户侧按 Location 取值。

## 测试门禁

Unicode byte/UTF-16 转换边界、路径逃逸、patch overlap、IR raw 保留、schema 向前读取、所有 public
enum round-trip。该 crate 目标行覆盖率不低于 90%。

## 团队源码语言与混合项目合约

新增 SourceEditControl、SourceWritePermit、SourceWriteSet、SourceDraft、ForeignSource、LayoutProfile 和
ExportRequest 的领域 DTO；具体字段见 [数据模型](../DATA_MODEL.md)。AuthorityMode 不再携带 publish_target。
控制 epoch 不进入正文 CRDT/可回退历史；组件原文引用唯一资源。纯语义与源码写集必须可区分，不能信任 UI 标签。
能力报告分别描述保真度、执行途径、编辑性、可移植性及 unresolved 原因。新增写集/schema/引用合约测试。

共同要求见 [混合源码与团队编辑](../MIXED_SOURCE_EDITING.md) 与 [ADR 0001](../adr/ADR-0001-mixed-source-team-editing.md)。

## 基础段落接入边界

最小实现提供 DocumentId/NodeId/RequestId、Revision、DocumentSnapshot 与 ReplaceParagraph，均无持久化 schema。
详见 [ADR 0024](../adr/ADR-0024-local-paragraph-integration.md)。

## 本地块编辑位置

`BlockPosition { block: NodeId, byte: usize }` 表示规范 `Block::markup_text()` 中的 UTF-8 字节偏移，
不表示屏幕像素、字符序号或持久协作锚点。`BlockEdit::ReplaceRange { start, end, text }` 将同一
revision 下的有序范围原子替换，可跨块。请求仍携带文档身份、base revision 和唯一 request id。
其阶段边界见 [ADR 0029](../adr/ADR-0029-direct-page-editing.md)。
