//! 预览延迟链测量（[报告 0045](../../docs/spikes/SPK-0045-render-latency-baseline.md)）。
//!
//! 通过 `PreviewCompiler` 的公开 API 复现 Visual 模式击键后的异步链：
//! 提交 → 编译 → 采纳 → 请求页 → 光栅化。编译段的耗时由 worker 自报；
//! 光栅段在独立线程上以与 `scholium_typst::compiler::raster` 相同的调用
//! 测量（pixel_per_pt = PIXELS_PER_PT = 2.0）。
//!
//! 运行：`cargo run --release --manifest-path spikes/render-latency/Cargo.toml`

use std::time::{Duration, Instant};

use scholium_model::{Block, BlockKind, DocumentId, Inline, NodeId, Revision};
use scholium_typst::{CompileOutcome, MAX_PREVIEW_PAGES, PreviewCompiler, PreviewEvent};
use typst::LibraryExt;

/// 光栅化比例，与 `scholium_typst::PIXELS_PER_PT` 同值（主 crate 未导出该常量）。
const PIXELS_PER_PT: f32 = 2.0;

/// 每档文档规模的段落数（与报告 0016 的夹具规模对齐）。
const SIZES: [usize; 3] = [8, 64, 256];
/// 每个规模的采样击键次数（首击含冷启动，另取 N 次热态）。
const WARM_SAMPLES: usize = 10;
/// 等待编译结果的上限；超时视为环境异常而不是性能数据。
const COMPILE_TIMEOUT: Duration = Duration::from_secs(120);

fn main() {
    println!("scholium 预览延迟链测量\n");
    println!(
        "版本组：typst 0.15.1 / typst-kit / typst-layout / typst-render（与主 workspace 同锁）"
    );
    print_host();
    for &blocks in &SIZES {
        measure(blocks);
    }
}

fn print_host() {
    let os = std::fs::read_to_string("/etc/os-release")
        .ok()
        .and_then(|text| {
            text.lines()
                .find(|line| line.starts_with("PRETTY_NAME"))
                .map(|line| {
                    line.trim_start_matches("PRETTY_NAME=")
                        .trim_matches('"')
                        .to_owned()
                })
        })
        .unwrap_or_else(|| "unknown".into());
    let cpus = std::thread::available_parallelism().map_or(0, |n| n.get());
    println!("平台：{os} · {cpus} 逻辑核\n");
}

/// 构造 blocks 个段落的快照；含中英混排与一个行内公式，覆盖常见字形路径。
fn snapshot(blocks: usize, revision: u64) -> scholium_model::DocumentSnapshot {
    let mut body = Vec::new();
    for i in 0..blocks {
        body.push(Block {
            node: NodeId::fresh(),
            kind: if i == 0 {
                BlockKind::Heading1
            } else {
                BlockKind::Paragraph
            },
            content: vec![
                Inline::Text(format!("第{i}段：中文正文与 English text 混排，")),
                Inline::Math("alpha + x/2".into()),
                Inline::Text("结尾。".into()),
            ],
        });
    }
    scholium_model::DocumentSnapshot {
        document: DocumentId::fresh(),
        revision: Revision(revision),
        blocks: body,
    }
}

fn measure(blocks: usize) {
    println!("=== {blocks} 段 ===");
    let mut compiler = PreviewCompiler::spawn();
    let mut doc = snapshot(blocks, 1);

    // 冷启动：首次编译包含系统字体扫描，单列不计入热态分布。
    compiler.submit_snapshot(&doc);
    let cold = wait_compile(&mut compiler, &doc);
    println!("  冷启动首编（含字体扫描）：{:>8.1} ms", cold.elapsed_ms);
    raster_baseline(&doc);

    let mut compile_ms = Vec::new();
    let mut raster_ms = Vec::new();
    for i in 0..WARM_SAMPLES {
        // 单字符击键：只改最后一段一个字，模拟连续输入的最小增量。
        doc.revision = Revision(doc.revision.0 + 1);
        let mut text = doc.blocks.last().expect("non-empty").markup_text();
        text.push('字');
        doc.blocks.last_mut().expect("non-empty").content = parse_line(&text);

        compiler.submit_snapshot(&doc);
        let outcome = wait_compile(&mut compiler, &doc);
        compile_ms.push(outcome.elapsed_ms as f64);
        raster_ms.push(raster_once(&doc));
        if i + 1 == WARM_SAMPLES {
            println!(
                "  geometry cells: {} 页 · {:>6} cells",
                outcome.geometry.len(),
                outcome
                    .geometry
                    .iter()
                    .map(|g| g.cells.len())
                    .sum::<usize>()
            );
        }
    }
    report("热态编译段", &compile_ms);
    report("热态整页光栅段", &raster_ms);

    let chain: Vec<f64> = compile_ms
        .iter()
        .zip(&raster_ms)
        .map(|(c, r)| c + r)
        .collect();
    report("击键→字形可见（编译+光栅）", &chain);
    println!();
}

fn report(label: &str, samples: &[f64]) {
    let mut sorted = samples.to_vec();
    sorted.sort_by(|a, b| a.total_cmp(b));
    let at = |q: f64| -> f64 {
        let idx = ((sorted.len() as f64 - 1.0) * q).round() as usize;
        sorted[idx.min(sorted.len() - 1)]
    };
    println!(
        "  {label:<18} p50 {:>7.1} ms · p95 {:>7.1} ms · max {:>7.1} ms",
        at(0.5),
        at(0.95),
        sorted[sorted.len() - 1]
    );
}

