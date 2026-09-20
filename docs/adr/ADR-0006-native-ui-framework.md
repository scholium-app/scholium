# ADR 0006：原生桌面框架选型 —— egui

- 状态：Accepted
- 日期：2026-09-16
- 决策者：ation_ciger
- 影响模块：桌面应用外壳、结构编辑视图、可访问性、输入法集成、渲染集成
- 关联：[ADR 0002 原生技术栈与 UI 验证顺序](ADR-0002-native-ui-validation-order.md)、报告 [0001](../spikes/SPK-0001-native-ui-iced.md)、[0002](../spikes/SPK-0002-native-ui-gpui.md)、[0003](../spikes/SPK-0003-native-ui-prescreen.md)、[0004](../spikes/SPK-0004-native-ui-egui.md)

## 背景

项目硬性要求原生（无 npm/Node.js/WebView）、中文输入法可用、可访问性可用，
且每个候选要跑同一组夹具（嵌套文本、数学结构、结构选区、中文 IME、源码编辑、无障碍）。
[ADR 0002](ADR-0002-native-ui-validation-order.md) 定下了"按 Iced → GPUI → C++ EUI-NEO → Slint 或 egui 依次验证"的顺序。

## 验证方法

四个候选用同一份共享核心（`spikes/native-ui/core`：语义图、光标、语义编辑、共用布局）与同一组脚本，
证据见报告 0001–0004，补充性能与延迟证据见报告 0005。关键判据与结果：

| 候选 | 文本输入控件 | 中文 IME（实测） | 可访问性（实测） | 结论 |
|---|---|---|---|---|
| Iced 0.14.0 | 有 | **可用**（预编辑 30 事件、提交 1 动作） | **无 accesskit，AT-SPI 无对象树** | Fail |
| GPUI 0.2.2 | **无**（Zed 的 `ui` crate 未发布） | 需自行实现 `InputHandler`，未实现 | **无 accesskit** | Fail |
| EUI-NEO | 有 | 有合成文本（未实测） | **无** | 预筛不投入（另有 C++/FFI 成本） |
| Slint 1.17.1 | 有 | 有 | 有 accesskit | **被许可证政策阻断**（GPLv3/商业/royalty-free） |
| **egui/eframe 0.36.2** | 有（`TextEdit`） | **可用**（用户实机确认） | **发布对象树**（frame + label） | **采用** |

无障碍测试的前提被实测确认过两次：会话开关必须 `IsEnabled` 与 `ScreenReaderEnabled` **都开**，
只开一个时 AccessKit 不注册（这是假阴性，报告 0004 已记录）。

## 候选方案

- **Iced**：架构与消息模型契合，输入法与文本控件都可用；但框架无 accesskit，**可访问性不是工作量问题而是缺失**。排除。
- **GPUI**：渲染性能好，但**没有可用的文本输入控件**，输入法要自己写，且无 accesskit。排除。
- **EUI-NEO**：C++/CMake + FFI，许可证为 Apache-2.0 但无任何无障碍；成本高、收益低。不投入。
- **Slint**：无障碍完备，但许可证不符合项目双许可（政策明确禁止链接）。排除。
- **egui**：三件事（文本输入、输入法、无障碍）都具备；自绘结构可通过 `Painter` 实现，
  并可用 `Response::widget_info` 给自绘内容补可访问描述。**采用**。

## 决策

**桌面外壳采用 egui / eframe 0.36.2**（`egui-winit 0.36.2`、`winit 0.30.13`、
`accesskit 0.24.1` + `accesskit_unix 0.21.1` 默认开启）。平台基线：Arch Linux / niri 26.04 (Wayland) /
fcitx5 5.1.22 / 系统 Source Han Serif；对照候选版本为 Iced 0.14.0、GPUI 0.2.2。

同时明确**不承诺**下列能力，它们必须在阶段 1 之前解决，但不属于本决策的选型依据：

- **结构选区**：共享核心目前只有单光标，没有选区模型；鼠标拖拽选区尚未实现。
- **正式渲染质量**：候选里的结构渲染是临时实现（固定字形宽度、不换行、根号用字符加横线拼），
  等语义树与渲染分层确定后重做。
- **增量布局**：现在每帧全量重算并全量重绘，20 页 release 约 1 ms 但**没有余量**（报告 0004/0005）。
- **输入法定位**：候选窗锚点已修正（egui-winit 在 Wayland 用 `IMEOutput.rect` 调 `set_ime_cursor_area`），
  但只验证过首页。

## 后果

正面：

- 中文输入法与可访问性同时具备，且这是四个候选里唯一做到的。
- 自绘结构与框架控件可以在同一窗口内混合（正文自绘 + 源码 `TextEdit`），符合"结构编辑 + 源码工作台"的形态。
- 应用逻辑与 eframe 解耦后（`src/lib.rs` + `tests/input.rs`），输入契约可以**无窗口**确定性测试：
  egui 8 个、Iced 8 个，覆盖同一组契约。

负面与风险：

- egui 是即时模式，**每帧重建 UI**；文档规模直接决定帧开销，必须补增量布局、按 revision 缓存与视口虚拟化。
- 无障碍依赖 accesskit_unix 的会话开关；CI 里做无障碍验收必须先打开两个开关，否则是假阴性。
- egui 的 API 在小版本间有破坏性变化（本次就踩到 `App::ui` 签名、`Modifiers::CTRL.command` 为 false 等），
  升级必须重跑 `tests/input.rs` 与无障碍/输入法实测。

需要新增的门禁：

- `tests/input.rs` 作为输入契约回归门（两个候选同构）。
- 无障碍验收脚本必须检查并报告两个会话开关的状态。
- 性能回归门：用报告 0004 的量法（文档规模—帧开销曲线）设阈值。

## 替换方案

共享核心（语义图、光标、语义编辑、布局、命中测试）与框架无关，`Layout`/`Item`/`Caret` 是稳定接口；
替换框架只需重写视图层，并以 `tests/input.rs` 的契约作为验收。Iced 是本决策的首选替代：
它的输入法与文本控件已实测可用，唯一缺口是可访问性——若将来 Iced 引入 accesskit，
替换成本主要是视图层与无障碍桥接。
