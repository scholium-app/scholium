# Typst 编辑开发依赖

`typst-edit/` 只由 `dev/typst-editor/Cargo.toml` 的完整同版本 patch 组使用。
根 workspace 不使用这些源码。来源为 Typst v0.15.1 提交
`9dfd3a08500b7896045f907433cf7b4b02434fad`（Apache-2.0），保留目录内上游 LICENSE。

修改仅为 `spikes/typst-edit-session/patches/0001`–`0004` 四份补丁和上游 workspace 成员裁剪；
来源逐文件校验命令与使用边界见 [开发入口](../dev/typst-editor/README.md)。
该 vendor 不批准绕过依赖安全通告或默认发行门禁。
