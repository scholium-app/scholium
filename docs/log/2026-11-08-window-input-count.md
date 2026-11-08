# 窗口检查：等待全部已发送输入、审计行单次写出

## 问题

PR #108 的 CI 出现三处与改动无关的失败：

- `backspace-block-merge`（中文）：Backspace 后实际状态仍停在 Home 之后，undo 深度少 1。
  [#106](2026-11-08-window-check-sync.md) 让 `parity-check.py` 等待 `input_seq` 增加，但只要求**至少 1 个**事件到达。
  `type right` 的第一个字母到达就放行，剩余字母随后被记到下一条命令名下，之后每一步都读到前一步的状态。
- `math-enter`（中文）：`$x^2` 的 `2` 尚未送达时就记录了“之前”的快照，撤销后与之比较不相等。
- `latex-exit`（英文）：读取 AUDIT 行时 JSON 解析失败。`println!`/`eprintln!` 把一行分成多次写出，
  沙箱内的 TeX/rtex 子进程继承同一输出，别的字节可以插进行中间。

## 改动

- `chrome_ui.expected_inputs`：按 xdotool 命令计算应用至少会计数的事件数（每个键入字符、每个按键组合、每次点击各 1）。
  `parity-check.py` 等待 `input_seq ≥ 发送前 + 预期数`；`math-enter-check.py` 的 `wait` 同样以此为前提。
  其余自带 `xdo` 的检查以内容谓词判定结果，暂未改动，可逐步迁移到同一函数。
- 新增 `audit_line::emit`：先格式化整行再一次写出；应用内 11 处机器读取的输出（AUDIT、PROJECT、PROJECT_UI、
  BREADCRUMB、SOURCE_PREVIEW 等）全部改用它。LaTeX 工作台的 AUDIT 从 stderr 改到 stdout，检查脚本本就合并两路输出。

没有放宽任何断言或阈值。

## 验证

- 单元测试：长 JSON 正文恰好成为一行并可解析。
- `cargo fmt`、workspace `clippy -D warnings`、`scholium-app` 测试、bins 构建、size、UI strings、layers 通过。
- 本机全部窗口检查中英双语 64/64 通过。