/// 轮询直到当前 revision 的编译结果到达。
fn wait_compile(
    compiler: &mut PreviewCompiler,
    doc: &scholium_model::DocumentSnapshot,
) -> CompileOutcome {
    let start = Instant::now();
    loop {
        if let Some(PreviewEvent::Compiled(outcome)) = compiler.poll() {
            assert_eq!(outcome.revision, doc.revision.0, "worker 只回最新请求");
            assert!(outcome.error.is_none(), "编译失败：{:?}", outcome.error);
            assert!(outcome.page_count <= MAX_PREVIEW_PAGES);
            return outcome;
        }
        assert!(start.elapsed() < COMPILE_TIMEOUT, "编译超时");
        std::thread::sleep(Duration::from_millis(2));
    }
}

/// 光栅基线：在独立线程上整页光栅化，与 PreviewCompiler::request_page 的
/// worker 路径一致（typst_render + pixel_per_pt = PIXELS_PER_PT）。
fn raster_once(doc: &scholium_model::DocumentSnapshot) -> f64 {
    let doc = doc.clone();
    let handle = std::thread::spawn(move || raster_document(&doc));
    handle.join().expect("raster thread")
}

fn raster_baseline(doc: &scholium_model::DocumentSnapshot) {
    let mut samples = Vec::new();
    for _ in 0..3 {
        samples.push(raster_once(doc));
    }
    report("冷态后光栅基线", &samples);
}

/// 整页光栅化耗时（毫秒）：在调用线程上编译并光栅化第 0 页。
/// 编译段已由 PreviewCompiler 自报，此处单独计时光栅调用本身。
fn raster_document(doc: &scholium_model::DocumentSnapshot) -> f64 {
    let source = scholium_typst::generate_typst(doc);
    let world = preview_world(&source);
    let document = typst::compile::<typst_layout::PagedDocument>(&world)
        .output
        .expect("spike 夹具必须编译成功");
    let start = Instant::now();
    let pixmap = typst_render::render(
        &document.pages()[0],
        &typst_render::RenderOptions {
            pixel_per_pt: f64::from(PIXELS_PER_PT).into(),
            render_bleed: false,
        },
    );
    let elapsed = start.elapsed().as_secs_f64() * 1000.0;
    println!(
        "    页面位图 {}×{} px（{:.0} 万像素）",
        pixmap.width(),
        pixmap.height(),
        (pixmap.width() as f64 * pixmap.height() as f64) / 10000.0
    );
    elapsed
}

/// 与 `PreviewWorld` 同构的最小 World：嵌入式字体 + 系统字体 + 单可换源。
struct SpikeWorld {
    library: typst::utils::LazyHash<typst::Library>,
    fonts: typst_kit::fonts::FontStore,
    main: typst::syntax::FileId,
    source: typst::syntax::Source,
}

fn preview_world(source: &str) -> SpikeWorld {
    let mut fonts = typst_kit::fonts::FontStore::new();
    fonts.extend(typst_kit::fonts::embedded());
    fonts.extend(typst_kit::fonts::system());
    let path = typst::syntax::RootedPath::new(
        typst::syntax::VirtualRoot::Project,
        typst::syntax::VirtualPath::new("main.typ").expect("fixed path is always valid"),
    );
    let main = path.intern();
    SpikeWorld {
        library: typst::utils::LazyHash::new(typst::Library::default()),
        fonts,
        main,
        source: typst::syntax::Source::new(main, source.to_owned()),
    }
}

impl typst::World for SpikeWorld {
    fn library(&self) -> &typst::utils::LazyHash<typst::Library> {
        &self.library
    }
    fn book(&self) -> &typst::utils::LazyHash<typst::text::FontBook> {
        self.fonts.book()
    }
    fn main(&self) -> typst::syntax::FileId {
        self.main
    }
    fn source(&self, id: typst::syntax::FileId) -> typst::diag::FileResult<typst::syntax::Source> {
        if id == self.main {
            Ok(self.source.clone())
        } else {
            Err(typst::diag::FileError::NotFound(
                id.vpath().get_without_slash().into(),
            ))
        }
    }
    fn file(
        &self,
        _id: typst::syntax::FileId,
    ) -> typst::diag::FileResult<typst::foundations::Bytes> {
        Err(typst::diag::FileError::NotFound("spike".into()))
    }
    fn font(&self, index: usize) -> Option<typst::text::Font> {
        self.fonts.font(index)
    }
    fn today(
        &self,
        _offset: Option<typst::foundations::Duration>,
    ) -> Option<typst::foundations::Datetime> {
        None
    }
}

/// 单行 markup → inline 序列（与 scholium_document::parse_markup 同构的最小子集，
/// spike 只需要构造快照，不引入对该 crate 的依赖）。
fn parse_line(line: &str) -> Vec<Inline> {
    let mut content = Vec::new();
    let mut text = String::new();
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            '$' => {
                if let Some(off) = chars[i + 1..].iter().position(|&c| c == '$') {
                    if !text.is_empty() {
                        content.push(Inline::Text(std::mem::take(&mut text)));
                    }
                    content.push(Inline::Math(chars[i + 1..i + 1 + off].iter().collect()));
                    i += off + 2;
                } else {
                    text.push('$');
                    i += 1;
                }
            }
            ch => {
                text.push(ch);
                i += 1;
            }
        }
    }
    if !text.is_empty() {
        content.push(Inline::Text(text));
    }
    content
}
