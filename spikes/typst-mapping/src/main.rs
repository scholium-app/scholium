//! 阶段 0 第 2 项验证：SDG → Typst 生成、编译、编译缓存，以及
//! `NodeId` ↔ 预览位置的双向映射（含行内结构）。
//!
//! 用法：
//! - `cargo run --release`        主流程
//! - `cargo run --release -- probe` 锚点写法探针

mod checks;
mod generator;
mod world;

use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

use scholium_spike_core::{ActorId, Document, Editor, Intent, NodeId, SemanticEdit, fixture};
use typst::foundations::Label;
use typst::introspection::Introspector;
use typst::utils::PicoStr;
use typst_layout::PagedDocument;

use generator::{Generation, Side};
use world::SpikeWorld;

/// 目标规模：约 20 页（13 段/页）。
const PARAGRAPHS: usize = 256;

/// 预览中的一个点。
#[derive(Clone, Copy, Debug)]
struct Spot {
    page: usize,
    x: f32,
    y: f32,
}

fn main() {
    if std::env::args().nth(1).as_deref() == Some("probe") {
        checks::probe();
        return;
    }
    if std::env::args().nth(1).as_deref() == Some("charmap") {
        checks::charmap();
        return;
    }
    if std::env::args().nth(1).as_deref() == Some("render") {
        checks::render_check();
        return;
    }

    let mut editor = Editor::new();
    fixture::build_large(&mut editor, PARAGRAPHS);
    let node_count = editor.document().node_count();
    println!("SDG：{} 段 / {} 节点", PARAGRAPHS + 1, node_count);

    // ---- 生成 ----
    let start = Instant::now();
    let generation = generator::generate(editor.document());
    let generate_ms = start.elapsed().as_secs_f64() * 1000.0;
    println!(
        "生成：{generate_ms:.2} ms，源码 {} 字节，源码映射 {} 节点，锚点 {}",
        generation.source.len(),
        generation.spans.len(),
        generation.anchors.len()
    );

    // ---- 首次编译 ----
    let start = Instant::now();
    let Some(first) = checks::compile(&generation) else {
        return;
    };
    let first_compile_ms = start.elapsed().as_secs_f64() * 1000.0;
    println!(
        "首次编译：{first_compile_ms:.0} ms，{} 页",
        first.pages().len()
    );
    report_mapping(&generation, &first, editor.document());

    // ---- 缓存探针 ----
    let warm = SpikeWorld::new(generation.source.clone());
    let start = Instant::now();
    let _ = typst::compile::<PagedDocument>(&warm);
    let warm_first = start.elapsed().as_secs_f64() * 1000.0;
    let start = Instant::now();
    let _ = typst::compile::<PagedDocument>(&warm);
    let warm_second = start.elapsed().as_secs_f64() * 1000.0;
    println!(
        "缓存探针（内容未变）：换新 World {warm_first:.0} ms，同 World {warm_second:.0} ms"
    );

    // ---- 增量：改一处，复用同一个 World ----
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

    let persistent = SpikeWorld::new(generation.source.clone());
    let _ = typst::compile::<PagedDocument>(&persistent);
    persistent.set_source(edited.source.clone());
    let start = Instant::now();
    let warned = typst::compile::<PagedDocument>(&persistent);
    let incremental_ms = start.elapsed().as_secs_f64() * 1000.0;
    let pages = warned.output.as_ref().map(|d| d.pages().len()).unwrap_or(0);
    println!(
        "改动一处 + 持久 World：{incremental_ms:.0} ms（冷编译的 {:.0}%），{pages} 页",
        incremental_ms / first_compile_ms * 100.0
    );

    let start = Instant::now();
    let cold = checks::compile(&edited);
    let cold_ms = start.elapsed().as_secs_f64() * 1000.0;
    println!(
        "改动一处 + 新建 World：{cold_ms:.0} ms（冷编译的 {:.0}%）；重新生成源码 {regenerate_ms:.2} ms",
        cold_ms / first_compile_ms * 100.0
    );
    if let Some(cold) = cold {
        println!("  新建 World 编译后 {} 页", cold.pages().len());
    }
}

