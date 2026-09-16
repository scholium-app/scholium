//! 阶段 0 第 2 项验证：SDG → Typst 生成、编译、以及 NodeId ↔ 预览位置双向映射。
//!
//! 用法：`cargo run --release`（release 才是有意义的编译耗时）。

mod generator;
mod world;

use std::time::Instant;

use scholium_spike_core::{ActorId, Editor, Intent, NodeId, SemanticEdit, fixture};
use typst::foundations::Label;
// `query_label` / `position` 定义在 trait 上，必须导入才能调用。
use typst::introspection::Introspector;
use typst::utils::PicoStr;
use typst_layout::PagedDocument;

use generator::Generation;
use world::SpikeWorld;

/// 目标规模：约 20 页（实测约 13 段/页）。
const PARAGRAPHS: usize = 256;

fn main() {
    let mut editor = Editor::new();
    fixture::build_large(&mut editor, PARAGRAPHS);
    let node_count = editor.document().node_count();
    println!("SDG：{} 段 / {} 节点", PARAGRAPHS + 1, node_count);

    // ---- 生成 ----
    let start = Instant::now();
    let generation = generator::generate(editor.document());
    let generate_ms = start.elapsed().as_secs_f64() * 1000.0;
    println!(
        "生成：{generate_ms:.2} ms，源码 {} 字节，映射 {} 个节点，标签 {} 个",
        generation.source.len(),
        generation.spans.len(),
        generation.labels.len()
    );

    // ---- 首次编译 ----
    let start = Instant::now();
    let Some(first) = compile(&generation) else {
        return;
    };
    let first_compile_ms = start.elapsed().as_secs_f64() * 1000.0;
    println!(
        "首次编译：{first_compile_ms:.0} ms，{} 页",
        first.pages().len()
    );
    report_mapping(&generation, &first);

    // ---- 缓存探针：内容未变时再编译 ----
    let warm = SpikeWorld::new(generation.source.clone());
    let start = Instant::now();
    let _ = typst::compile::<PagedDocument>(&warm);
    let warm_first = start.elapsed().as_secs_f64() * 1000.0;
    let start = Instant::now();
    let _ = typst::compile::<PagedDocument>(&warm);
    let warm_second = start.elapsed().as_secs_f64() * 1000.0;
    println!("缓存探针（内容未变）：换新 World 编译 {warm_first:.0} ms，同 World 再编译 {warm_second:.0} ms");

    // ---- 增量：改一个字符后，用**同一个 World** 重新编译 ----
    let target = first_text_node(&editor);
    editor
        .apply(
            ActorId(1),
            Intent::Typing,
            SemanticEdit::InsertText {
                node: target,
                at: 0,
                text: "改".to_string(),
            },
        )
        .expect("插入文本");

    let start = Instant::now();
    let edited = generator::generate(editor.document());
    let regenerate_ms = start.elapsed().as_secs_f64() * 1000.0;

    // 持久 World：先建一次（含首次编译），之后只换源码。
    let persistent = SpikeWorld::new(generation.source.clone());
    let start = Instant::now();
    let _ = typst::compile::<PagedDocument>(&persistent);
    let persistent_first_ms = start.elapsed().as_secs_f64() * 1000.0;

    persistent.set_source(edited.source.clone());
    let start = Instant::now();
    let warned = typst::compile::<PagedDocument>(&persistent);
    let incremental_ms = start.elapsed().as_secs_f64() * 1000.0;
    let page_count = warned.output.as_ref().map(|d| d.pages().len()).unwrap_or(0);
    println!(
        "改动一处 + 持久 World：{incremental_ms:.0} ms（冷编译 {first_compile_ms:.0} ms 的 {:.0}%），{page_count} 页",
        incremental_ms / first_compile_ms * 100.0
    );
    let _ = persistent_first_ms;

    // 对照：同一份改动交给**新建的** World。
    let start = Instant::now();
    let cold = compile(&edited);
    let cold_ms = start.elapsed().as_secs_f64() * 1000.0;
    println!(
        "改动一处 + 新建 World：{cold_ms:.0} ms（冷编译的 {:.0}%）；重新生成源码 {regenerate_ms:.2} ms",
        cold_ms / first_compile_ms * 100.0
    );
    if let Some(cold) = cold {
        println!("  新建 World 编译后 {} 页", cold.pages().len());
    }
}

