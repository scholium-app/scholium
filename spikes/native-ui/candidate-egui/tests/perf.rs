//! egui 候选的帧开销测量（无窗口）。
//!
//! 动机：候选在**每一帧**都重算 `layout_document`，所以文档规模直接决定帧开销。
//! 这里用 `Context::run_ui` 在无窗口条件下测量"一个输入事件 → 核心更新 → 重新布局 → 画完一帧"
//! 的耗时。**它不包含 GPU 光栅化**，因此是逻辑侧的下界，不是完整帧时。
//!
//! 用法：`cargo test -- --nocapture`（数字会打印出来）。

use std::time::Instant;

use egui::{Context, Event, RawInput};
use scholium_spike_core::{Editor, fixture};
use scholium_spike_egui::SpikeApp;

fn frame_with(app: &mut SpikeApp, ctx: &Context, events: Vec<Event>) -> f64 {
    let input = RawInput {
        events,
        ..Default::default()
    };
    let start = Instant::now();
    let mut output = ctx.run_ui(input, |ui| app.draw(ui));
    let elapsed = start.elapsed().as_secs_f64() * 1000.0;
    output.textures_delta.clear();
    elapsed
}

fn measure(paragraphs: usize, frames: usize) -> (f64, f64) {
    let ctx = Context::default();
    let mut app = SpikeApp::new_layout_probe(&ctx, None);
    if paragraphs > 0 {
        let mut core = Editor::new();
        fixture::build_large(&mut core, paragraphs);
        app.replace_document_for_test(core);
    }

    // 让启动聚焦与首帧布局稳定下来。
    frame_with(&mut app, &ctx, vec![]);
    frame_with(&mut app, &ctx, vec![]);

    let mut samples = Vec::with_capacity(frames);
    for index in 0..frames {
        // 每个事件都可能触发一次"核心更新 + 重新布局"，正是要测的路径。
        let events = vec![Event::Text(
            if index % 2 == 0 { "a" } else { "b" }.to_string(),
        )];
        samples.push(frame_with(&mut app, &ctx, events));
    }
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let median = samples[samples.len() / 2];
    let p95 = samples[(samples.len() * 95 / 100).min(samples.len() - 1)];
    (median, p95)
}

#[test]
fn frame_cost_scales_with_document_size() {
    let frames = 60;
    println!("段落数 | 帧中位(ms) | 帧 p95(ms)   ← 仅逻辑侧，不含 GPU 光栅化，随当前构建类型变化");
    let mut results = Vec::new();
    for paragraphs in [0usize, 100, 500, 1000] {
        let (median, p95) = measure(paragraphs, frames);
        println!("{paragraphs:>6} | {median:>10.2} | {p95:>10.2}");
        results.push((paragraphs, median, p95));
    }

    // 只做防灾难性回归的松散断言：debug 构建下 1000 段的一帧不应超过 500ms。
    let (_, _, worst_p95) = results.last().copied().unwrap_or((0, 0.0, 0.0));
    assert!(
        worst_p95 < 500.0,
        "1000 段的一帧 p95 达到 {worst_p95:.1}ms，明显异常"
    );
}