/// 解析全部锚点，报告覆盖情况，并做双向映射的往返测试。
fn report_mapping(generation: &Generation, document: &PagedDocument, sdg: &Document) {
    let introspector = document.introspector();

    let mut begins: BTreeMap<NodeId, Spot> = BTreeMap::new();
    let mut ends: BTreeMap<NodeId, Spot> = BTreeMap::new();
    let mut resolved = 0usize;
    for (name, (node, side)) in &generation.anchors {
        let Ok(content) = introspector.query_label(
            Label::new(PicoStr::intern(name.as_str())).expect("锚点名非空"),
        ) else {
            continue;
        };
        let Some(location) = content.location() else {
            continue;
        };
        let Some(position) = introspector.position(location) else {
            continue;
        };
        resolved += 1;
        let spot = Spot {
            page: position.page.get(),
            x: position.point.x.to_pt() as f32,
            y: position.point.y.to_pt() as f32,
        };
        match side {
            Side::Begin => begins.insert(*node, spot),
            Side::End => ends.insert(*node, spot),
        };
    }
    // 终点锚点可能被折到下一行，那样起止不在同一行，区间无效。
    // 只有同一行内的起止才能当作"该节点在预览里的横向区间"。
    let same_line = |node: &NodeId| -> bool {
        match (begins.get(node), ends.get(node)) {
            (Some(begin), Some(end)) => {
                begin.page == end.page && (begin.y - end.y).abs() <= 2.0
            }
            _ => false,
        }
    };
    println!(
        "锚点解析：{resolved} / {}（节点 {} 个有起点，{} 个起止同行可作区间）",
        generation.anchors.len(),
        begins.len(),
        begins.keys().filter(|node| same_line(node)).count()
    );

    // 每页锚点数：确认定位不是只在第 1 页成立。
    let mut per_page: BTreeMap<usize, usize> = BTreeMap::new();
    for spot in begins.values() {
        *per_page.entry(spot.page).or_default() += 1;
    }
    let pages_covered = per_page.len();
    let pages_with_nodes = per_page.values().filter(|count| **count > 0).count();
    let min_per_page = per_page.values().copied().min().unwrap_or(0);
    let max_per_page = per_page.values().copied().max().unwrap_or(0);
    println!(
        "  跨页覆盖：{pages_covered} 页有锚点（{pages_with_nodes} 页非空），每页 {min_per_page}..{max_per_page} 个"
    );

    // 按节点类型统计"是否拿到了坐标"。
    let mut by_kind: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    for node in begins.keys() {
        let kind = sdg
            .node(*node)
            .map(|n| format!("{:?}", n.kind))
            .unwrap_or_else(|_| "?".to_string());
        let entry = by_kind.entry(kind).or_default();
        entry.0 += 1;
        if same_line(node) {
            entry.1 += 1;
        }
    }
    let summary: Vec<String> = by_kind
        .iter()
        .map(|(kind, (total, both))| format!("{kind} {both}/{total}"))
        .collect();
    println!("  按类型（同行区间/总数）：{}", summary.join("，"));

    // 反查：点 → 节点。同一行内取区间最小的（最内层）结构。
    let hit_test = |page: usize, x: f32, y: f32| -> Option<NodeId> {
        let mut best: Option<(f32, f32, NodeId)> = None;
        for (node, begin) in &begins {
            let Some(end) = ends.get(node) else {
                continue;
            };
            if begin.page != page || end.page != page {
                continue;
            }
            // 跨行的区间无效（终点被折到下一行），退化为"起点附近的点"。
            let (low, high) = if (begin.y - end.y).abs() <= 2.0 {
                (begin.x.min(end.x), begin.x.max(end.x))
            } else {
                (begin.x - 2.0, begin.x + 2.0)
            };
            if x < low - 4.0 || x > high + 4.0 {
                continue;
            }
            let dy = (y - begin.y).abs();
            if dy > 14.0 {
                continue;
            }
            let width = high - low;
            let better = match best {
                None => true,
                Some((best_dy, best_width, _)) => {
                    (dy, width) < (best_dy, best_width)
                }
            };
            if better {
                best = Some((dy, width, *node));
            }
        }
        if let Some((_, _, node)) = best {
            return Some(node);
        }
        // 兜底：没有区间包含该点（例如公式、段落只有起点），
        // 取同一行内最近的起点；同样优先最内层（源码范围最小）。
        let span_len = |node: NodeId| -> usize {
            generation
                .spans
                .get(&node)
                .map(|(start, end)| end - start)
                .unwrap_or(usize::MAX)
        };
        let mut nearest: Vec<(f32, usize, NodeId)> = begins
            .iter()
            .filter(|(_, begin)| begin.page == page && (begin.y - y).abs() <= 14.0)
            .map(|(node, begin)| ((begin.x - x).abs(), span_len(*node), *node))
            .collect();
        nearest.sort_by(|a, b| {
            (a.0, a.1)
                .partial_cmp(&(b.0, b.1))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        nearest.first().map(|(_, _, node)| *node)
    };

    // 往返测试：行内结构与公式各取样本。
    let targets = ["Fraction", "Script", "Sqrt", "Matrix", "Math"];
    for kind_name in targets {
        // 公式与段落只有起点，用起点当区间（探针取起点）。
        let point_only_kind = kind_name == "Math" || kind_name == "Paragraph";
        let _ = point_only_kind;
        let samples: Vec<(NodeId, Spot, Spot)> = begins
            .iter()
            .filter(|(node, _)| {
                sdg.node(**node)
                    .map(|n| format!("{:?}", n.kind) == kind_name)
                    .unwrap_or(false)
            })
            .filter_map(|(node, begin)| {
                let end = ends.get(node).copied().unwrap_or(*begin);
                Some((*node, *begin, end))
            })
            .filter(|(_, begin, end)| {
                point_only_kind
                    || (begin.page == end.page && (begin.y - end.y).abs() <= 2.0)
            })
            .collect();
        // 跨页抽样：均匀取 40 个，覆盖整个文档而不是只看前几页。
        let step = (samples.len() / 40).max(1);
        let samples: Vec<(NodeId, Spot, Spot)> =
            samples.into_iter().step_by(step).take(40).collect();
        if samples.is_empty() {
            println!("  {kind_name}：无样本");
            continue;
        }
        let point_only = kind_name == "Math" || kind_name == "Paragraph";
        let mut hits = 0;
        let mut example = String::new();
        for (node, begin, end) in &samples {
            let probe_x = if point_only {
                begin.x
            } else {
                (begin.x + end.x) / 2.0
            };
            let found = hit_test(begin.page, probe_x, begin.y);
            if found == Some(*node) {
                hits += 1;
            } else if example.is_empty() {
                example = format!(
                    "（未命中示例：节点 {} → {:?}）",
                    node.index(),
                    found.map(|n| n.index())
                );
            }
        }
        let (_, begin, end) = samples[0];
        let pages: BTreeSet<usize> = samples.iter().map(|(_, b, _)| b.page).collect();
        println!(
            "  {kind_name} 反查：{hits} / {} 命中（覆盖 {} 页，第 {:?}..{:?} 页）；示例 节点 {} 第 {} 页 x∈[{:.1}, {:.1}] {example}",
            samples.len(),
            pages.len(),
            pages.iter().next(),
            pages.iter().next_back(),
            samples[0].0.index(),
            begin.page,
            begin.x.min(end.x),
            begin.x.max(end.x),
        );
    }

    // 源码范围反查。
    if let Some((node, (start, _))) = generation.spans.iter().nth(generation.spans.len() / 2) {
        if let Some(found) = generation.node_at(start + 1) {
            println!(
                "源码偏移 {} → 节点 {}（期望 {}）",
                start + 1,
                found.index(),
                node.index()
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
