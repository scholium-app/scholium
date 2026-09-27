# 当前编辑器自动验收

基线提交：`a619b61f9d02e59a0c20cec4c9a1eacf538253e0`

体验用例：31 通过，1 失败，共 32 项。

失败是未满足的操作契约，不做 expected-failure 豁免。

| 检查 | 退出码 | 秒 | 日志 |
|---|---:|---:|---|
| format | 0 | 0.165 | [format.log](format.log) |
| size | 0 | 0.032 | [size.log](size.log) |
| clippy | 0 | 0.264 | [clippy.log](clippy.log) |
| baseline | 0 | 22.356 | [baseline.log](baseline.log) |
| rustdoc | 0 | 0.215 | [rustdoc.log](rustdoc.log) |
| usability | 101 | 16.296 | [usability.log](usability.log) |
| native-build | 0 | 0.265 | [native-build.log](native-build.log) |
| native | 0 | 36.352 | [native.log](native.log) |

## 原生窗口

| 场景 | 结果 |
|---|---|
| inline-enter | passed |
| display-enter | passed |
| formula-backspace | passed |
| plain-split | passed |
| undo | passed |
| views-save-restore | passed |
| keyboard-crlf-paste | passed |
| ribbon-crlf-paste | passed |

## Debug 性能观测（毫秒）

```text
METRIC input_frame paragraphs=1 n=40 p50_ms=0.681 p95_ms=1.764 max_ms=2.759
METRIC compile_wait paragraphs=1 n=10 p50_ms=5.121 p95_ms=5.152 max_ms=5.152
METRIC input_frame paragraphs=50 n=40 p50_ms=21.153 p95_ms=23.954 max_ms=27.799
METRIC compile_wait paragraphs=50 n=10 p50_ms=76.261 p95_ms=86.164 max_ms=86.164
METRIC input_frame paragraphs=200 n=40 p50_ms=22.026 p95_ms=25.713 max_ms=26.130
METRIC compile_wait paragraphs=200 n=10 p50_ms=896.654 p95_ms=938.325 max_ms=938.325
```

## 体验明细

| 用例 | 结果 |
|---|---|
| `editing::a01_enter_at_inline_formula_end_preserves_math` | ok |
| `editing::a02_enter_at_display_formula_end_preserves_math` | ok |
| `editing::a03_enter_inside_formula_does_not_turn_delimiters_into_text` | ok |
| `editing::a04_enter_inside_bold_preserves_formatting` | ok |
| `editing::a05_backspace_after_formula_keeps_formula_structure` | ok |
| `editing::a06_delete_before_formula_keeps_formula_structure` | ok |
| `editing::a07_paste_plain_punctuation_matches_typing` | FAILED |
| `editing::a08_multiline_unicode_paste_preserves_empty_last_paragraph` | ok |
| `editing::a09_cancelled_ime_allows_following_typing` | ok |
| `editing::a10_backspace_preserves_emoji_graphemes_without_geometry` | ok |
| `editing::a11_plain_paragraph_split_and_merge_round_trip` | ok |
| `editing::a12_cross_paragraph_selection_is_atomic` | ok |
| `editing::a13_keyboard_formula_entry_round_trips` | ok |
| `editing::a14_ime_commit_is_one_edit_and_preedit_is_not_saved` | ok |
| `editing::a15_home_end_without_geometry_stay_in_current_paragraph` | ok |
| `editing::a16_copy_cut_paste_preserves_formula_markup` | ok |
| `editing::a17_undo_restores_typing` | ok |
| `editing::a18_select_all_replacement_preserves_unicode` | ok |
| `performance::c01_measure_input_and_warm_compile_at_three_document_sizes` | ok |
| `rendering::b01_real_formula_glyph_click_then_enter_preserves_formula` | ok |
| `rendering::b02_pending_echo_does_not_paint_hidden_math_delimiters` | ok |
| `rendering::b03_pending_echo_does_not_paint_bold_markers` | ok |
| `rendering::b04_first_input_visible_before_any_compile` | ok |
| `rendering::b05_typing_after_enter_is_visible_before_compile` | ok |
| `rendering::b06_stale_geometry_after_deletion_keeps_surviving_line_clickable` | ok |
| `rendering::b07_stale_second_block_geometry_is_shifted_once` | ok |
| `rendering::b08_stale_unicode_click_inserts_at_clicked_position` | ok |
| `rendering::b09_arrow_right_at_formula_end_exits_math` | ok |
| `rendering::b10_backspace_after_formula_with_geometry_preserves_structure` | ok |
| `rendering::b11_compiled_unicode_backspace_removes_whole_grapheme` | ok |
| `rendering::b12_formula_compiles_and_maps_exact_token` | ok |
| `rendering::b13_stale_click_and_typing_in_one_frame_never_panics` | ok |

无窗口测试驱动真实 egui 输入、SessionBridge 和 LocalSession；排版用例使用真实 Typst 字形。
它不等同系统 IME、GPU 像素或跨平台验收。`--native` 另测隔离 X11 原生窗口、剪贴板及 SQLite 保存。
原生测试证据见 native/results.json；没有该文件表示未运行或未完成。
性能采样是 debug 构建下的页面输入/会话处理和编译等待，不是完整屏幕呈现延迟。
