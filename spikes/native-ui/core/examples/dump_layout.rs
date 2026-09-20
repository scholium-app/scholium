//! 打印标准夹具的布局结果，用于核对结构渲染的位置。
//!
//! 布局器是手写近似（见 `core/src/layout.rs` 模块说明），错位时要靠这里的数字定位，
//! 而不是对着截图猜。
//!
//! 用法：`cargo run --example dump_layout`

use scholium_spike_core::{Editor, Item, fixture, layout_document, layout_node};

fn main() {
    let mut core = Editor::new();
    let paragraph = fixture::build_standard(&mut core);

    let document = core.document();
    let math = document.slot(paragraph, 0).expect("段落槽位")[1];

    println!("=== 段落（含文本与内联公式）===");
    dump(&layout_node(document, paragraph, Default::default()));

    println!();
    println!("=== 公式节点本身 ===");
    dump(&layout_node(document, math, Default::default()));

    println!();
    println!("=== 整篇 ===");
    let whole = layout_document(document);
    println!(
        "宽 {} 高 {} 基线 {} 图元 {}",
        round(whole.width),
        round(whole.height),
        round(whole.baseline),
        whole.items.len()
    );
}

fn round(value: f32) -> f32 {
    (value * 10.0).round() / 10.0
}

fn dump(layout: &scholium_spike_core::Layout) {
    println!(
        "外框 {}×{} 基线 {}",
        round(layout.width),
        round(layout.height),
        round(layout.baseline)
    );
    for item in &layout.items {
        match item {
            Item::Text {
                x,
                baseline,
                size,
                content,
                ..
            } => println!(
                "  Text  x={:<7} baseline={:<7} size={:<4} top={:<7} {:?}",
                round(*x),
                round(*baseline),
                round(*size),
                round(Item::top_of(*baseline, *size)),
                content
            ),
            Item::Rule {
                x,
                y,
                width,
                height,
            } => println!(
                "  Rule  x={:<7} y={:<7} w={:<7} h={}",
                round(*x),
                round(*y),
                round(*width),
                round(*height)
            ),
        }
    }
}
