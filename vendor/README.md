# Typst 编辑依赖

`typst-edit/` 由根 workspace 以完整同版本组 `[patch.crates-io]` 使用
（[ADR 0036](../docs/adr/ADR-0036-typst-default-editor.md)）。来源为 Typst v0.15.1 提交
`9dfd3a08500b7896045f907433cf7b4b02434fad`（Apache-2.0），保留目录内上游 LICENSE。

修改仅为 `spikes/typst-edit-session/patches/0001`–`0004` 四份补丁和上游 workspace 成员裁剪。
逐文件来源校验（从上游 Git 对象重建并比较，不接受未记账修改）：

```sh
git init /tmp/typst-origin
git -C /tmp/typst-origin fetch --depth 1 https://github.com/typst/typst.git 9dfd3a08500b7896045f907433cf7b4b02434fad
python3 scripts/verify-typst-fork.py /tmp/typst-origin
```

fork 与独立 stock Typst 的逐像素对照：

```sh
cargo run --locked -p scholium-typst --features editor --example typst-main-fork-check -- /tmp/fork-pixels
cargo run --locked --manifest-path spikes/typst-main-editor/reference/Cargo.toml \
  --target-dir target/stock-reference -- /tmp/stock-pixels
python3 spikes/typst-main-editor/compare.py /tmp/fork-pixels /tmp/stock-pixels
```

该 vendor 不批准绕过依赖安全通告：通告照常失败报告，不加 ignore。
