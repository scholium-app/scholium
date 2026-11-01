# IME 候选窗与行内预编辑

## Part A：候选窗锚点

修正了当前 caret 和 pending Scene fallback 两处 IME 输出。egui-winit 0.36.2
以 `IMEOutput.rect` 调用 `set_ime_cursor_area`；两矩形均发布 caret，并限制在可见页面。
保持已有 pending 时不断开输入法的策略。AUDIT 增加两矩形，真实窗口检查比较其一致性。

先运行 `cargo test --locked -p scholium-app ime_area_tracks -- --nocapture`，回归测试失败：
实际 rect 为整页 `[80,100,680,800]`，cursor 为 `[250.5,172,250.5,180.3]`。
修复后同一测试通过（1/1）。Part A 先独立提交，Part B 与全量门禁随后执行。
