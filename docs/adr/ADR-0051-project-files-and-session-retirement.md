# ADR 0051：项目入口、语言装配与旧会话退役

- 状态：Accepted（项目与文件命令接入；不包含完整 Source Studio）
- 日期：2026-11-03
- 影响模块：storage、session、app
- 关联：[ADR 0049](ADR-0049-formal-project-storage.md)、[ADR 0041](ADR-0041-separate-latex-typst-kernels.md)、
  [ADR 0033](ADR-0033-structured-local-session-migration.md)、[文件操作](../MIXED_SOURCE_EDITING.md)

## 背景

正式项目存储与旧 SessionStore 是两个格式。继续用会话文件启动会使正式项目锁、Action、草稿与源码物化
只停留在库中；把普通 `.tex` / `.typ` 导入原生结构又会隐式切换权威。项目语言还必须在打开时决定工作区，
不能先启动 Typst 再把 LaTeX 当作其输入。

## 候选方案

1. 继续以 SessionStore 为默认权威，项目数据库作备份：产生两个可写正文，拒绝。
2. 按扩展名把一切转成结构正文：破坏源码权威与固定语言，拒绝。
3. 持久项目声明区分语言和 authority；各内核接同一存储底座：采用。

## 决策

### 声明与入口

项目声明为 `.scholium/project.toml`，信封 `format = "scholium.project"`、`version = 1`。
包含独立随机项目 ID、资源 ID、项目语言、authority 与项目内相对入口路径。路径不是身份。
创建/导入以正式项目锁串行化声明发布；声明使用 fsync、hard_link 不覆盖发布和目录同步。
不识别的声明/语言/authority 或不安全路径直接拒绝，不尝试改写成当前版本。

新建 Typst 是原生结构权威，进入现有页面编辑；新建 LaTeX 是 `.tex` 源码权威。
普通 `.typ` / `.tex` 按明确入口导入，源字节不变；目录只有一个候选入口时自动选择，
多入口或无入口时要求指定相对入口。已有声明不能借打开另一扩展名改变项目语言。
同格式另存为复制当前内容和普通资源，排除 `.scholium` 私人历史/草稿，重新生成项目与资源 ID；
已有目标、原项目内部目标、symlink 和特殊资源拒绝，失败目标保留。无跨语言另存为或 PDF 导出。

### Action 和 checkpoint 的过渡格式

`ProjectStore` 的永久 schema 不变。原生 payload 为 `scholium.temporary.native` version 1 JSON，
带独立事件身份、已验证 structured 文档及该本地状态的完整请求身份日志。
**该临时编码将由共享 SDG adapter 的 Action 格式替换**；它不包含 CRDT 协议、actor/origin 的协作语义，
不是长期跨端交换格式，也不能冒充产品历史 DAG。

首次打开空原生项目提交初始化事件及快照，使 revision 0 也有有效的 Action 外键边界。
编辑命令先在可回滚的内存边界完成；每个成功的结构提交收集一份状态，成功后依次 `append_action`。
失败/恐慌的整条命令不提交任何收集状态。存储失败保留已接受状态与待写队列并显示错误，保存会重试；
不得显示已保存。这里不提供多条结构请求的跨 Action 崩溃原子组。
本地 undo/redo 和丢弃同样追加恢复事件，不删除/修改既有 Action；不声称已实现协作 undo。
显式保存写带版本信封的不可变 structured checkpoint；重复保存相同边界无操作。
打开验证最高快照并逐条验证/重放其后 Action。临时完整状态编码成本随文档增长，正式 adapter 接入时
再落实增量格式和自动 checkpoint 策略，不宣称本批已达到长文档逐键性能目标。

### 锁、冲突与装配

打开的内核持有 `ProjectStore` 至项目关闭，锁冲突在窗口明确显示，不能降级打开另一个临时正文。
源码输入保留在私人缓冲并提交独立 draft；失败时仍保留新输入、显示错误和未保存状态；保存以原始 exact-byte hash 调用 materialize。
外部修改显示“外部已修改”，提供重新载入/另存为/取消，不静默覆盖；重新载入明确丢弃私人草稿。
若仍有未完成物化意图，重新载入拒绝清除它，应先恢复或另存为保全输入。
物化已完成而草稿清理前崩溃时，只在磁盘、草稿与已完成 manifest 三者 hash 一致时接纳新基线。

