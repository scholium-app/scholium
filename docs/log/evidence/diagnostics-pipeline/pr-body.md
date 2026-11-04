## 问题

审查中的 LaTeX `\foo` 没进入问题面板。锁定 rtex 的 live 会话后台确实报告错误，但适配器丢失 severity/file/line，应用又把消息送到工作台错误状态，问题面板仍读取 Typst editor 的列表。整编失败也没有可查询的结构化诊断。

## 改动

- model 增加可序列化、无 UI 的统一诊断及版本化列表，明确项目相对路径、1-based Unicode scalar 行列、可选 UTF-8 范围、NodeId 和原始日志位置。
- LaTeX 接入 rtex 文件/行号，把无版本后台诊断与后续布局版本配对；LiveSession 提供当前列表查询与整编结果合并，拒绝旧版本并支持修复清除。
- 实现 TeX 日志文件栈、错误、宏包/引用警告与 box 信息解析；整编成功和失败均保留版本化诊断，失败保留原日志。
- Typst 编译结果携带统一诊断；当前 World 的 span 提供文件、行列、字节，已知生成块提供准确 NodeId。
- 更新三个模块规范、开发日志，提交真实 LuaLaTeX/XeLaTeX 日志夹具与诊断验收证据。

最小共享装配改动：Cargo.lock、latex Cargo.toml/lib.rs/live.rs/protocol.rs、model lib.rs、Typst lib.rs/compiler.rs，以及诊断测试装配。未修改 app、build、transport、工具链发现或脚本。

## 验证

- 工作区测试 663 passed、15 ignored；完整共享门禁逐项见 `gates.json`，中英文窗口 54/54 均有通过记录，见 `window-summary.json`；原始轮次和复验日志均保留。
- 本地显式运行真实 rtex 沙箱：`Undefined control sequence.` 出现在 `main.tex:3`。
- XeLaTeX/pdfLaTeX 沙箱整编失败保留 `main.tex:5` 与 revision，修复后成功出图。
- CI 使用 fake-rtex 与真实日志夹具；任意 Unicode 输入属性测试覆盖 2048 组。
- 系统字体、仅打包字体两套 stock Typst 对照各 31/31 逐像素相同。
- 窗口首次被外部全局 Xvfb 清理中断；最终受影响项在同一 runner/机器锁下使用 byte-identical Xvfb 的专用进程名复验 8/8，通过且未改断言，临时包装不提交。
- 保留测试先失败、后修复的记录；无新增外部 crate 或 TeX 安装依赖。

## 待办

本任务交付生产者与公共诊断结构。问题面板、源码行标记与 F8/跳转由后续 UI 批次接线：LaTeX 查询 `LiveSession::diagnostics()`，整编读取成功结果或 FullFailure；原生 Typst 预览调用 `submit_generated` 传递当前 GeneratedSource 映射。不能安全关联版本的 fast/standby 引擎故障保留在 supervision 消息中，不猜测源码位置。
