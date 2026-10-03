# 主程序 Typst Content 开发候选

该 manifest 编译真正的 `scholium-app` 源码，统一使用跟踪的 Typst fork。
普通仓库根构建仍使用 stock Typst。选项只在此开发构建中可用，**不替换默认后端**。

在仓库根执行，显式选择一个隔离的新文件路径：

```sh
SCHOLIUM_SESSION_FILE=/tmp/scholium-typst-candidate.sqlite \
  cargo +1.96.0 run --locked --manifest-path dev/typst-editor/Cargo.toml \
  --bin scholium-app -- --typst-editor
```

已有路径必须是结构 schema v1；旧库/损坏库会拒绝启动。自动迁移不属于这个入口。
保存可保留空数学槽，重开时 IDs 和请求日志不变。

`$` / Ctrl+M 进入或离开公式；Ctrl+/ 把当前数学叶包成分数，光标进入空分母；Tab /
Shift+Tab 切换叶/槽位；Ctrl+Z / Ctrl+Shift+Z 撤销/重做；Ctrl+S 保存。
候选采用受控不分页 flow，Source 按钮显示只读结构 JSON。整叶格式、正文 Enter 拆段可用；
跨叶选区、复制剪切、多行粘贴、旧 RawMath 转换、分页和实机输入法/无障碍仍待后续验收。
RawMath 不执行，布局明确报错；失败不切回 egui 正文回显。

```sh
cargo +1.96.0 test --locked --manifest-path dev/typst-editor/Cargo.toml
cargo +1.96.0 clippy --locked --manifest-path dev/typst-editor/Cargo.toml --all-targets -- -D warnings
xvfb-run -a -s '-screen 0 1280x900x24' \
  python3 spikes/typst-main-editor/native-check.py \
  dev/typst-editor/target/debug/scholium-app /tmp/scholium-native-evidence
```

窗口脚本使用 Xvfb、xdotool、xclip、ImageMagick 与标准 X11 关闭消息；结果目录必须为空。
显式 `SCHOLIUM_EDITOR_AUDIT=1` 才记录测试正文/几何，日常启动不记录这些内容。
脚本只验证原生窗口行为，不能作为物理 60 Hz 延迟或真实中文输入法的证明。

来源校验不读取上游 checkout 的工作区改动：

```sh
python3 dev/typst-editor/verify-fork.py /path/to/pinned/typst/git/checkout
```

完整来源、独立 stock 对照、依赖通告与边界见
[报告 0054](../../docs/spikes/SPK-0054-typst-main-app-candidate.md)、
[ADR 0034](../../docs/adr/ADR-0034-typst-main-app-candidate.md)。
