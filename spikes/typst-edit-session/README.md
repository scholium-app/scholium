# Typst 结构输入布局入口探针

对应阶段 0 第 1/2 项：结构编辑与 Typst 映射。报告见
`docs/spikes/SPK-0049-typst-content-layout.md`，实施方向见
`docs/plan/TYPST_EDIT_KERNEL.md`。

动手前的限定判据：

1. 直接构造包含中英文和分数的 Typst Content，经公开 layout_frame 得到非空 Frame；
   World::source 的实际调用次数为零。
2. 分母为空和填入分母两种状态均成功布局，填入后布局指纹改变。
3. 再次排版同一 Content 得到相同指纹；收集字形和分数线条，记录 detached span。
4. 记录 40 次不同修改的布局与 Frame 遍历耗时；性能仅作观测，无达标承诺。

本探针没有修改 Typst，没有页面合成、精确 caret、空槽几何、源码对照、GUI、GPU 或沙箱验证。
它只验证可用切入点，不证明编辑内核已经完成。夹具 Content 由本程序固定构造。

```sh
cargo run --release --locked --manifest-path spikes/typst-edit-session/Cargo.toml
```