/// 编译并报告诊断。失败时打印前几条错误。
fn compile(generation: &Generation) -> Option<PagedDocument> {
    let world = SpikeWorld::new(generation.source.clone());
    let warned = typst::compile::<PagedDocument>(&world);
    match warned.output {
        Ok(document) => {
            if !warned.warnings.is_empty() {
                println!("  编译警告 {} 条", warned.warnings.len());
            }
            Some(document)
        }
        Err(errors) => {
            println!("  编译失败，错误 {} 条：", errors.len());
            for error in errors.iter().take(5) {
                println!("    {}", error.message);
            }
            None
        }
    }
}

/// NodeId → 预览位置，以及 预览位置 → NodeId 的反查。
fn report_mapping(generation: &Generation, document: &PagedDocument) {
    let introspector = document.introspector();

    let mut located: Vec<(usize, f32, f32, NodeId)> = Vec::new();
    for (label, node) in &generation.labels {
        let Ok(content) = introspector.query_label(
            Label::new(PicoStr::intern(label.as_str())).expect("标签名非空")
        ) else {
            continue;
        };
        let Some(location) = content.location() else {
            continue;
        };
        let Some(position) = introspector.position(location) else {
            continue;
        };
        located.push((
            position.page.get(),
            position.point.x.to_pt() as f32,
            position.point.y.to_pt() as f32,
            *node,
        ));
    }
    located.sort_by(|a, b| {
        (a.0, a.2)
            .partial_cmp(&(b.0, b.2))
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    println!(
        "位置映射：{} / {} 个标签解析出页码与坐标",
        located.len(),
        generation.labels.len()
    );
    if located.len() < generation.labels.len() {
        let resolved: std::collections::BTreeSet<NodeId> =
            located.iter().map(|(_, _, _, node)| *node).collect();
        let missing: Vec<String> = generation
            .labels
            .iter()
            .filter(|(_, node)| !resolved.contains(node))
            .map(|(label, node)| format!("{label}(节点 {})", node.index()))
            .collect();
        println!("  未解析：{}", missing.join(", "));
    }
    for (page, x, y, node) in located.iter().take(3) {
        println!("  节点 {} → 第 {page} 页 ({x:.1}, {y:.1}) pt", node.index());
    }
    if let Some((page, x, y, node)) = located.last() {
        println!(
            "  节点 {} → 第 {page} 页 ({x:.1}, {y:.1}) pt（最后一个）",
            node.index()
        );
    }

    // 反查：给一个坐标，找回最近的节点。
    if let Some((page, x, y, expected)) = located.get(located.len() / 2).copied() {
        let probe = (x + 5.0, y + 2.0);
        if let Some((found_page, _, _, found)) = located
            .iter()
            .filter(|(p, _, _, _)| *p == page)
            .min_by(|a, b| {
                let da = (a.1 - probe.0).powi(2) + (a.2 - probe.1).powi(2);
                let db = (b.1 - probe.0).powi(2) + (b.2 - probe.1).powi(2);
                da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
            })
        {
            println!(
                "反查：第 {found_page} 页 ({:.1}, {:.1}) pt → 节点 {}（期望节点 {}）",
                probe.0,
                probe.1,
                found.index(),
                expected.index()
            );
        }
    }

    // 源码范围反查：取一个偏移，看能否回到节点。
    if let Some((_, (offset, _))) = generation.spans.iter().nth(generation.spans.len() / 2) {
        if let Some(node) = generation.node_at(offset + 1) {
            println!(
                "源码偏移 {} → 节点 {}（范围 {:?}）",
                offset + 1,
                node.index(),
                generation.spans.get(&node)
            );
        }
    }
}

fn first_text_node(editor: &Editor) -> NodeId {
    let document = editor.document();
    document
        .first_text_descendant(document.root())
        .expect("文档有文本叶子")
}
