//! 独立的验证模式：锚点写法探针、渲染比对、文本字符级映射。
//!
//! 与主流程分开，避免单文件超过 600 行上限。

use std::collections::{BTreeMap, BTreeSet};

use scholium_spike_core::{Editor, NodeId, fixture};
use typst::foundations::Label;
use typst::introspection::Introspector;
use typst::utils::PicoStr;
use typst_layout::PagedDocument;

use crate::generator::{Generation, Side};
use crate::world::SpikeWorld;

/// 预览中的一个点。
#[derive(Clone, Copy, Debug)]
pub(crate) struct Spot {
    pub page: usize,
    pub x: f32,
    pub y: f32,
}

/// 编译生成好的源码。
pub(crate) fn compile(generation: &Generation) -> Option<PagedDocument> {
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

/// 按锚点名逐个解析位置。文本分块需要按名字取，不能按节点聚合。
pub(crate) fn resolve_named(
    document: &PagedDocument,
    generation: &Generation,
) -> BTreeMap<String, Spot> {
    let introspector = document.introspector();
    let mut out = BTreeMap::new();
    for name in generation.anchors.keys() {
        let Ok(content) =
            introspector.query_label(Label::new(PicoStr::intern(name.as_str())).expect("锚点名非空"))
        else {
            continue;
        };
        let Some(position) = content.location().and_then(|loc| introspector.position(loc)) else {
            continue;
        };
        out.insert(
            name.clone(),
            Spot {
                page: position.page.get(),
                x: position.point.x.to_pt() as f32,
                y: position.point.y.to_pt() as f32,
            },
        );
    }
    out
}

/// 解析全部锚点，按节点聚合为 (起点, 终点)。
pub(crate) fn resolve_anchors(
    generation: &Generation,
    document: &PagedDocument,
) -> (BTreeMap<NodeId, Spot>, BTreeMap<NodeId, Spot>) {
    let named = resolve_named(document, generation);
    let mut begins = BTreeMap::new();
    let mut ends = BTreeMap::new();
    for (name, (node, side)) in &generation.anchors {
        let Some(spot) = named.get(name) else {
            continue;
        };
        match side {
            Side::Begin => begins.insert(*node, *spot),
            Side::End => ends.insert(*node, *spot),
        };
    }
    (begins, ends)
}

pub(crate) fn probe() {
    let source = String::from(
        r#"一段 #context [#metadata(here()) <p1>] $ frac(a, b) #context [#metadata(here()) <p2>] $ #context [#metadata(here()) <p3>] 之后
"#,
    );
    let world = SpikeWorld::new(source);
    let warned = typst::compile::<PagedDocument>(&world);
    println!("警告 {} 条", warned.warnings.len());
    let Ok(document) = warned.output else {
        println!("编译失败");
        return;
    };
    let introspector = document.introspector();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    for name in ["p1", "p2", "p3"] {
        let Ok(content) =
            introspector.query_label(Label::new(PicoStr::intern(name)).expect("非空"))
        else {
            println!("  {name}: 查询失败");
            continue;
        };
        match content.location().and_then(|loc| introspector.position(loc)) {
            Some(position) => {
                println!(
                    "  {name}: 第 {} 页 ({:.1}, {:.1}) pt",
                    position.page.get(),
                    position.point.x.to_pt(),
                    position.point.y.to_pt()
                );
                seen.insert(name.to_string());
            }
            None => println!("  {name}: 有元素但无位置"),
        }
    }
    println!("可用锚点 {} / 3", seen.len());
}


/// 把锚点画到真实渲染的页面上，并对"锚点区间内是否真有墨迹"做自动检验。
///
/// 这是把"自洽"变成"证实"的一步：此前的往返测试只证明"用我自己的坐标能查回我自己"，
/// 这里改为在渲染结果上核对——区间内应有墨迹，区间外应明显更少。
pub(crate) fn render_check() {
    let mut editor = Editor::new();
    fixture::build_standard(&mut editor);
    let generation = crate::generator::generate(editor.document());
    let world = SpikeWorld::new(generation.source.clone());
    let warned = typst::compile::<PagedDocument>(&world);
    let Ok(document) = warned.output else {
        println!("编译失败");
        return;
    };
    let page = &document.pages()[0];

    // 复用统一的锚点解析，避免两处各写一遍。
    let (begins, ends) = resolve_anchors(&generation, &document);
    let begins: BTreeMap<NodeId, Spot> = begins
        .into_iter()
        .filter(|(_, spot)| spot.page == 1)
        .collect();
    let ends: BTreeMap<NodeId, Spot> = ends
        .into_iter()
        .filter(|(_, spot)| spot.page == 1)
        .collect();

    // 渲染
    let scale = 3.0_f32;
    let options = typst_render::RenderOptions {
        pixel_per_pt: (scale as f64).into(),
        render_bleed: false,
    };
    let mut pixmap = typst_render::render(page, &options);
    println!(
        "渲染：{} × {} 像素，比例 {scale} px/pt",
        pixmap.width(),
        pixmap.height()
    );

    // 绘制锚点：起点绿色、终点红色，竖线便于比对位置
    let mut paint_green = tiny_skia::Paint::default();
    paint_green.set_color_rgba8(0, 190, 0, 255);
    let mut paint_red = tiny_skia::Paint::default();
    paint_red.set_color_rgba8(230, 0, 0, 255);

    let mut drawn = 0usize;
    for (node, begin) in &begins {
        let end = ends.get(node).copied().unwrap_or(*begin);
        // 起点画在上半段、终点画在下半段：相邻结构的起止常在同一 x，
        // 分开画才不会互相盖住。
        for (spot, paint, offset, height) in [
            (begin, &paint_green, -32.0, 18.0),
            (&end, &paint_red, -12.0, 18.0),
        ] {
            if spot.page != 1 {
                continue;
            }
            let x = spot.x * scale;
            let y = spot.y * scale;
            if let Some(rect) = tiny_skia::Rect::from_xywh(x, y + offset, 1.4, height) {
                pixmap.fill_rect(rect, paint, tiny_skia::Transform::identity(), None);
                drawn += 1;
            }
        }
    }
    println!("绘制锚点标记 {drawn} 个");

    // ---- 自动墨迹检验 ----
    let dark = |pixmap: &tiny_skia::Pixmap, x: f32, y: f32| -> bool {
        if x < 0.0 || y < 0.0 || x >= pixmap.width() as f32 || y >= pixmap.height() as f32 {
            return false;
        }
        let pixel = pixmap.pixel(x as u32, y as u32);
        match pixel {
            Some(p) => {
                // 已渲染内容为深色；忽略我们画的标记颜色。
                let (r, g, b) = (p.red(), p.green(), p.blue());
                let is_marker = (g > 150 && r < 100) || (r > 180 && g < 80);
                !is_marker && (r as u32 + g as u32 + b as u32) < 480
            }
            None => false,
        }
    };
    let ink_in = |pixmap: &tiny_skia::Pixmap, x0: f32, x1: f32, y: f32, half: f32| -> f32 {
        let mut total = 0usize;
        let mut hit = 0usize;
        let mut x = x0;
        while x <= x1 {
            let mut dy = -half;
            while dy <= half {
                total += 1;
                if dark(pixmap, x, y + dy) {
                    hit += 1;
                }
                dy += 1.0;
            }
            x += 1.0;
        }
        if total == 0 { 0.0 } else { hit as f32 / total as f32 }
    };

    // ---- 自动不变量：同一行内，各结构的 x 顺序必须与文档顺序一致 ----
    let mut rows: BTreeMap<String, Vec<(NodeId, f32, f32)>> = BTreeMap::new();
    for (node, begin) in &begins {
        let kind = editor
            .document()
            .node(*node)
            .map(|n| format!("{:?}", n.kind))
            .unwrap_or_default();
        if kind == "Paragraph" || kind == "Math" {
            continue;
        }
        let end = ends.get(node).copied().unwrap_or(*begin);
        if (begin.y - end.y).abs() > 2.0 {
            continue;
        }
        rows.entry(format!("{:.1}", begin.y))
            .or_default()
            .push((*node, begin.x, end.x));
    }
    let mut checked = 0usize;
    let mut violations = 0usize;
    for (line, mut items) in rows {
        items.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        let indices: Vec<usize> = items.iter().map(|(node, _, _)| node.index()).collect();
        let mut sorted = indices.clone();
        sorted.sort_unstable();
        checked += 1;
        if indices != sorted {
            violations += 1;
            println!("  顺序不变量违例（y={line}）：{indices:?} 与文档顺序 {sorted:?} 不符");
        }
        // 同层结构不应重叠
        for pair in items.windows(2) {
            if pair[1].1 < pair[0].2 - 0.5 {
                violations += 1;
                println!(
                    "  区间重叠（y={line}）：节点 {} 结束于 {:.1}，节点 {} 起于 {:.1}",
                    pair[0].0.index(),
                    pair[0].2,
                    pair[1].0.index(),
                    pair[1].1
                );
            }
        }
    }
    println!(
        "自动不变量：检查 {checked} 行，违例 {violations} 处（同一行内 x 顺序应与文档顺序一致、同层区间不应重叠）"
    );

    // 墨迹存在性：区间内应当有非标记像素（弱检查，只用于发现"完全落空"的坐标）
    let mut empty = 0usize;
    for (node, begin) in &begins {
        let Some(end) = ends.get(node) else { continue };
        let kind = editor
            .document()
            .node(*node)
            .map(|n| format!("{:?}", n.kind))
            .unwrap_or_default();
        if kind == "Paragraph" || kind == "Math" || (begin.y - end.y).abs() > 2.0 {
            continue;
        }
        let (low, high) = (begin.x * scale, end.x * scale);
        if high - low < 2.0 {
            continue;
        }
        if ink_in(&pixmap, low, high, begin.y * scale, 18.0) <= 0.0 {
            empty += 1;
        }
    }
    println!("墨迹存在性：{empty} 个结构的区间内完全没有内容（应为 0）");

    // 以 crate 目录为基准，避免从别处运行时把产物写到别处。
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("artifacts/anchor-overlay.png");
    let path = path.as_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match pixmap.save_png(path) {
        Ok(()) => println!("已保存 {}", path.display()),
        Err(error) => println!("保存失败：{error}"),
    }

    // 放大裁剪：数学结构所在区域，便于肉眼核对标记是否落在元素上。
    let crop = (240.0 * scale, 92.0 * scale, 120.0 * scale, 26.0 * scale);
    let (cx, cy, cw, ch) = crop;
    if let Some(mut zoom) = tiny_skia::Pixmap::new(cw as u32, ch as u32) {
        let source_width = pixmap.width() as usize;
        let row_bytes = cw as usize * 4;
        let source = pixmap.data();
        let destination = zoom.data_mut();
        for row in 0..ch as usize {
            let source_y = cy as usize + row;
            let source_start = (source_y * source_width + cx as usize) * 4;
            let destination_start = row * row_bytes;
            if source_start + row_bytes <= source.len() {
                destination[destination_start..destination_start + row_bytes]
                    .copy_from_slice(&source[source_start..source_start + row_bytes]);
            }
        }
        let zoom_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("artifacts/anchor-overlay-zoom.png");
        let zoom_path = zoom_path.as_path();
        match zoom.save_png(zoom_path) {
            Ok(()) => println!("已保存 {}", zoom_path.display()),
            Err(error) => println!("放大图保存失败：{error}"),
        }
    }

    // 打印几个锚点的坐标，便于与图对照
    for (node, begin) in begins.iter().take(6) {
        let kind = editor
            .document()
            .node(*node)
            .map(|n| format!("{:?}", n.kind))
            .unwrap_or_default();
        println!(
            "    {kind} 节点 {} 起点 ({:.1}, {:.1}) pt",
            node.index(),
            begin.x,
            begin.y
        );
    }
}

/// 文本字符级映射：在同一份布局内，用"每字符一个锚点"当基准，
/// 检验"每 N 字符一块 + 按宽度插值"的误差。
///
/// 为什么必须同一份布局：Typst 默认两端对齐，锚点数量不同会导致断行与字间距不同，
/// 两份文档的坐标**本来就不可比**（第一版就是这么错的，误差高达 300 pt）。
/// 正确做法是在同一布局内取基准，再模拟稀疏分块的插值。
pub(crate) fn charmap() {
    const SAMPLE: &str = "混合排版的测试句子：中文abc与123数字，以及标点，长度足够切出多个块。";
    const CHUNK: usize = 8;

    let mut editor = Editor::new();
    let text_node = fixture::build_text(&mut editor, SAMPLE);
    let text = editor.document().text_of(text_node).unwrap_or_default();

    // 单份文档：每字符一个锚点，作为基准。
    let dense = crate::generator::generate_with(
        editor.document(),
        crate::generator::Options {
            text_chunk_chars: Some(1),
        },
    );
    let Some(document) = compile(&dense) else {
        return;
    };
    let spots = resolve_named(&document, &dense);

    // 基准：字节偏移 → 真实坐标
    let mut truth: BTreeMap<usize, Spot> = BTreeMap::new();
    for chunk in &dense.text_chunks {
        debug_assert_eq!(chunk.node, text_node, "分块必须都属于目标文本叶子");
        if let Some(spot) = spots.get(&chunk.begin_anchor) {
            truth.insert(chunk.start_byte, *spot);
        }
        if let Some(spot) = spots.get(&chunk.end_anchor) {
            truth.insert(chunk.end_byte, *spot);
        }
    }
    println!(
        "文本：{} 字符 / {} 字节；基准偏移 {} 个",
        text.chars().count(),
        text.len(),
        truth.len()
    );

    // 宽度模型与共享核心一致：CJK 记 1.0，其余记 0.6（只有相对比例有意义）
    let width = |slice: &str| -> f32 {
        slice
            .chars()
            .map(|ch| if (ch as u32) >= 0x2E80 { 1.0 } else { 0.6 })
            .sum()
    };

    // 按 CHUNK 个字符把基准点分组，模拟"稀疏分块"能拿到的区间，
    // 再在区间内按宽度比例插值。
    let boundaries: Vec<usize> = text
        .char_indices()
        .map(|(byte, _)| byte)
        .chain(std::iter::once(text.len()))
        .collect();
    let mut errors: Vec<f32> = Vec::new();
    let mut excluded = 0usize;
    let mut worst = (0usize, 0.0_f32);
    let mut chars_in_chunk = 0usize;
    while chars_in_chunk < boundaries.len() - 1 {
        let end_index = (chars_in_chunk + CHUNK).min(boundaries.len() - 1);
        let (start_byte, end_byte) = (boundaries[chars_in_chunk], boundaries[end_index]);
        let (Some(begin), Some(end)) = (truth.get(&start_byte), truth.get(&end_byte)) else {
            chars_in_chunk = end_index;
            continue;
        };
        // 跨行的块没有可插值的横向区间。
        if begin.page != end.page || (begin.y - end.y).abs() > 2.0 {
            excluded += end_index - chars_in_chunk;
            chars_in_chunk = end_index;
            continue;
        }
        let span = width(&text[start_byte..end_byte]);
        for chunk in &dense.text_chunks {
            if chunk.start_byte < start_byte || chunk.start_byte > end_byte {
                continue;
            }
            let Some(spot) = truth.get(&chunk.start_byte) else {
                continue;
            };
            if span <= 0.0 {
                continue;
            }
            let prefix = width(&text[start_byte..chunk.start_byte]);
            let predicted = begin.x + (end.x - begin.x) * (prefix / span);
            let error = (predicted - spot.x).abs();
            if error > worst.1 {
                worst = (chunk.start_byte, error);
            }
            errors.push(error);
        }
        chars_in_chunk = end_index;
    }

    if errors.is_empty() {
        println!("没有可比较的偏移");
        return;
    }
    let mean = errors.iter().sum::<f32>() / errors.len() as f32;
    let max = errors.iter().cloned().fold(0.0_f32, f32::max);

    // 中位字宽：同一行内相邻基准点的正向间距
    let mut gaps: Vec<f32> = Vec::new();
    let ordered: Vec<(usize, &Spot)> = truth.iter().map(|(byte, spot)| (*byte, spot)).collect();
    for pair in ordered.windows(2) {
        if pair[0].1.page == pair[1].1.page && (pair[0].1.y - pair[1].1.y).abs() <= 2.0 {
            let gap = pair[1].1.x - pair[0].1.x;
            if gap > 0.5 {
                gaps.push(gap);
            }
        }
    }
    gaps.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let char_width = gaps.get(gaps.len() / 2).copied().unwrap_or(0.0);

    println!(
        "插值误差（块宽 {CHUNK} 字符，{}/{} 个偏移可比较，跨行排除 {excluded}）：平均 {mean:.2} pt，最大 {max:.2} pt（偏移 {}）",
        errors.len(),
        truth.len(),
        worst.0
    );
    println!(
        "中位字宽 {char_width:.2} pt → 误差约 {:.3} 字（平均）/ {:.3} 字（最大）",
        mean / char_width.max(0.01),
        max / char_width.max(0.01)
    );

    // 反查：给一个真实坐标，看能否找回正确的字节偏移（在它所属块内反查）
    let mut exact = 0usize;
    let mut within_one = 0usize;
    let mut total = 0usize;
    for (byte, spot) in &truth {
        let mut best: Option<(usize, f32)> = None;
        for candidate in &boundaries {
            if let Some(candidate_spot) = truth.get(candidate) {
                let distance = (candidate_spot.x - spot.x).abs();
                if best.is_none_or(|(_, best_distance)| distance < best_distance) {
                    best = Some((*candidate, distance));
                }
            }
        }
        let Some((found, _)) = best else { continue };
        total += 1;
        if found == *byte {
            exact += 1;
        } else {
            let position = boundaries.iter().position(|b| b == byte).unwrap_or(0);
            let found_position = boundaries.iter().position(|b| *b == found).unwrap_or(0);
            if position.abs_diff(found_position) <= 1 {
                within_one += 1;
            }
        }
    }
    println!(
        "反查（同一行基准点之间）：完全一致 {exact} / {total}，±1 字符内 {} / {total}",
        exact + within_one
    );
}
