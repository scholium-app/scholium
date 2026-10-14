# 2026-10-14：命令注册表、界面文字目录与确认过的快捷键/Esc 决定

本记录只追溯本次 PR 做了什么、怎么验证；行为规范见 [app.md](../modules/app.md#命令系统)，
目录选型见 [ADR 0040](../adr/ADR-0040-ui-string-catalog.md)。

## 做了什么

- 新增 `crates/scholium-app/src/command/`：`CommandSpec` 数据模型不含 egui 类型，按菜单领域（文件、编辑、插入、
  格式、文档、视图、工具、帮助）拆成八个 `specs()`，共注册 56 条命令，其中未交付的（打开、另存为、导出、查找、下划线、
  根式、页面设置……）可用性为 `Unimplemented` 并带原因 key。
- Ribbon、标签栏工作方式按钮、状态栏诊断/缩放按钮、文件/帮助菜单和快捷键都改为从注册表生成或分发：
  `command::ui` 提供大/小按钮、图标按钮、状态栏按钮和按组分隔、带子菜单的菜单树；`command::shortcuts` 遍历 `App`
  路由的和弦；页面路由的和弦（Ctrl+B/I/M、Ctrl+/、Ctrl+0/1/2）由 `input.rs` 向注册表解析。撤销/重做原有的
  “带 Shift 的先消费、不带 Shift 的在 Shift 按住时让位”逻辑移入注册表通用规则，没有丢。
- 快捷键：Ctrl+0 / 1 / 2 设正文 / 标题 1 / 标题 2；Ctrl+Shift+M 在两种工作方式之间切换；Ctrl+1/2 不再切换工作方式；
  Ctrl+0 不再是“实际大小”（该命令只剩菜单与按钮）。其余绑定不变。
- Esc：先取消选区与 IME 组合；二者都没有且光标在公式内时离开公式，落点与 Ctrl+M 离开相同；页面焦点过滤器仍
  `escape: true`，整窗帧测试断言连续两次 Esc 后页面仍持有焦点。
- 文字目录：Project Fluent，`i18n/zh-Hans.ftl` 与 `en.ftl`，key 规则与回退见 ADR；`WorkspaceState.locale` 默认 `auto`
  （`SCHOLIUM_LOCALE`、`LC_ALL`、`LC_MESSAGES`、`LANG`，缺省英语），整窗帧测试在同一个窗口里 en → zh-Hans → en 切换并核对
  Ribbon 上绘出的“Bold”/“粗体”，文档快照不变。
- 门禁：`scripts/check-ui-strings.py` 扫描 `crates/` 非测试 Rust 里含 CJK 的字符串字面量，对照
  `scripts/ui-strings-allowlist.txt`（按文件加字面量文本匹配，与行号无关）；新字面量失败，已不存在的条目也失败，所以
  只减不增。接入 CI 的 lint 作业。起点 133 条，是现有字面量减去注册表接管的部分。
- 窗口检查脚本把切换到源码的 `ctrl+2`、返回的 `ctrl+1` 改为 `ctrl+shift+m`。`scripts/window-checks.py` 给子进程设
  `SCHOLIUM_LOCALE=zh-Hans`：几项检查按中文标签排版的 Ribbon 坐标点击（英文标签更宽，组会被挤开）。

## 与规格的差异

- 可见标签有少量变化：“新建”“保存”不变；“I 强调”→“斜体”、“B 粗体”→“粗体”（注册表标题不带字母前缀）；视图页的“100%”与布局页的
  “实际大小”统一为“实际大小”；“收起功能区”按钮标题为“功能区”并以选中态表示已收起；“放大 +”“缩小 −”改为“放大”“缩小”。
- 剪贴板和弦（Ctrl+X/C/V）以 `Platform` 路由登记，只用于显示，不由应用消费（平台已把它们变成 Copy/Cut/Paste 事件）。
- 英文界面下 Ribbon 小按钮比中文宽，组宽是固定值，布局会被挤开；窗口检查因此固定中文界面。Ribbon 将被下一个任务替换，
  本次不为英文重排。
- 工具栏/焦点工具栏位置、`writes`、`in_math` 已登记并有数据，但 T12 的界面还没有读取它们（`#[allow(dead_code)]` 标注）。

## 验证

环境：Linux，Rust 1.99.0，私有 Xvfb，`SCHOLIUM_LOCALE=zh-Hans`。

- 单元/整窗测试：`cargo test --locked --workspace` 全部通过，`scholium-app` 223 项；新增注册表测试（id 唯一、标题与提示两种语言齐全、
  和弦不重复、P1 各菜单命令齐全、源码模式禁用写命令、组合输入阻止写入、未交付原因、撤销/重做和弦互不触发、Shift 按住时缩放
  仍可用）、目录测试（两种语言 key 集合相同、无未使用 key、缺 key 可发现、CLDR 复数、`LANG` 判定）、页面行为测试（Ctrl+0/1/2、
  Esc 三层、Ctrl+Shift+M 不触发公式）、整窗测试（Ctrl+1/2 不切换模式、Esc 不丢焦点、运行时换语言）。
- `cargo fmt --check`、`cargo clippy --locked --workspace --all-targets -- -D warnings`、
  `RUSTDOCFLAGS='-D warnings' cargo doc`、`check-rust-size.py`、`check-doc-links.py`、`check-ui-strings.py` 通过。
- `cargo deny check licenses bans sources` 通过；`advisories` 在本变更前后都失败（`bincode` 等既有项），见 ADR。
- `scripts/window-checks.py`：12/12 通过（a4-surface、caret-follow、default-launch、inline-style、interaction、math-attach、math-enter、native、sharp-raster、source-view、undo-group、a11y）。第一次运行暴露了两个问题并已修：英文界面挤开 Ribbon 坐标（见上，改为固定中文）；非开关命令误用 `Button::selected(false)` 被暴露成切换按钮，a11y 检查按“按钮”角色找不到“复制”（现仅 `checked` 命令才设选中态）。
