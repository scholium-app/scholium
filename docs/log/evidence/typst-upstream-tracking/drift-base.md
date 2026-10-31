# 同一脚本在固定基线 9dfd3a0 (v0.15.1) 上的全绿运行，用于验证三阶段都能跑通
Typst upstream drift: `9dfd3a08500b` (2026-07-17) vs fork base v0.15.1 `9dfd3a08500b`

| stage | result | detail |
| --- | --- | --- |
| patch 0001-structural-math-origin-and-holes | clean |  |
| patch 0002-composite-slots-and-fraction-rule-owners | clean |  |
| patch 0003-shaped-text-caret-geometry | clean |  |
| patch 0004-editor-paragraph-computation-counters | clean |  |
| compile scholium-typst (editor) | ok |  |
| pixels: patched vs unpatched | equal | full, scripts, styles, heading (all RGBA bytes) |
