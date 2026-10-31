# 2026-10-31：Typst fork 跟进上游（试 rebase 与每周 drift job）

本记录说明这次做了什么：把 fork 的四份补丁试 rebase 到上游 Typst main，并新增每周 drift 检查。规范写在
[scholium-typst 上游跟进](../modules/typst.md#上游跟进)，fork 的来源规则见 [ADR 0036](../adr/ADR-0036-typst-default-editor.md)。
原始证据放在 [evidence/typst-upstream-tracking](evidence/typst-upstream-tracking/)。这次只做了内部维护，没有向 typst/typst 提交任何东西，
`vendor/typst-edit`、四份补丁和锁定基线都没有改。

## 结论

| 项 | 结果 |
|---|---|
| 上游 | typst/typst main `84cca406aafe7f830ed3f37f1cc6f0c8b6b65a03`（2026-10-28，“Bump `krilla` and `hayro` (#9022)”），比基线多 224 个提交 |
| 基线 | v0.15.1 `9dfd3a0`（2026-07-17）在 `0.15` 发布分支上，不是 main 的祖先；与 main 的分叉点是 `50d2220`（2026-06-09），发布分支另有 35 个提交（多数是从 main 拣选过去的修复） |
| 补丁 | 0001 冲突，冲突点只有 `typst-library/src/lib.rs` 的两处，都是简单冲突；0002 直接应用；0003 由三方合并自动解决；0004 直接应用 |
| adapter | `scholium-typst --features editor` 不能编译：两项上游 API 变化，6 处错误，全在我们的代码里；打了补丁的 Typst 库本身编译通过 |
| 像素 | 在临时副本里移植 adapter（4 个文件，未进仓库）后，fork-check 的 4 个夹具与同一提交的未打补丁上游 RGBA 逐字节相等；与 v0.15.1 的输出也逐字节相同 |
| 测试 | 同一临时副本 `cargo test --workspace` 442 个通过、2 个忽略、0 个失败 |
| 规模 | 补丁相对上游净新增 592 行、删除 16 行（21 个文件）；rebase 后不变；四份补丁各自的新增加起来是 595 行 |
| 用时 | 从克隆到补丁序列 rebase 完成约 3 分钟（解决冲突只用了 1 分钟）；加上编译定位、临时移植、像素比较和全量测试，约 15 分钟 |

## 补丁逐份结果

先用 GNU patch 把补丁原样依次打到 main 的树上（[apply-gnu-patch.txt](evidence/typst-upstream-tracking/apply-gnu-patch.txt)），
再在上游克隆里把四份补丁作为提交放到基线上，用 `git apply --3way` 依次重放到 main 上。

| 补丁 | 新增/删除 | 结果 | 上游原因 | 处理 |
|---|---|---|---|---|
| 0001 structural math origin and holes | 192/0 | **冲突**：`typst-library/src/lib.rs` 第 2、3 个 hunk 冲突，第 1 个 hunk 需要 fuzz 2；`math/ir/resolve.rs` 第 2 个 hunk 需要 fuzz 1，三方合并能自动解决 | lib.rs：`c0b6245e4` “Set rules for default document format options (#8496)” 给 `Library` 结构体和 `LibraryBuilder::build` 的末尾加了 `formats` 字段，并把 `pub mod pdf` 改成 `pub mod format`，而补丁在同一位置加了 `editing` 字段。resolve.rs：`b0693a74b` “Revise and expand clippy configuration (#8539)” 把函数签名改成省略生命周期 | 两边的字段都保留：先放上游的 `formats`，后放补丁的 `#[cfg(feature = "editor")] editing`，行为不变 |
| 0002 composite slots and fraction rule owners | 95/10 | 直接应用（只有行号偏移） | — | — |
| 0003 shaped-text caret geometry | 283/9 | `git apply` 严格应用失败，三方合并成功（`inline/collect.rs` 第 1 个 hunk 需要 fuzz 1） | `b0693a74b` 把 `collect()` 里 `if … { … };` 末尾的分号删掉了。同一文件里 `20cb02ed8`（#8570）新增的 `Item::Indent` 不影响补丁：补丁按 span 和全文字节偏移工作，不匹配 `Item` 变体 | 用自动合并的结果 |
| 0004 editor paragraph computation counters | 25/0 | 直接应用（偏移 -3 行） | — | — |

rebase 后的补丁序列放在 [rebased-84cca406/](evidence/typst-upstream-tracking/rebased-84cca406/)。它能用 `git apply` 严格应用到 main，
得到的 13 个库与试验树逐文件一致。它只是证据，构建不使用。

## adapter 编译失败清单

试验方法：不改仓库文件，用 `cargo --config <file>` 传入 `[patch.crates-io]`，让 13 个库指向 rebase 后的树
（配置文件里的 patch 优先于 `Cargo.toml` 里的）。完整输出见 [adapter-errors.txt](evidence/typst-upstream-tracking/adapter-errors.txt)。

| 位置 | 错误 | 上游原因 |
|---|---|---|
| `scholium-typst/src/lib.rs:73`、`editor/kernel.rs:45` | `Library::default()` 不存在 | `c0b6245e4`（#8496）：`LibraryExt` 改为 `Library::new(formats)` / `Library::builder(formats)`，必须传入导出格式列表 |
| `scholium-typst/src/compiler.rs:249`、`editor/kernel.rs:170` | `RenderOptions` 没有 `pixel_per_pt` 字段 | `c0b6245e4` 把它改成 `RenderOptions<F> { render_bleed, format: PngFormatOptions<F> }`；`fddf6e1fb`（#8903）把分辨率改成 `ppi`，`render` 按 `ppi / 72` 换算 |
| `editor/raster.rs:215`、`:248` | 需要 `&RenderOptions<Complete>`，传入的是 `&RenderOptions` | 同上：`render` 现在只接受解析完的 `Complete` 选项 |

另外，spike 的 `spikes/typst-edit-session/src/kernel.rs`（stock 参照通过 `#[path]` 引用它）有同样的两处问题。
临时移植的内容见 [adapter-port.diff](evidence/typst-upstream-tracking/adapter-port.diff)：`Library::new([typst_render::FORMAT])`，
以及 `RenderOptions::<Complete> { render_bleed: false, format: PngFormatOptions { ppi: SpannedValue::detached(Scalar::new(px_per_pt * 72.0)) } }`。
固定 2 px/pt 对应 144 ppi，换算是精确的。`compiler.rs` 预览会把 DPR 限制到 0.25–8 之间，小数倍率经过 f32 的 `ppi / 72` 后可能差最后一位，
真正移植时需要核对。格式列表选哪些也要在移植时决定。`typst-pdf` 不在我们的 13 个库里，pdf 模块已经从 `typst-library` 移到了那里。

## 像素与测试

在临时副本里，fork-check 示例用 rebase 后的树运行；stock 参照用 `spikes/typst-main-editor/reference`，只把 13 个库指向同一提交未打补丁的树。
随后用 `compare.py` 比较。full、scripts、styles、heading 四个夹具尺寸相同，RGBA 逐字节相等（[pixels-main.json](evidence/typst-upstream-tracking/pixels-main.json)）。
这四个哈希与 crates.io 0.15.1 的 stock 参照也完全相同，说明这段时间上游在这些夹具上没有排版变化。
同一副本的 `cargo test --workspace` 全部通过：442 个通过，2 个忽略，其中 scholium-typst 自身 35 个。真实窗口检查没有对试验树运行。

## drift 脚本与 workflow

- `scripts/typst-upstream-drift.py <upstream-clone> [--ref main]` 按三个阶段运行，前一阶段失败就不再继续：
  1. 补丁：先在克隆的临时 git worktree 里把补丁作为提交放到基线上，给三方合并提供原始 blob，再依次重放到目标提交上。
     结果分为 clean、merged、conflict；出现冲突后，后面的补丁标为 not tried，并列出冲突文件和上游提交（不含已拣选进基线的）。
  2. 编译：`cargo --config` 指向打了补丁的树，`check -p scholium-typst --features editor --examples`。
     错误按位置分别计数：在 Scholium 代码里的，和在打了补丁的树里的。
  3. 像素：分别运行 fork-check 和 stock 参照，再运行 `compare.py`。

  退出码 1/2/3 表示失败的阶段。构建产物放在 `target/typst-upstream`，源码树和日志放在仓库外（默认在系统临时目录）。
  这是因为仓库根目录下的 path crate 会被当成 Scholium workspace 的成员，继承 `workspace.package` 时会出错，第一次本地运行就遇到了这个问题。
  两个 Cargo.lock 在每次 cargo 调用后都会恢复原样。
- `scripts/verify-typst-fork.py` 只做了重构：常量和缩减 workspace 的函数供 drift 脚本导入，并在上游 workspace 成员行变化时直接报错。
  它对固定基线的输出与改动前逐字节相同。
- `.github/workflows/typst-upstream.yml`：每周一 03:17 UTC 运行，也可以用 `workflow_dispatch` 手动触发（输入任意上游 ref）。
  它运行在 `ubuntu-26.04` 上，复用 `.github/actions/setup`（缓存键 `typst-upstream`），在 job summary 里写结果表，并上传日志 artifact。
  它不在 push 或 PR 上运行，也不开 issue。

本地端到端运行了三次。对 main 运行，在阶段 1 失败（退出码 1），冲突行指向 `c0b6245e4`，见 [drift-main.md](evidence/typst-upstream-tracking/drift-main.md)。
对基线 `9dfd3a0` 运行，三个阶段全部通过（退出码 0，约 80 秒，[drift-base.md](evidence/typst-upstream-tracking/drift-base.md)）。
阶段 2 的失败分支是单独调用验证的：直接用 rebase 后的树调用编译阶段函数，报告 6 个错误、全在 Scholium 代码里，与上表一致。
这意味着在 0.16 移动完成前，每周的 job 都会停在阶段 1。结果表能看出冲突有没有扩散到别的文件或补丁，但后两个阶段要等补丁与 adapter 移到新基线后才会再运行。

## 0.16 移动的工作量估计

按这次的结果，移动到 0.16 时需要做这些事：

1. 补丁 rebase：今天只有一处简单冲突。如果到发布前上游再改动 `math/` 或 `inline/`，预计 0.5–2 小时。
2. adapter 移植：两项 API 变化，3 个文件，外加 spike kernel；还要决定格式列表和小数 DPR 的换算。约 1 小时。
3. 版本相关改动：
   - `scholium-typst` 与 stock 参照里的 `=0.15.1` 版本约束、两个锁文件；
   - `verify-typst-fork.py` 里的提交和标签，以及重新生成的 vendor 树；
   - `cargo deny`。注意：main 现在用 git 依赖（typst-assets、codex、hayro、krilla），正式发布时应当会换回 crates.io，这也是基线只钉发布版的原因之一。
4. 全部门禁和两种语言的真实窗口检查。

合计约一个执行者一天，前提是发布版相对今天的 main 没有大的结构变化。