应用通过 `open_latex_workspace(&ProjectHandle)` 装配占位工作区：项目名、入口和“LaTeX 预览开发中”。
它只持有 SourceProject，不启动 Typst 文档/worker；LaTeX executor 后续替换此窄接口。
普通 `.typ` 使用最小源码输入面，保存不经结构模型或语法编译。原生 Typst 保持现有页面与生成源码视图；
本批不修改 `source_view`，不实现完整 Source Studio 或 LaTeX 预览。

### 最近项目与路径选择

文件菜单复用原生命令注册表。路径选择采用 egui 原生文本路径对话框，支持文件夹、单文件和显式入口，
无需平台 shell 或外部文件选择进程；这不是操作系统文件浏览器。所有新增界面文案进入中英 Fluent 目录。
最近项目只记成功打开的规范根路径，去重、最近在前、最多 12 项，不自动执行或打开所列项目。
记录为用户配置目录 `scholium/recent-projects.json`：Linux 使用 XDG_CONFIG_HOME / ~/.config，
macOS 为 ~/Library/Application Support，Windows 为 APPDATA。同目录临时文件同步后 rename，再同步目录。
测试用 `SCHOLIUM_RECENT_FILE` 隔离；记录失败可见，不撤销已经打开的项目。

### SessionStore 退役与迁移

默认启动改为数据目录的 `scholium/default-project`；测试/显式启动使用 `SCHOLIUM_PROJECT_PATH`。
`SCHOLIUM_SESSION_FILE` 不再被主程序读取，所有真实窗口检查改用项目目录。
默认项目尚不存在时，优先探测旧 `document.v1.sqlite`，其次有 head 的 `session.scholium.sqlite`，
只读一致读取/验证当前 head 与请求日志，旧 schema 0 的历史也验证；生成新项目并提交一个导入事件。
旧公式保持 RawMath，报告数量；不制造旧动作、actor 或结构历史。旧会话库完整保留原位作为只读档案，
迁移报告写在 `.scholium/migration.json`。用户也可在“打开”对话框选择“迁移旧会话”并指定独立目标。

目标必须不存在。迁移中的目标带 `.scholium-importing` 标记，主程序拒绝打开；
写入、保存、重开比对及报告同步成功后才移除标记并同步根目录。失败目标和原库均保留，
不会把半迁移目标猜成空白文档。已有目标永不被另一次迁移覆盖。
旧 SessionStore API 与 `EditorCore::open(path)` 只保留兼容读取/工具及既有 fixture 路径，
不再是主程序默认 authority。未知格式不会原地升级。

## 验证方法

存储/会话测试验证固定语言与身份、锁冲突、源码草稿恢复、外部冲突、物化后清草稿前崩溃、
另存为资源复制、旧库字节保持、迁移目标不覆盖，以及原生未保存 Action 恢复、保存重开和撤销追加。
真实窗口检查涵盖新建 Typst—编辑—保存—SIGKILL—重开、`.typ` 草稿恢复与外部冲突、`.tex` 占位保存重开，
并保留原窗口、默认迁移、键盘、IME、无障碍及 stock-Typst 对照。结果见[开发日志](../log/2026-11-03-project-files.md)。

## 后果与替换方案

项目锁与源码恢复共用正式底座，两个项目语言不形成两个权威。源码输入不因语法错误被丢弃。
Linux 本地文件系统的 fsync/rename/锁是当前验收范围；Windows/macOS 的原生持久性与文件选择器仍须另行验收。
不增加 Loro 依赖、不改永久 schema、不做 watcher、跨文件 write group、PDF 或 LaTeX 排版。
共享 SDG adapter 接入时需显式识别临时信封并在独立目标迁移，验证恢复与兼容报告；不得把新二进制
直接重解释旧 payload，不能删除旧 Actions。原生源码/协作历史与自动 checkpoint 策略由该接入批次收敛。
