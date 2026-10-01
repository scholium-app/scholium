# Local structured session migration probe

跨模块夹具只调用正式 model/document/storage，不持有第二个编辑内核，不执行 Typst，
不读写主程序默认会话。夹具与探针按仓库 MIT OR Apache-2.0 双许可。

```sh
CARGO_TARGET_DIR="$PWD/target" cargo run --locked --manifest-path spikes/structured-session-migration/Cargo.toml -- /tmp/scholium-structure-evidence
```

目标目录必须为空。输出 legacy/candidate SQLite 文件与 stdout JSON：三个旧快照迁移，
两个原始公式 Raw 保留，随后新建数学、填写/包分数/清空必填槽，保存重开、单机快照撤销重做
及新的渲染 epoch。它不验证主程序窗口、协作撤销、执行旧公式或断电持久性。
报告见 [SPK-0053](../../docs/spikes/SPK-0053-structured-session-migration.md)。
