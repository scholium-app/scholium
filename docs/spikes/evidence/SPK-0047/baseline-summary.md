# 当前编辑器自动验收

基线提交：`59eca0cb4b575353904fea707182d18608aa8bdb`

体验用例：13 通过，19 失败，共 32 项。

失败是未满足的操作契约，不做 expected-failure 豁免。

| 检查 | 退出码 | 秒 | 日志 |
|---|---:|---:|---|
| usability | 101 | 62.0 | [usability.log](usability.log) |

## Debug 性能观测（毫秒）

```text
METRIC input_frame paragraphs=1 n=40 p50_ms=1.104 p95_ms=4.470 max_ms=12.450
METRIC compile_wait paragraphs=1 n=10 p50_ms=5.091 p95_ms=5.118 max_ms=5.118
METRIC input_frame paragraphs=50 n=40 p50_ms=48.903 p95_ms=51.854 max_ms=56.065
METRIC compile_wait paragraphs=50 n=10 p50_ms=91.270 p95_ms=91.377 max_ms=91.377
METRIC input_frame paragraphs=200 n=40 p50_ms=44.566 p95_ms=53.873 max_ms=59.967
METRIC compile_wait paragraphs=200 n=10 p50_ms=924.804 p95_ms=993.744 max_ms=993.744
```

## 体验明细

| 用例 | 结果 |
|---|---|
| `editing::a01_enter_at_inline_formula_end_preserves_math` | FAILED |
| `editing::a02_enter_at_display_formula_end_preserves_math` | FAILED |
| `editing::a03_enter_inside_formula_does_not_turn_delimiters_into_text` | FAILED |
| `editing::a04_enter_inside_bold_preserves_formatting` | FAILED |
| `editing::a05_backspace_after_formula_keeps_formula_structure` | FAILED |
| `editing::a06_delete_before_formula_keeps_formula_structure` | FAILED |
| `editing::a07_paste_plain_punctuation_matches_typing` | FAILED |
| `editing::a08_multiline_unicode_paste_preserves_empty_last_paragraph` | ok |
| `editing::a09_cancelled_ime_allows_following_typing` | ok |
| `editing::a10_backspace_preserves_emoji_graphemes_without_geometry` | ok |
| `editing::a11_plain_paragraph_split_and_merge_round_trip` | ok |
| `editing::a12_cross_paragraph_selection_is_atomic` | ok |
| `editing::a13_keyboard_formula_entry_round_trips` | ok |
| `editing::a14_ime_commit_is_one_edit_and_preedit_is_not_saved` | ok |
| `editing::a15_home_end_without_geometry_stay_in_current_paragraph` | FAILED |
| `editing::a16_copy_cut_paste_preserves_formula_markup` | ok |
| `editing::a17_undo_restores_typing` | FAILED |
| `editing::a18_select_all_replacement_preserves_unicode` | ok |
| `performance::c01_measure_input_and_warm_compile_at_three_document_sizes` | ok |
| `rendering::b01_real_formula_glyph_click_then_enter_preserves_formula` | FAILED |
| `rendering::b02_pending_echo_does_not_paint_hidden_math_delimiters` | FAILED |
| `rendering::b03_pending_echo_does_not_paint_bold_markers` | FAILED |
| `rendering::b04_first_input_visible_before_any_compile` | FAILED |
| `rendering::b05_typing_after_enter_is_visible_before_compile` | FAILED |
| `rendering::b06_stale_geometry_after_deletion_keeps_surviving_line_clickable` | FAILED |
| `rendering::b07_stale_second_block_geometry_is_shifted_once` | FAILED |
| `rendering::b08_stale_unicode_click_inserts_at_clicked_position` | FAILED |
| `rendering::b09_arrow_right_at_formula_end_exits_math` | ok |
| `rendering::b10_backspace_after_formula_with_geometry_preserves_structure` | FAILED |
| `rendering::b11_compiled_unicode_backspace_removes_whole_grapheme` | ok |
| `rendering::b12_formula_compiles_and_maps_exact_token` | ok |
| `rendering::b13_stale_click_and_typing_in_one_frame_never_panics` | FAILED |

无窗口测试驱动真实 egui 输入、SessionBridge 和 LocalSession；排版用例使用真实 Typst 字形。
它不等同系统 IME、GPU 像素或跨平台验收。`--native` 另测隔离 X11 原生窗口、剪贴板及 SQLite 保存。
原生测试证据见 native/results.json；没有该文件表示未运行或未完成。
性能采样是 debug 构建下的页面输入/会话处理和编译等待，不是完整屏幕呈现延迟。
