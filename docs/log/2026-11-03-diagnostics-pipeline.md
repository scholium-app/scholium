# T43：诊断数据通路

## 问题与定位

基线为 `origin/main`（实施时的精确 SHA 见证据），任务在 wt2 的
`ation_ciger/feat/diagnostics-pipeline` 完成，没有修改 app、build 或 transport/工具链发现代码。

审查中的 `\foo` 并非只存在于原始日志。锁定 rtex `531e2c6b32a09d33be58f6be1301fe8f793411cd`
在 LuaLaTeX 实时会话的后台整编中会发出错误：本地产品沙箱测试得到 `main.tex:3`、error、
`Undefined control sequence.`，layout 的 outcome 为 WithErrors，见 [真实 rtex 记录](evidence/diagnostics-pipeline/rtex-native.log)。
这证明 live 会话能取得错误，不表示段落实时引擎本身拥有完整诊断。

上游该提交 `session/passes.rs::deliver_layout` 先解析 capture log，发送 source=background 的 Diagnostics，
再发带 source_revision 的 LayoutUpdate；其 `session/mod.rs::Diagnostic` 包含 severity/file/line/message/context，
Diagnostics 事件却没有 revision。适配前 `protocol::Diagnostic` 仅反序列化 message，
`Supervisor::diagnostics` 逐条发字符串事件，丢失文件、行、级别与替换/清除语义。
app 的 `latex_workspace/worker.rs` 又把这些事件写入 Update::Error；
问题面板仍以 editor 的 Typst 诊断列表为来源。因此根因是适配信息丢失及 UI 通路未接入；本任务修复前者并交付后者所需接口。
原始实现与上述上游位置由 [定位证据](evidence/diagnostics-pipeline/provenance.txt) 固定。

## 实现与边界

model 增加可序列化的内核诊断、版本化完整列表、1-based Unicode scalar 行列、可选 UTF-8 范围、
结构 NodeId 和原日志位置；路径只表示项目相对资源，没有 UI/rtex 依赖。
LaTeX 的新日志解析器处理括号文件栈、错误的后续 l.N、LaTeX/宏包警告、未定义引用/引文和 box 信息，
不能映射的日志位置保留原 header 行与 byte range，不替换成主文件。

live actor 暂存无版本 background 诊断，与随后的布局版本配对；只接受已确认且等于当前版本的结果。
查询返回完整当前列表；独立整编结果可按版本合并，每个来源可清除，旧版本拒绝，重启丢弃未配对批次。
无版本 fast 引擎故障保持为 supervision 消息，不能安全附着到当前源码。
完整构建成功给出最后一轮日志的诊断，TeX 非零退出时返回带诊断、revision 和原日志的 FullFailure，保留旧预览语义。

Typst 将当前 World 的 span 映射到文件、行列和字节；准确包含于当前生成块范围时提供 NodeId。
常驻编译结果携带统一列表，`submit_generated` 接收完整 GeneratedSource；原 `submit` 保持兼容。
外部 `.typ` 的沙箱执行权限没有扩大。

新增依赖只有 latex → model，Cargo.lock 相应多一条 workspace 依赖。必须最小接触的公共装配文件为
latex Cargo.toml/lib.rs/live.rs/protocol.rs、model lib.rs、Typst lib.rs/compiler.rs，以及测试 mod.rs/fake-rtex/real.rs；
均属诊断接口、生产者或测试装配，没有进入其它 slot 明确拥有的代码。

## 验证

先保留缺失诊断接口的失败测试，以及包错误缺失行号、TeX 自动换行切分位置短语的失败复现，随后修复。
记录见 [初始失败](evidence/diagnostics-pipeline/tests-before.log)、
[包错误失败](evidence/diagnostics-pipeline/package-before.log)、
[换行失败](evidence/diagnostics-pipeline/wrap-before.log)。

真实 LuaLaTeX/XeLaTeX 日志由随附小工程生成，输入、命令、版本与许可说明在
[夹具说明](../../crates/scholium-latex/tests/fixtures/logs/README.md)。
CI 不安装 TeX；使用已提交日志、2048 组确定性任意 Unicode 输入、fake-rtex 事件链验证文件行号、修复清除和旧结果拒绝。
Typst 使用真实编译器验证当前源码行列、精确块身份及常驻 worker 的端到端结果。

本地显式运行 ignored 的 rtex 测试与 XeLaTeX/pdfLaTeX 整编失败→修复测试，均通过。
后者记录见 [整编验收](evidence/diagnostics-pipeline/full-native.log)。普通工作区测试中的其它既有 native 测试继续显式 ignored。
工作区测试 663 passed、15 ignored；系统字体与仅打包字体各 31/31 stock 对照逐像素相同。
全部共享门禁的逐命令状态与原始输出收录于 [门禁结果](evidence/diagnostics-pipeline/gates.json)。
额外验收命令记录见 [补充结果](evidence/diagnostics-pipeline/extras.json)。
窗口测试使用仓库 runner 的机器锁及私有 Xvfb，未访问用户桌面；中文/英文分别完整运行。

## 后续接线

问题面板后续批次消费 model DTO，并从 `LiveSession::diagnostics` 取得完整列表；整编读取
`FullPreview.diagnostics` 或 `LatexError::Compilation` 中的 FullFailure，再按当前版本发布。
Typst 原生预览调用方使用 `submit_generated` 传入既有映射；`.typ` 沙箱编译结果可经 `map_diagnostics` 适配。
UI 跳转、行标记、F8 与面板设计不在本任务内；这里没有声称审查截图中的面板已经修复。
