# 可移植文档字体与无损压缩

## 实现与来源

本次任务基于 `origin/main` 的 `7de292e592e9c63d8953cac9d8bbfd58fc84cc38`，在指定 wt1 slot 完成。
从 Liberation 官方 2.1.5 发行包取得四个未经修改的 Serif 字体：Regular、Italic、Bold、BoldItalic。
保留原始字节、字体名称及完整 OFL 许可；没有子集化，也没有提交 Microsoft 字体。
字体来源、版本、原始文件 SHA-256 与压缩再生成命令记在[字体资产说明](../../crates/scholium-app/assets/fonts/README.md)。

正文顺序改为 Times New Roman → Liberation Serif → SimSun → Noto Sans CJK SC，标题将 SimSun 换为 SimHei。
`style.rs` 保持唯一列表定义，编辑 Content 与生成 preamble 共用。System 与 Bundled 都载入四种 Liberation、
Noto 及 Typst embedded；只有 System 扫描宿主字体。首次离线启动不下载字体。

仓库只提交五个 `.br` 无损容器，移除原始 Noto OTF，不提交四个原始 Liberation TTF。
纯 Rust `brotli-decompressor 6.2.0` 在首次加载时解压，由进程级 `OnceLock` 缓存；
界面经 `bundled_cjk_font_bytes()` 与 Typst 共用 Noto 字节。
协调者补充的 stock reference 独立解压五个 `.br` 并逐个断言 README 记录的原始 SHA-256；
该路径不调用 adapter 的解压实现。相关 reference 清单和锁文件改动均予保留。

## 断行与分页实测

新回归测试以旧字体加载实现运行时因缺少 Liberation Serif 失败；修改加载策略后通过。
最终 fixture 含中英混排正文、标题、粗体、斜体、行内数学，使用自然分页，共两页、44 行（32 + 12）。
Bundled 连续两次布局记录的每行第一个源 UTF-8 byte 完全一致；去掉注入 preamble 后的偏移如下：

```json
[
  [2, 36, 132, 253, 321, 417, 538, 606, 702, 823, 891, 987, 1108, 1176, 1272, 1393, 1461, 1557, 1678, 1746, 1842, 1963, 2033, 2067, 2163, 2284, 2352, 2448, 2569, 2637, 2733, 2854],
  [2922, 3018, 3139, 3207, 3303, 3424, 3492, 3588, 3709, 3777, 3873, 3994]
]
```

| 本地比较（同一 fixture） | 实测结果 |
| --- | --- |
| Noto Serif CJK SC 对 bundled Noto Sans CJK SC，保持 Liberation Serif | 两页的全部 44 个行首 byte 相同 |
| Times New Roman 对 Liberation Serif，保持 bundled Noto Sans CJK SC | 两页的全部 44 个行首 byte 相同 |
| 无宿主字体的 Fontconfig 配置，Bundled profile | 同一 golden 通过 |

本机确实安装了两种 Noto 及 TNR；测试同时检查所要求字体实际出现在 Frame 的文字项中，避免以静默 fallback 冒充比较。
两项依赖宿主字体的比较是显式执行的 ignored 测试，不成为普通 CI 的宿主字体要求。
上述结果只描述该 fixture；不同字体的外观及未覆盖字符不由此推断一致。

## 压缩与 release 体积

先测量原始五个字体共 17,953,440 bytes，再决定采用压缩：

| 无损方式 | 压缩后 bytes | 节省 bytes |
| --- | ---: | ---: |
| zstd 1.5.7，`-19` | 12,749,228 | 5,204,212 |
| zstd 1.5.7，最高级别 `--ultra -22` | 12,636,017 | 5,317,423 |
| Brotli 1.2.1，最高质量 `-q 11` | 11,833,855 | 6,119,585 |

Brotli 节省约 6.1 MB（5.836 MiB），超过任务的 4 MB 门槛，因此采用。
四个 Liberation 原始字节共 1,516,076 bytes，其 `.br` 容器共 690,696 bytes。
`brotli-decompressor` 及其两个分配器依赖为纯 Rust，未启用 FFI 或 unsafe feature，无 C 构建依赖；
`cargo deny --locked check licenses bans sources` 通过。
`bundled_brotli_assets_restore_recorded_bytes` 验证五个解压结果与记录的原始 SHA-256 全部相等。

相同 `cargo build --locked --release -p scholium-app` 命令下，
`target/release/scholium-app` 从 115,230,864 bytes 降为 94,419,256 bytes（115.2 → 94.4 MB），
减少 20,811,608 bytes。此下降大于压缩本身的 6.1 MB，原因是中文字体此前被界面与 Typst 各嵌入一次，
本次改为只嵌入一份压缩容器，再共享解压字节。

## 门禁与独立参照

- `cargo fmt --all -- --check`、`cargo clippy --locked --workspace --all-targets -- -D warnings` 通过。
- `cargo test --locked --workspace`：448 passed / 4 ignored；两项本地字体比较另行执行，2/2 通过。
- `RUSTDOCFLAGS='-D warnings' cargo doc --locked --workspace --no-deps` 通过。
- `check-rust-size.py`、`check-doc-links.py`、`check-ui-strings.py`、`check-layers.py` 全部通过。
- `cargo deny --locked check licenses bans sources`、`cargo build --locked -p scholium-app -p scholium-storage --bins` 通过。
- `python3 scripts/window-checks.py --locale zh-Hans en`：最终状态整轮重跑双语 30/30 通过；
  等待机器级锁后在私有 Xvfb 执行。第一轮同样 30/30。
- 按 `.github/workflows/typst-fork.yml` 重建 pinned fork，438 个文件与四份补丁重建结果一致。
  stock 像素对照在 System 下 4/4 相等，在 Bundled 下另 4/4 相等；没有放宽任何阈值。

独立 stock 参照的字体列表与输入随字体策略更新；协调者将 reference 改为独立解压和哈希核验后，重新生成了对照输出。
本次按任务验证要求额外修改 `spikes/typst-main-editor/{stock-reference.rs,reference/Cargo.toml,reference/Cargo.lock}`，
并给共用的 `spikes/typst-edit-session/src/kernel.rs` 增加接收 FontStore 的四行构造入口。
这些是维持独立参照的必要改动；未触碰 wt2 负责的 source-view/i18n/字符串检查文件。
原始下载与早期测量临时文件已收拢至 slot 的 `target/`。

完整窗口截图、AUDIT 与构建日志保留在忽略的 `target/t23-*`；摘要证据随本日志提交。

证据：[全部门禁的命令、退出码与环境](evidence/2026-11-01-portable-fonts/gates.json)、
[workspace 测试摘要](evidence/2026-11-01-portable-fonts/workspace-test-summary.txt)、
[最终双语窗口摘要](evidence/2026-11-01-portable-fonts/window-summary.txt)、
[两项本机字体比较原始输出](evidence/2026-11-01-portable-fonts/installed-font-comparisons.txt)、
[无宿主字体配置的 golden](evidence/2026-11-01-portable-fonts/bundled-isolated.txt)、
[压缩统计](evidence/2026-11-01-portable-fonts/compression.json)、
[release 体积](evidence/2026-11-01-portable-fonts/release-sizes.json)、
[System stock 对照](evidence/2026-11-01-portable-fonts/stock-comparison.json)、
[Bundled stock 对照](evidence/2026-11-01-portable-fonts/bundled-stock-comparison.json)、
[fork 重建摘要](evidence/2026-11-01-portable-fonts/fork-source-summary.json)。
