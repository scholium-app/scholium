# ADR 0052：LaTeX 内核图元、同引擎回退与共享源码工作台

状态：接受

## 问题

L1 需要把 rtex 元数据接为可呈现预览，同时保持源码权威、UI 独立和非 Lua 引擎兼容。
上游增量页面不能在队列合并时丢失；未知字形/图元不能伪装成完整预览；整编不能执行用户 shell。

## 决定

- 公开 Scholium 自有不透明 Drawing 与不可变 RGBA RasterPage，上游 JSON 只存在私有协议层。
  监督层先累计完整页面，再合并事件；栅格线程保留所有页，UI 可丢弃过期结果而不丢中间依赖。
- 使用既有 E4 实验验证的 tiny-skia + skrifa 字形路径，TFM 通过 pdftex.map/encoding 解析 Type 1；
  宿主字体解析仅允许正规系统字体/TeX 树。图元坐标为 TeX sp，页面为 bp，固定 300 dpi。
- degraded、图片、缺失字形或未知图元改用同次 layout 的 PDF，经 hayro 渲染；没有当前 PDF 则请求完整布局。
  预算约束在分配前检查，失败保留上次结果。像素验收仍为 E4 的 ≤1% 未匹配墨迹、1 像素容差。
- build 增加独立 FullCompilation profile；不可变项目资源快照在沙箱内编译。XeLaTeX 显式分成 XDV 与 xdvipdfmx 两步，
  不开放 shell；最多两轮，宿主 deadline 覆盖全部子进程，结果绑定固定引擎、revision 与来源。
- app 共享源码编辑器只消费语言内核的文本/高亮/诊断。Typst 生成源码仍只读，LaTeX 使用可编辑 TextEdit；
  IME 预编辑不进入源码权威。补全复用 math-input 候选逻辑，接受后插入 LaTeX 源码。

## 后果

渲染不进入 UI/心跳热路径；无 rtex/TeX 的 CI 验证明确不可用状态，本机另做实际像素与真实窗口 E6。
Linux 外部工具白名单保持 fail-closed，完整 TeX 安装与系统字体仍为本机依赖，不自动下载宏包。
开发入口只读入 main.tex，编辑未持久化；文件菜单、项目装配和保存由独立任务接入。
