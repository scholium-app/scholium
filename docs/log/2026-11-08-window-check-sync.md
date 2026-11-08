# 2026-11-08：修复 main 上偶发失败的窗口检查

## 现象

合并 #103 和 #105 后，`main` 上的 Rust workspace 工作流失败，而同样的提交在 PR 上是绿的；中间合并的 #104 在 `main` 上也通过。
两次失败都在窗口检查，且检查项不同：

| 合并 | 运行 | 失败的检查 |
|---|---|---|
| #103（`8713f75`） | 41805223362 | `math-input [en]`：`commands` 用例第 84 行断言“输入命令期间文档不变”失败 |
| #105（`e625189`） | 42446267792 | `sharp-raster [zh-Hans]`：等待“长页面排好”超时 |

两次运行的 `window-evidence` 产物都下载下来看过，下面的结论来自其中的 `app.log` 与 `audit.json`。

## 原因

两者是同一个弱点：检查用固定的等待时间猜测应用何时处理完输入，CI 机器一忙，猜测就会落空。都不是产品缺陷。

- **`math-input`：** `Replay.ready()` 把“相隔 100 ms 两次读到相同的 AUDIT”当作已稳定。`xdotool type before` 返回后，应用可能还没收到按键，
  `ready()` 便返回了输入前的状态：`audit.json` 第一行记录的是空文档、revision 0，而不是 `before`。下一步读到 revision 6 的 `before`，
  “文档不变”的断言因此失败。
- **`sharp-raster`：** 滚动用例一次性输入 150 行、每行 46 个字符，约 7000 次按键，然后最多等 60 秒。超时时应用已处理约 2000 次编辑
  （revision 2038，文档到 `Line 45`），排队输入为 0，没有丢键；布局本身只要 3 ms。应用在这台 runner 上每帧处理约 30 个按键、
  约一帧一秒，60 秒内处理不完。

## 改动

- **应用（只影响 `SCHOLIUM_EDITOR_AUDIT` 输出）：** AUDIT 新增 `input_seq`，按 egui pass 统计收到的按键按下、文字、粘贴、IME 提交与指针按下，
  同一 pass 只计一次，编辑器忽略的按键也计入。每次送达的输入因此都会在 AUDIT 中留下变化。附单元测试。
- **`parity-check.py` 的 `Replay`：** `xdo` 执行 `key`、`type`、`click` 前记下当时的 `input_seq`；`ready()` 先等 `input_seq` 超过该值
  （应用确实收到了输入），再按原规则等状态稳定。用 `Replay` 的 7 个检查都受益。
- **`sharp-raster-check.py`：** 长页面改为输入 `Line N` 短行（约 1300 次按键，页面高度仍远超 4000 px）；“长页面排好”的等待改为看进度：
  只要 revision 还在前进就继续等，连续 60 秒没有新 revision 才判失败。断言内容不变。

## 验证

非窗口门禁：fmt、clippy、`cargo test --workspace` 726 passed、doc、size、links、ui-strings、layers、deny 全部通过。
受影响的 8 个窗口检查（`parity`、`math-input`、`sharp-raster`、`backspace-block-merge`、`display-formula`、`fraction-at-caret`、`math-root-delimiter`、`math-edge-caret`）在本机中英文下 16/16 通过。
`math-input` 的 `commands` 用例第一步现在记录的是 revision 6 的 `before`（`input_seq` 为 12，即 6 次按键加 6 个文字事件），不再是输入前的空文档。
