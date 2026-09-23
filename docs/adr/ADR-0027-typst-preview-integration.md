# ADR 0027：Typst 快速预览首步接入

- 状态：Accepted（本地受信内容限定范围）
- 日期：2026-09-23
- 影响模块：scholium-typst（新增）、scholium-app
- 关联：[ADR 0007](ADR-0007-typst-integration.md)、[ADR 0012](ADR-0012-typst-worker-isolation.md)、[ADR 0026](ADR-0026-block-document-session.md)

## 背景

ADR 0026 的多块会话稳定后，ROADMAP 阶段 1 的"Typst 快速预览"具备了接入前提。
此前真实文档的源码模式只显示"尚未接入"占位；spikes（报告 0005/0029/0030）已验证
进程内编译、增量缓存与后台编译机制，本 ADR 记录把这些机制带入产品 crate 的第一步边界。

## 决策

新增 `crates/scholium-typst`：块文档 → 只读 Typst 源码生成（块文本全部转义，
用户标点不成为 Typst 语法）+ 常驻编译线程 + `typst-render` 栅格化页。
依赖锁定 spikes/typst-mapping 同一版本组（typst 0.15.1 系）；升级须重跑映射与延迟基准。

编译在单条后台线程进行，持有同一个 `World` 实例（源码经 Mutex 替换），comemo 缓存
跨编译存活以保留增量行为；UI 帧永不编译。提交策略：正文停止修改 250 ms 后且处于
源码模式才提交；请求槽只保留最新 revision。结果按 revision 门控：旧 revision 的
页面到达即丢弃，不闪回（阶段 0 出口条件 3 的机制延续）。预览上限 8 页、2 px/pt。

**字体**：排版使用论文常规组合——正文 Times New Roman + SimSun（宋体），标题
Times New Roman + SimHei（黑体），来自系统安装字体（`typst-kit` system 源）；
typst-kit 内嵌字体仅作缺字回退。不因此捆绑新字体资产；OFL 类许可已由
[ADR 0011](ADR-0011-font-asset-licenses.md) 放行。界面字体与本决策无关。

**安全边界**：本路径只编译本应用内存文档的自产源码，属受信内容；不可信项目源码的
编译入口仍必须走 OS 沙箱（ADR 0012/0023），未因本决策放松。`World` 不解析文件系统、
不提供时钟（`today()` 明确失败，保证预览可重现）。

UI 边界：预览只出现在源码模式右窗格（EXPERIENCE：所见即所得模式单一编辑面，
不常驻重复预览）；左窗格的生成源码标注"生成视图 · 只读"。状态栏显示正文 revision
与排版 revision（编译中/失败/完成）。所见即所得模式仍为块编辑器，不宣称 WYSIWYG 完成。

## 不在范围

源码 → 正文的 reconcile（生成视图只读，编辑须经 ADR 0008 的正式路径）；
节点 ↔ 页面双向定位与点击跳转；增量纹理更新（当前整页上传）；页数 > 8 的长文档；
LaTeX 后端（按架构属最终构建，完整 Source Studio 归阶段 3）；预览缩放跟随 Ctrl+加减；
字体缺失的显式提示。

## 验证

- `scholium-typst`：生成映射与转义快照测试；真实编译链路（A4 竖版、RGBA 尺寸断言）；
  请求槽"最新优先"语义测试。
- `scholium-app`：headless 帧测试覆盖 revision 登记、视觉模式不编译、源码模式去抖后
  提交、再次编辑重新去抖、会话重置清空预览。
- 实机（Arch/niri）：`scripts/ui-smoke.sh all`（ydotool 注入 + grim 截图）覆盖示例工作区、
  多块编辑与源码预览三场景；Typst 预览以 Times New Roman/宋体正文、黑体标题渲染，
  含标题编号、首行缩进、两端对齐与页码；编辑后重编译实测 r2 · 151 ms。

## 后续替换门禁

双向定位接入前移植 spike 的锚点/SourceMap 机制；不可信内容接入前执行 ADR 0012
进程隔离；长文档前做分页加载与纹理复用；正式 parser 裁决（ADR 0023 门禁 3）不受本决策影响。
