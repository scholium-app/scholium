//! P0 验证 #1：span 粒度。
//!
//! 问题：Typst 编译产物里的每个字形，能否反查到源码中足够细的位置？
//! 通过标准：分子、分母、上标、根号内容各自可区分。
//!
//! 不通过的退路：序列化时插标记节点提高分辨率。

mod world;

use typst::WorldExt;
use typst::layout::{Frame, FrameItem, Point};
use typst_layout::PagedDocument;
use world::SpikeWorld;

/// 验证用文档。每个数学结构都放不同字符，便于肉眼核对归属。
const DOC: &str = r#"= 标题

行内公式 $a + b$ 与正文混排。

$ frac(p, q) $

$ x^u + y_v $

$ sqrt(w) $

$ mat(1, 2; 3, 4) $
"#;

fn main() {
    let world = SpikeWorld::new(DOC);
    println!("字体数: {}", world.font_count());

    let result = typst::compile::<PagedDocument>(&world);

    for warn in result.warnings.iter() {
        println!("warn: {}", warn.message);
    }

    let doc = match result.output {
        Ok(doc) => doc,
        Err(errors) => {
            for e in errors.iter() {
                eprintln!("error: {}", e.message);
            }
            std::process::exit(1);
        }
    };

    println!("页数: {}\n", doc.pages().len());

    let mut stats = Stats::default();
    for (i, page) in doc.pages().iter().enumerate() {
        println!("═══ 第 {} 页 ═══", i + 1);
        walk(&page.frame, Point::zero(), &world, 0, &mut stats);
    }

    println!("\n═══ 统计 ═══");
    println!("字形总数:       {}", stats.glyphs);
    println!("  有 span:      {}", stats.mapped);
    println!("  detached:     {}", stats.detached);
    println!("  span 无范围:  {}", stats.no_range);
    println!("Shape 总数:     {}", stats.shapes);
    println!("  有 span:      {}", stats.shapes_mapped);
    println!("非零 u16 偏移:  {}", stats.nonzero_offset);
    println!("不同源码范围数: {}", stats.distinct_ranges.len());
}

#[derive(Default)]
struct Stats {
    glyphs: usize,
    mapped: usize,
    detached: usize,
    no_range: usize,
    shapes: usize,
    shapes_mapped: usize,
    nonzero_offset: usize,
    distinct_ranges: std::collections::BTreeSet<(usize, usize)>,
}

/// 递归遍历 frame，打印每个字形的位置、源码字节范围和对应源码文本。
fn walk(frame: &Frame, origin: Point, world: &SpikeWorld, depth: usize, stats: &mut Stats) {
    let pad = "  ".repeat(depth);
    let src = world.source_ref();

    for (pos, item) in frame.items() {
        let abs = origin + *pos;

        match item {
            FrameItem::Group(group) => {
                println!("{pad}[Group]");
                walk(&group.frame, abs, world, depth + 1, stats);
            }

            FrameItem::Text(text) => {
                println!(
                    "{pad}[Text] {:?} size={:.1}pt glyphs={}",
                    text.text,
                    text.size.to_pt(),
                    text.glyphs.len()
                );

                for g in &text.glyphs {
                    let (span, offset) = g.span;
                    stats.glyphs += 1;
                    if offset != 0 {
                        stats.nonzero_offset += 1;
                    }

                    let range = if span.is_detached() {
                        stats.detached += 1;
                        None
                    } else {
                        stats.mapped += 1;
                        let r = world.range(span);
                        if r.is_none() {
                            stats.no_range += 1;
                        }
                        r
                    };

                    if let Some(r) = &range {
                        stats.distinct_ranges.insert((r.start, r.end));
                    }

                    // 源码中对应的文本，验证归属是否正确
                    let snippet = range
                        .as_ref()
                        .and_then(|r| src.text().get(r.clone()))
                        .unwrap_or("<无映射>");

                    // 该字形在 TextItem.text 内的切片
                    let in_item = text
                        .text
                        .get(g.range.start as usize..g.range.end as usize)
                        .unwrap_or("?");

                    println!(
                        "{pad}  x={:>7.2} id={:<5} item[{:>2}..{:<2}]={:<6?} src{:<12} off={} {:?}",
                        abs.x.to_pt(),
                        g.id,
                        g.range.start,
                        g.range.end,
                        in_item,
                        range
                            .map(|r| format!("[{}..{}]", r.start, r.end))
                            .unwrap_or_else(|| "[detached]".into()),
                        offset,
                        snippet,
                    );
                }
            }

            FrameItem::Shape(_, span) => {
                stats.shapes += 1;
                let range = if span.is_detached() {
                    None
                } else {
                    stats.shapes_mapped += 1;
                    world.range(*span)
                };
                let snippet = range
                    .as_ref()
                    .and_then(|r| src.text().get(r.clone()))
                    .unwrap_or("<无映射>");
                println!(
                    "{pad}[Shape] y={:.2} src{} {:?}",
                    abs.y.to_pt(),
                    range
                        .map(|r| format!("[{}..{}]", r.start, r.end))
                        .unwrap_or_else(|| "[detached]".into()),
                    snippet
                );
            }

            FrameItem::Image(_, size, span) => {
                println!("{pad}[Image] {:?} span={:?}", size, world.range(*span));
            }

            FrameItem::Link(..) => println!("{pad}[Link]"),
            FrameItem::Tag(_) => {}
        }
    }
}
