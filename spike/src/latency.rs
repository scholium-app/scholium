//! P0 验证 #2：增量编译延迟。
//!
//! 问题：单字符编辑后重新编译，能否进 16ms（一帧）？
//! 通过标准：目标 <16ms，可接受 <50ms。
//!
//! 不通过的退路：降级块级编译，全量放到 idle。
//!
//! 注意：Typst 的 memoization 由 comemo 管理，需要 `comemo::evict` 才能观察到
//! 缓存行为的真实代价，否则缓存会无限增长。这里模拟真实编辑器：不主动 evict，
//! 但记录内存增长趋势。

mod world;

use std::time::{Duration, Instant};
use typst_layout::PagedDocument;
use world::SpikeWorld;

/// 生成 n 页规模的测试文档：正文 + 数学混排。
fn make_doc(paragraphs: usize) -> String {
    let mut s = String::from("= 测试文档\n\n");
    for i in 0..paragraphs {
        s.push_str(&format!(
            "== 第 {i} 节\n\n\
             这是一段中英混排的正文 text，用来占据版面并触发断行。\
             行内公式 $a_{i} + b^{i}$ 混在句子里。\n\n\
             $ sum_(k=1)^{i} frac(x_k, y_k) = sqrt(z_{i}) $\n\n\
             $ mat(1, 2; 3, 4) times vec(alpha, beta) $\n\n"
        ));
    }
    s
}

fn compile_once(world: &SpikeWorld) -> Result<usize, String> {
    let result = typst::compile::<PagedDocument>(world);
    match result.output {
        Ok(doc) => Ok(doc.pages().len()),
        Err(errors) => Err(errors
            .iter()
            .map(|e| e.message.to_string())
            .collect::<Vec<_>>()
            .join("; ")),
    }
}

fn percentile(sorted: &[Duration], p: f64) -> Duration {
    if sorted.is_empty() {
        return Duration::ZERO;
    }
    let idx = ((sorted.len() as f64 - 1.0) * p).round() as usize;
    sorted[idx]
}

/// 对一个文档规模做一轮测量。
fn measure(paragraphs: usize, edits: usize) {
    let text = make_doc(paragraphs);
    let byte_len = text.len();
    let mut world = SpikeWorld::new(text);

    // 冷启动：首次编译含字体索引、库构建
    let t0 = Instant::now();
    let pages = match compile_once(&world) {
        Ok(p) => p,
        Err(e) => {
            println!("  编译失败: {e}");
            return;
        }
    };
    let cold = t0.elapsed();

    // 无改动重编译：纯缓存命中的下界
    let t1 = Instant::now();
    let _ = compile_once(&world);
    let cached = t1.elapsed();

    // 单字符编辑：插到文档中部的正文里。
    //
    // 不能简单取 byte_len/2——那可能落在 `sum` 之类的标识符中间，
    // 把它改成 `sxum` 导致编译错误，测出来的就不是排版延迟了。
    // 锚点选正文里的固定串，插到它后面的空格处，保证语法合法。
    let mut samples = Vec::with_capacity(edits);
    const ANCHOR: &str = "混排的正文 ";
    let insert_at = {
        let src = world.source_ref().text().to_string();
        // 中点要先对齐到字符边界，否则切片会 panic（文档含 CJK）
        let mut mid = src.len() / 2;
        while mid > 0 && !src.is_char_boundary(mid) {
            mid -= 1;
        }
        // 从中点往后找第一个锚点，找不到就往前找
        let found = src[mid..]
            .find(ANCHOR)
            .map(|i| mid + i + ANCHOR.len())
            .or_else(|| src[..mid].rfind(ANCHOR).map(|i| i + ANCHOR.len()));
        match found {
            Some(at) => at,
            None => {
                println!("  找不到插入锚点，跳过");
                return;
            }
        }
    };
    debug_assert!(world.source_ref().text().is_char_boundary(insert_at));

    for _ in 0..edits {
        world.edit(insert_at, "x");
        let t = Instant::now();
        if let Err(e) = compile_once(&world) {
            println!("  编辑后编译失败: {e}");
            return;
        }
        samples.push(t.elapsed());
    }

    samples.sort_unstable();
    let sum: Duration = samples.iter().sum();
    let mean = sum / samples.len() as u32;

    println!(
        "  {:>3} 节 / {:>2} 页 / {:>6} 字节",
        paragraphs, pages, byte_len
    );
    println!("    冷启动:       {:>8.2?}", cold);
    println!("    无改动重编译: {:>8.2?}", cached);
    println!(
        "    单字符编辑:   mean {:>7.2?}  p50 {:>7.2?}  p95 {:>7.2?}  max {:>7.2?}",
        mean,
        percentile(&samples, 0.50),
        percentile(&samples, 0.95),
        samples[samples.len() - 1],
    );

    let p95 = percentile(&samples, 0.95);
    let verdict = if p95 < Duration::from_millis(16) {
        "✅ 达标 (<16ms)"
    } else if p95 < Duration::from_millis(50) {
        "⚠️  可接受 (<50ms)，需 debounce"
    } else {
        "❌ 超标，需块级编译"
    };
    println!("    判定:         {verdict}\n");
}

fn main() {
    println!("=== Typst 增量编译延迟 ===");
    println!("每档 30 次单字符编辑，报告 p50/p95\n");

    for paragraphs in [1, 5, 15, 40] {
        measure(paragraphs, 30);
    }

    println!("=== 大文档 ===");
    println!("PLAN 里的目标规模是 30 页，以及超出预期的压力档\n");
    for paragraphs in [150, 400] {
        measure(paragraphs, 20);
    }

    println!("=== comemo 缓存驱逐后的表现 ===");
    println!("模拟长时间编辑后缓存被清理的最坏情况\n");
    comemo::evict(0);
    measure(15, 10);
}
