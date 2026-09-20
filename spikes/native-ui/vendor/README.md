# 原生无障碍补丁依赖

仅 candidate-egui 通过 `[patch.crates-io]` 使用。依据 ADR 0021，不修改 Cargo registry cache。
原始 `.crate` 发布包来源 crates.io（本地 rsproxy 缓存），上游源版本如下：

| 副本 | 上游提交 | 发布包 SHA-256 |
|---|---|---|
| egui 0.36.2 | `emilk/egui@49682f8baa058bf49e011035cfbd6e825f88a5ef` | `dc938cc27cd911415e1e4d151bc56ff9df063d8db081374e87aeb805ea74b2ba` |
| accesskit_atspi_common 0.18.1 | `AccessKit/accesskit@f40dfc01a0c0e76de535969f82fb35e19513737d` | `1e8c61bee90b42a772d39d06a740207dc71a4e780004ace1db8d99fb1baaa954` |

两者 MIT OR Apache-2.0；许可证从对应固定上游提交补齐，AccessKit 同时保留 LICENSE.chromium。
删除仅属下载/解析缓存的 `.cargo-ok`、库自身 Cargo.lock，保留原始 Cargo.toml.orig 和 VCS 信息。

本地源码差异只有：

- egui/src/text_selection/accesskit_text.rs：TextRun 改为局部 bounds，由透明 GenericContainer 的
  affine transform 承担平移和缩放；ID、文本顺序与选区端点不变。
- accesskit_atspi_common/src/node.rs：disabled 节点不再发布 Enabled/Sensitive。

上游文件按原组织保留以便审查；不把第三方源码当作项目自行设计的大模块继续扩写。
升级时从上述发布包比较全部文件，只允许已说明差异；运行 source_ui 回归、完整原生验收和许可证门禁。
