//! 测量布局器随文档规模的开销。
//!
//! egui 候选每帧都会重算 `layout_document`，因此这条曲线决定"文档能开多大"。
//! 这是 **debug 构建**的数字，用于发现量级问题，不是发布性能。
//!
//! 用法：`cargo run --release --example bench_layout` 或 `--offline`（debug）。

use std::time::Instant;

use scholium_spike_core::{Editor, fixture, layout_document};

fn main() {
    println!("段落数 | 首帧布局(ms) | 稳态均值(ms) | 图元数 | 每次布局(µs/节点)");
    for paragraphs in [1usize, 10, 100, 500, 2000] {
        let mut core = Editor::new();
        fixture::build_large(&mut core, paragraphs);
        let document = core.document();
        let nodes = document.node_count();

        let first = Instant::now();
        let layout = layout_document(document);
        let first_ms = first.elapsed().as_secs_f64() * 1000.0;

        let rounds = 20;
        let start = Instant::now();
        for _ in 0..rounds {
            std::hint::black_box(layout_document(document));
        }
        let mean_ms = start.elapsed().as_secs_f64() * 1000.0 / rounds as f64;

        println!(
            "{paragraphs:>6} | {first_ms:>12.2} | {mean_ms:>12.2} | {:>6} | {:>15.2}",
            layout.items.len(),
            mean_ms * 1000.0 / nodes as f64
        );
    }
}
