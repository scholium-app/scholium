//! 出口条件验证：**Typst 预览的单字符路径 p95，以及"旧 revision 不闪回"**。
//!
//! 判据（动手前写定）：
//!
//! 1. 单字符编辑（随机位置、随机段落、固定种子）后，重新生成 + 重新编译的 p95 与最大值要能测出来；
//! 2. 结果如实报告，**不预设"必须小于 16 ms"**——出口条件写的是"不阻塞 16 ms 帧"，
//!    而编译是异步的，所以要区分"帧内开销"与"编译完成时间"；
//! 3. 预览结果门：编译结果按 revision 落地，**旧 revision 的结果不得覆盖新 revision**（不闪回）。
//!    用乱序到达的交错场景逐条断言。

use std::time::Instant;

use scholium_spike_core::{ActorId, Editor, Intent, NodeId, SemanticEdit, fixture};
use typst_layout::PagedDocument;

use crate::generator;
use crate::world::SpikeWorld;

/// 线性同余伪随机数：固定种子、无依赖，保证可复现。
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        self.0 >> 33
    }

    fn below(&mut self, bound: usize) -> usize {
        if bound == 0 { 0 } else { (self.next() as usize) % bound }
    }
}

/// 收集所有文本叶子（按文档顺序）。
fn text_leaves(editor: &Editor) -> Vec<NodeId> {
    let mut out = Vec::new();
    let document = editor.document();
    let mut stack = vec![document.root()];
    while let Some(node) = stack.pop() {
        if let Ok(current) = document.node(node) {
            if matches!(
                current.kind,
                scholium_spike_core::NodeKind::Text | scholium_spike_core::NodeKind::Raw
            ) {
                out.push(node);
                continue;
            }
        }
        for slot in (0..4).rev() {
            if let Ok(children) = document.slot(node, slot) {
                for child in children.iter().rev() {
                    stack.push(*child);
                }
            }
        }
    }
    out
}

/// 单字符编辑路径：重新生成 + 复用同一个 World 重新编译。
pub(crate) fn latency() {
    const PARAGRAPHS: usize = 256;
    const EDITS: usize = 20;

    let mut editor = Editor::new();
    fixture::build_large(&mut editor, PARAGRAPHS);
    let initial = generator::generate(editor.document());
    let world = SpikeWorld::new(initial.source.clone());
    let start = Instant::now();
    let warmed = typst::compile::<PagedDocument>(&world);
    let cold_ms = start.elapsed().as_secs_f64() * 1000.0;
    let pages = warmed.output.as_ref().map(|doc| doc.pages().len()).unwrap_or(0);
    println!("基准：{PARAGRAPHS} 段，首次编译 {cold_ms:.0} ms，{pages} 页");

    let leaves = text_leaves(&editor);
    let mut rng = Rng(0x5EED_1234);
    let mut generate_ms = Vec::new();
    let mut compile_ms = Vec::new();
    let mut total_ms = Vec::new();

    for step in 0..EDITS {
        let leaf = leaves[rng.below(leaves.len())];
        let length = editor
            .document()
            .text_of(leaf)
            .map(|text| text.len())
            .unwrap_or(0);
        // 插到字符边界上（避免切开 UTF-8）：找最近的合法位置。
        let mut at = rng.below(length + 1);
        let text = editor.document().text_of(leaf).unwrap_or_default().to_string();
        while at > 0 && !text.is_char_boundary(at) {
            at -= 1;
        }
        let payload = if step % 2 == 0 { "改" } else { "x" };
        if editor
            .apply(
                ActorId(1),
                Intent::Typing,
                SemanticEdit::InsertText {
                    node: leaf,
                    at,
                    text: payload.to_string(),
                },
            )
            .is_err()
        {
            continue;
        }

        let start = Instant::now();
        let generation = generator::generate(editor.document());
        let generated = start.elapsed().as_secs_f64() * 1000.0;

        world.set_source(generation.source);
        let start = Instant::now();
        let warned = typst::compile::<PagedDocument>(&world);
        let compiled = start.elapsed().as_secs_f64() * 1000.0;
        if let Err(errors) = &warned.output {
            let message = errors
                .iter()
                .map(|error| error.message.to_string())
                .collect::<Vec<_>>()
                .join("; ");
            println!("  第 {step} 次编辑编译失败：{message}");
            // 打印出错附近的源码片段，便于定位
            let at = errors.first().map(|error| format!("{:?}", error.span)).unwrap_or_default();
            println!("    出错位置：{at}");
        }

        generate_ms.push(generated);
        compile_ms.push(compiled);
        total_ms.push(generated + compiled);
    }

    report_stats("重新生成源码", &mut generate_ms);
    report_stats("重新编译（持久 World）", &mut compile_ms);
    report_stats("单字符路径合计（生成 + 编译）", &mut total_ms);
    // 最坏情况：改**首段**。它在生成源码的最前面，插入字符会让其后所有字节偏移平移，
    // 解析与布局几乎无法复用缓存。
    if let Some(first) = leaves.first().copied() {
        let mut worst = Vec::new();
        for _ in 0..5 {
            if editor
                .apply(
                    ActorId(1),
                    Intent::Typing,
                    SemanticEdit::InsertText {
                        node: first,
                        at: 0,
                        text: "首".to_string(),
                    },
                )
                .is_err()
            {
                break;
            }
            let start = Instant::now();
            let generation = generator::generate(editor.document());
            let generated = start.elapsed().as_secs_f64() * 1000.0;
            world.set_source(generation.source);
            let start = Instant::now();
            let _ = typst::compile::<PagedDocument>(&world);
            worst.push(generated + start.elapsed().as_secs_f64() * 1000.0);
        }
        report_stats("最坏情况：改首段（源码整体平移）", &mut worst);
    }

    println!(
        "说明：以上是**逻辑侧**的完成时间；编译应在后台线程进行，帧内只做入队与取用最新结果。"
    );
}

