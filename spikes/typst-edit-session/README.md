# Typst 结构编辑内核探针

独立 workspace，不修改产品依赖或 registry。实施方向见
[编辑内核计划](../../docs/plan/TYPST_EDIT_KERNEL.md)，历史入口验证见
[报告 0049](../../docs/spikes/SPK-0049-typst-content-layout.md)，身份与空槽验证见
[报告 0050](../../docs/spikes/SPK-0050-typst-edit-origin-and-holes.md)。
组合身份、正文插入点与原生窗口验证见
[报告 0051](../../docs/spikes/SPK-0051-typst-caret-native-editor.md)。
持久 Content、增量段落与可见帧性能边界见
[报告 0052](../../docs/spikes/SPK-0052-typst-incremental-content.md)。

## K1 动手前判据

1. 分数分子/分母有稳定身份；空分母有非零面积并可命中。
2. 填槽后 Hole 状态消失，不打印额外占位字符/图形。
3. 相同内容不同 ID 不串缓存；重复请求与修改分母结果正确。
4. 嵌套分数、平移缩放后身份与坐标保留。
5. strict 布局拒绝未填 Hole，完整内容成功。
6. 完整/嵌套公式与独立 stock Typst 0.15.1 的同 Content 栅格图逐像素一致。

初始判据覆盖固定分数子集。后续验证增加嵌套槽位与分数线归属、正文 cluster/caret、
真实原生窗口操作；仍不覆盖完整数学树、选区、持久会话或分页。

## 固定上游与补丁

上游：https://github.com/typst/typst，tag `v0.15.1`，commit
`9dfd3a08500b7896045f907433cf7b4b02434fad`。Typst 上游及补丁中的上游代码按
Apache-2.0 使用；新探针代码按 MIT OR Apache-2.0。补丁维持上游原有许可证。

`bootstrap.py` 创建被忽略的 `.vendor/typst`，校验版本并应用 `patches/*.patch`。
已经应用的补丁会跳过；版本不符或有冲突会停止并保留现有文件。只在本探针的
Cargo.toml 使用 path patch；`reference/` 独立 workspace 使用未修改的 registry 依赖。
新增 `editor` feature 默认只在本探针启用，`Library.editing` 默认 false。
补丁依次为身份/Hole、组合槽位/分数线、shaping/空行插入点、段落实际计算计数；bootstrap 校验完整补丁前缀，
拒绝覆盖任何与这些状态不符的本地修改。路径补丁覆盖同一版本组，避免混用 Typst 类型。

```sh
python3 spikes/typst-edit-session/bootstrap.py
cargo run --release --locked --manifest-path spikes/typst-edit-session/Cargo.toml -- --editor
python3 spikes/typst-edit-session/check.py
```

最后一条需要 Python Pillow；自动保存断言日志与 stock/fork 图片到
`docs/spikes/evidence/SPK-0052/`，失败非零退出；可用 `--out <directory>` 更改。
两组图片在同一机器使用相同字体集合。
脚本默认复用 `spikes/render-latency/target`；可用 `CARGO_TARGET_DIR` 指定其他构建目录。
干净机器需要联网克隆上游与下载锁定依赖。

## 原生验证窗口

```sh
CARGO_TARGET_DIR="$PWD/spikes/render-latency/target" cargo run --release --locked \
  --manifest-path spikes/typst-edit-session/Cargo.toml --features native --bin typst-edit-window
```

点击空分母或正文直接输入，Tab 切换文本叶子，Ctrl+/ 在数学槽位嵌套分数，Ctrl+Z 撤销文字。
唯一可写状态来自既有 native-ui 验证核心。后台从可信结构构造 Content，不生成/求值源码；
同一结果携带 Typst 像素与 Frame 几何。pending 时保留标记为旧 revision 的图像，暂停旧几何
点击和光标；输入仍接受到语义模型。正文没有 egui TextEdit/临时回显，IME preedit 单独显示。
这不是生产应用开关，不支持保存、完整选区或分数结构撤销。

worker 持有派生 ContentSession。初始/重同步捕获整树，之后请求携带已接受语义 revision
的节点描述；文字编辑仅发送修改的 Text，包裹分数增加旧父和新子树。容量 1 的请求队列满时
合并连续更新，保留 base，worker 顺序应用；版本不连续时由权威模型重同步，不丢掉模型动作。
文字撤销更新原动作所属叶。缓存依节点局部版本与 text/math context 复用 Content；
像素与当前几何原子采纳的规则不变。旧无缓存投影保留为 benchmark/测试的整树 oracle。

```sh
export CARGO_TARGET_DIR="$PWD/spikes/render-latency/target"
cargo build --release --locked --manifest-path spikes/typst-edit-session/Cargo.toml \
  --features native --bin typst-edit-window
cargo test --release --locked --manifest-path spikes/typst-edit-session/Cargo.toml \
  --features native --bin typst-edit-window
xvfb-run -a -s "-screen 0 1280x900x24" python3 spikes/typst-edit-session/native-check.py /tmp/typst-native-check
```

最后一条需要 Xvfb、xdotool、ImageMagick，输出目录必须为空。脚本隔离 Wayland，固定 X11
缩放 1，并人为增加 200 ms 布局等待以验证 stale 点击门禁；不能用这个运行测性能。
合字内部 caret 使用标记为非精确的比例回退；跨叶合字、大小写改写不猜可写偏移。

## 增量与可见帧测量

```sh
export CARGO_TARGET_DIR="$PWD/spikes/render-latency/target"
cargo build --release --locked --manifest-path spikes/typst-edit-session/Cargo.toml \
  --features native --bins
"$CARGO_TARGET_DIR/release/typst-session-bench" /tmp/typst-content-benchmark.json
xvfb-run -a -s "-screen 0 1280x900x24" python3 spikes/typst-edit-session/present-check.py /tmp/typst-present-check
```

benchmark 每组 60 次，1/64/256 段首中尾共 540 次，断言每次实际计算 1 个 paragraph、
收到 1 个节点、与整树 Content/Frame 一致；首末样本像素对照。另测行宽/字号依赖失效。
它仍运行整个不分页 flow 与全 Frame 几何，不代表分页、字体族/资源更新或全文耗时解耦。

present-check 需要 Pillow、Xvfb、xdotool，目录必须为空；与其他验证任务顺序运行。
连续 30 秒 10 Hz，交替插入/删除；无人工 worker 延迟。测试开关在同帧画 revision 条码，
截图实际匹配后记录接受/worker 完成/UI 采纳/首次观测时间，包含截图成本。条码默认关闭。
输出 `app.log`、`samples.json`、`summary.json` 和最后截图；功能断言或 p95 > 32 ms 时非零退出。
报告 0052 的本机结果未过 32 ms，不能当作物理显示器 60 Hz 呈现证明；也不能把 worker
低耗时当作端到端达标。正式应用接入仍需完整 K3/K4 与依赖安全门禁。

## K0 公开入口探针

历史版本 `fb62d37` 未修改 Typst：直接构造中英文与分数 Content，空字符串分母和填入
分母均布局成功，World::source 调用为零；重复指纹稳定，并记录 40 次暖态布局与 Frame
遍历耗时。它没有 Hole、精确 caret、GUI 或 GPU 验证。当前未修改 Typst 的复现入口为：

```sh
cargo run --release --locked --manifest-path spikes/typst-edit-session/reference/Cargo.toml
```

此命令重跑公开入口夹具，不保证不同机器复现历史耗时。