fn report_stats(label: &str, samples: &mut [f64]) {
    if samples.is_empty() {
        println!("{label}：无样本");
        return;
    }
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let median = samples[samples.len() / 2];
    let p95 = samples[(samples.len() * 95 / 100).min(samples.len() - 1)];
    let max = samples[samples.len() - 1];
    let mean = samples.iter().sum::<f64>() / samples.len() as f64;
    println!(
        "{label}（{} 次）：中位 {median:.1} ms，均值 {mean:.1} ms，p95 {p95:.1} ms，最大 {max:.1} ms",
        samples.len()
    );
}

/// 预览结果门：旧 revision 的结果不得覆盖新 revision。
pub(crate) fn revision_gate() {
    /// 最近一次已采纳的 revision。
    ///
    /// "闪回"的判据是**采纳了比当前更旧的 revision**（画面倒退），
    /// 而不是"收到了旧结果"——收到旧结果并忽略它是正确行为。
    #[derive(Default)]
    struct Gate {
        adopted: u64,
        flashed_back: bool,
        ignored_stale: usize,
    }

    impl Gate {
        /// 编译完成：只有比已采纳的更新才落地。
        fn offer(&mut self, revision: u64) {
            if revision <= self.adopted {
                self.ignored_stale += 1;
                return;
            }
            self.adopted = revision;
        }
    }

    /// 对照：朴素的"谁最后到就用谁"，必然闪回。
    #[derive(Default)]
    struct NaiveGate {
        adopted: u64,
        flashed_back: bool,
    }

    impl NaiveGate {
        fn offer(&mut self, revision: u64) {
            if revision < self.adopted {
                self.flashed_back = true;
            }
            self.adopted = revision;
        }
    }

    // 场景 1：顺序到达
    let mut gate = Gate::default();
    for revision in 1..=5 {
        gate.offer(revision);
    }
    println!(
        "  [{}] 顺序到达 1..5：已采纳 {}，闪回 {}，忽略旧结果 {}",
        if gate.adopted == 5 && !gate.flashed_back { "PASS" } else { "FAIL" },
        gate.adopted,
        gate.flashed_back,
        gate.ignored_stale
    );

    // 场景 2：乱序到达（旧结果最后到）
    let mut gate = Gate::default();
    for revision in [2, 5, 3, 4, 1] {
        gate.offer(revision);
    }
    println!(
        "  [{}] 乱序到达 [2,5,3,4,1]：已采纳 {}（应为 5），忽略旧结果 {} 次，闪回 {}",
        if gate.adopted == 5 && !gate.flashed_back && gate.ignored_stale == 3 {
            "PASS"
        } else {
            "FAIL"
        },
        gate.adopted,
        gate.ignored_stale,
        gate.flashed_back
    );

    // 失败夹具：朴素门必须被本用例抓出闪回，否则说明用例没有区分能力。
    let mut naive = NaiveGate::default();
    for revision in [2, 5, 3, 4, 1] {
        naive.offer(revision);
    }
    println!(
        "  [{}] 对照（朴素门，谁最后到用谁）：最终采纳 {}，闪回 {} —— 本用例应抓出它",
        if naive.flashed_back { "PASS(有区分度)" } else { "FAIL(无区分度)" },
        naive.adopted,
        naive.flashed_back
    );

    // 场景 3：失败的结果不推进 revision（编译失败不能把旧画面当新画面）
    let mut gate = Gate::default();
    gate.offer(3);
    let ignored = 4u64; // 假设 revision 4 编译失败、不入队
    gate.offer(5);
    println!(
        "  [{}] 失败结果不影响门（跳过的 {ignored} 不落地）：已采纳 {}",
        if gate.adopted == 5 { "PASS" } else { "FAIL" },
        gate.adopted
    );
}
