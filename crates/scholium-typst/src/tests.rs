use super::*;
use scholium_model::{Block, BlockKind, DocumentId, Inline, NodeId, Revision};

fn snapshot(blocks: &[(BlockKind, &str)]) -> DocumentSnapshot {
    DocumentSnapshot {
        document: DocumentId::fresh(),
        revision: Revision(1),
        blocks: blocks
            .iter()
            .map(|(kind, text)| Block {
                node: NodeId::fresh(),
                kind: *kind,
                content: vec![Inline::Text((*text).to_owned())],
            })
            .collect(),
    }
}

#[test]
fn generation_maps_kinds_and_escapes_punctuation() {
    let snap = snapshot(&[
        (BlockKind::Heading1, "谱与振动"),
        (BlockKind::Paragraph, "设 $x > 0$ 与 #tag*强调*不生效"),
        (BlockKind::Heading2, "能量法"),
    ]);
    let source = generate_typst(&snap);
    assert!(source.contains("= 谱与振动"), "heading1 uses level 1");
    assert!(source.contains("== 能量法"), "heading2 uses level 2");
    assert!(
        source.contains(r"设 \$x \> 0\$ 与 \#tag\*强调\*不生效"),
        "user punctuation stays literal: {source}"
    );
    assert!(source.contains(r#""Times New Roman", "SimSun""#));
    assert!(source.contains(r#""Times New Roman", "SimHei""#));
}

#[test]
fn math_segments_render_as_formulas_not_literal_text() {
    let snap = DocumentSnapshot {
        document: DocumentId::fresh(),
        revision: Revision(1),
        blocks: vec![Block {
            node: NodeId::fresh(),
            kind: scholium_model::BlockKind::Paragraph,
            content: vec![
                Inline::Text("频率比 ".into()),
                Inline::Math("alpha/2 + sqrt(T/rho)".into()),
                Inline::Text(" 决定模态。".into()),
            ],
        }],
    };
    let source = generate_typst(&snap);
    assert!(
        source.contains("频率比 $alpha/2 + sqrt(T/rho)$ 决定模态。"),
        "math stays unescaped between $ delimiters: {source}"
    );
    let mut compiler = PreviewCompiler::spawn();
    compiler.submit(snap.document, 1, source, snap.blocks.len());
    let outcome = wait_for_compile(&mut compiler, snap.document, 1);
    assert!(
        outcome.error.is_none(),
        "math must compile: {:?}",
        outcome.error
    );
    assert!(outcome.page_count > 0);
}

#[test]
fn compiler_produces_pages_for_a_document() {
    let snap = snapshot(&[
        (BlockKind::Heading1, "谱与振动"),
        (BlockKind::Paragraph, "设弦的长度为 L，张力为 T。"),
        (BlockKind::Paragraph, "固定端点给出边界条件。"),
    ]);
    let mut compiler = PreviewCompiler::spawn();
    compiler.submit(
        snap.document,
        1,
        generate_typst_anchored(&snap),
        snap.blocks.len(),
    );
    let outcome = wait_for_compile(&mut compiler, snap.document, 1);
    assert_eq!(outcome.revision, 1);
    let error = outcome.error.unwrap_or_default();
    assert!(error.is_empty(), "compile should succeed: {error}");
    assert!(outcome.page_count > 0, "at least one compiled page");
    compiler.request_page(snap.document, 1, 0, 2.0);
    let page = wait_for_page(&mut compiler, snap.document, 1, 0).pixels;
    assert!(page.width > 0 && page.height > page.width, "A4 portrait");
    assert_eq!(
        page.rgba.len(),
        page.width as usize * page.height as usize * 4
    );
}

#[test]
fn latest_revision_wins_when_requests_overtake_each_other() {
    let snap = snapshot(&[(BlockKind::Paragraph, "短文档")]);
    let mut compiler = PreviewCompiler::spawn();
    compiler.submit(
        snap.document,
        7,
        generate_typst_anchored(&snap),
        snap.blocks.len(),
    );
    compiler.submit(
        snap.document,
        9,
        generate_typst_anchored(&snap),
        snap.blocks.len(),
    );
    let outcome = wait_for_compile(&mut compiler, snap.document, 9);
    assert_eq!(
        outcome.revision, 9,
        "queued request is replaced, not queued up"
    );
}

// Compile includes a system font scan on first use; allow seconds, not ms.
fn wait_for_compile(
    compiler: &mut PreviewCompiler,
    document: DocumentId,
    revision: u64,
) -> CompileOutcome {
    for _ in 0..600 {
        if let Some(PreviewEvent::Compiled(outcome)) = compiler.poll() {
            if outcome.document != document {
                continue;
            }
            // A compile already in flight may finish before the latest request.
            if outcome.revision == revision {
                return outcome;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    panic!("no compile outcome within 15s (font scan + compile)");
}

fn wait_for_page(
    compiler: &mut PreviewCompiler,
    document: DocumentId,
    revision: u64,
    page: usize,
) -> PageOutcome {
    for _ in 0..600 {
        if let Some(PreviewEvent::Page(outcome)) = compiler.poll()
            && (outcome.document, outcome.revision, outcome.page) == (document, revision, page)
        {
            return outcome;
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    panic!("no page outcome within 15s");
}

#[test]
fn anchors_locate_every_block_and_map_clicks() {
    let snap = snapshot(&[
        (BlockKind::Heading1, "谱与振动"),
        (BlockKind::Paragraph, "第一段正文。"),
        (BlockKind::Paragraph, "第二段正文。"),
    ]);
    let mut compiler = PreviewCompiler::spawn();
    compiler.submit(
        snap.document,
        1,
        generate_typst_anchored(&snap),
        snap.blocks.len(),
    );
    let outcome = wait_for_compile(&mut compiler, snap.document, 1);
    assert!(outcome.error.is_none(), "{:?}", outcome.error);
    assert_eq!(outcome.anchors.len(), 3, "one anchor per block");
    assert!(outcome.anchors.iter().all(|a| a.page == 1));
    assert!(outcome.anchors.iter().all(|a| a.start_page == 1));
    assert!(outcome.anchors[0].start_y < outcome.anchors[0].y);
    assert!(outcome.anchors[0].y < outcome.anchors[1].y);
    assert!(outcome.anchors[1].y < outcome.anchors[2].y);
    // An anchor follows its block: the next marker below the pointer owns it.
    let mid = (outcome.anchors[1].y + outcome.anchors[2].y) / 2.0;
    assert_eq!(block_at_click(&outcome.anchors, 1, mid), Some(2));
    assert_eq!(block_at_click(&outcome.anchors, 1, 0.0), Some(0));
    assert_eq!(block_at_click(&outcome.anchors, 1, f32::MAX), Some(2));
    assert_eq!(
        block_at_click(&outcome.anchors, 9, 10.0),
        None,
        "unknown page"
    );
}

#[test]
fn trailing_anchors_map_page_spanning_text_to_its_block() {
    let anchors = [
        BlockAnchor {
            block: 0,
            start_page: 1,
            start_x: 0.0,
            start_y: 50.0,
            page: 2,
            x: 0.0,
            y: 120.0,
        },
        BlockAnchor {
            block: 1,
            start_page: 2,
            start_x: 0.0,
            start_y: 150.0,
            page: 3,
            x: 0.0,
            y: 100.0,
        },
    ];
    assert_eq!(block_at_click(&anchors, 1, 700.0), Some(0));
    assert_eq!(block_at_click(&anchors, 2, 80.0), Some(0));
    assert_eq!(block_at_click(&anchors, 2, 200.0), Some(1));
    assert_eq!(block_at_click(&anchors, 3, 50.0), Some(1));
}

#[test]
fn page_after_the_old_eight_page_limit_can_be_rendered_on_demand() {
    let document = DocumentId::fresh();
    let source = (0..10)
        .map(|page| {
            if page == 9 {
                format!("Page {page}\n")
            } else {
                format!("Page {page}\n#pagebreak()\n")
            }
        })
        .collect::<String>();
    let mut compiler = PreviewCompiler::spawn();
    compiler.submit(document, 1, source, 0);
    let outcome = wait_for_compile(&mut compiler, document, 1);
    assert!(outcome.error.is_none(), "{:?}", outcome.error);
    assert_eq!(outcome.page_count, 10);
    compiler.request_page(document, 1, 9, 2.0);
    let last = wait_for_page(&mut compiler, document, 1, 9);
    assert!(last.pixels.width > 0 && last.pixels.height > 0);
}

#[test]
fn exceeding_the_page_limit_reports_failure_instead_of_truncating() {
    let document = DocumentId::fresh();
    let source = "Page\n#pagebreak()\n".repeat(MAX_PREVIEW_PAGES) + "Last page";
    let mut compiler = PreviewCompiler::spawn();
    compiler.submit(document, 1, source, 0);
    let outcome = wait_for_compile(&mut compiler, document, 1);
    assert_eq!(outcome.page_count, MAX_PREVIEW_PAGES + 1);
    assert!(
        outcome
            .error
            .as_deref()
            .is_some_and(|error| error.contains("100"))
    );
    assert!(outcome.anchors.is_empty());
}

#[test]
fn new_document_and_plain_typing_compile_with_editable_geometry() {
    let mut compiler = PreviewCompiler::spawn();
    for content in [
        vec![],
        vec![Inline::Text("a".into())],
        vec![Inline::Text("abc".into())],
        vec![Inline::Text("你好".into())],
    ] {
        let mut snap = snapshot(&[(BlockKind::Paragraph, "")]);
        snap.blocks[0].content = content;
        compiler.submit_snapshot(&snap);
        let outcome = wait_for_compile(&mut compiler, snap.document, snap.revision.0);
        assert!(
            outcome.error.is_none(),
            "{:?}: {:?}\n{}",
            snap.blocks[0].content,
            outcome.error,
            generate_typst_anchored(&snap)
        );
        assert!(!outcome.geometry[0].cells.is_empty());
    }
}

#[test]
fn incomplete_formula_input_remains_visible_and_editable_until_completed() {
    let mut compiler = PreviewCompiler::spawn();
    let mut snap = snapshot(&[(BlockKind::Paragraph, "")]);
    for (revision, formula) in [
        "a", "al", "alp", "alph", "alpha", "alpha/", "alpha/2", "sqrt(", "sqrt(x)", "hello",
    ]
    .iter()
    .enumerate()
    {
        snap.revision = Revision(revision as u64);
        snap.blocks[0].content = vec![Inline::Math((*formula).into())];
        compiler.submit_snapshot(&snap);
        let start = std::time::Instant::now();
        let outcome = wait_for_compile(&mut compiler, snap.document, snap.revision.0);
        eprintln!(
            "formula={formula:?} compile={}ms result={:?} receive={}ms",
            outcome.elapsed_ms,
            outcome.error,
            start.elapsed().as_millis()
        );
        assert!(outcome.error.is_none(), "{formula}: {:?}", outcome.error);
        assert!(!outcome.geometry[0].cells.is_empty(), "{formula}");
        let complete = matches!(*formula, "a" | "alpha" | "alpha/2" | "sqrt(x)");
        assert_eq!(outcome.warning.is_none(), complete, "{formula}");
        compiler.request_page(snap.document, snap.revision.0, 0, 2.0);
        let raster_start = std::time::Instant::now();
        wait_for_page(&mut compiler, snap.document, snap.revision.0, 0);
        eprintln!("raster={}ms", raster_start.elapsed().as_millis());
    }
}

#[test]
fn formula_feedback_preserves_source_and_does_not_degrade_healthy_formulas() {
    let mut compiler = PreviewCompiler::spawn();
    let mut snap = snapshot(&[(BlockKind::Paragraph, "")]);
    snap.blocks[0].content = vec![
        Inline::Text("正文 ".into()),
        Inline::Math("al".into()),
        Inline::Text(" 后续 ".into()),
        Inline::Math("beta/2".into()),
        Inline::Math("sqrt(".into()),
    ];
    let source = generate_typst(&snap);
    compiler.submit_snapshot(&snap);
    let feedback = wait_for_compile(&mut compiler, snap.document, snap.revision.0);
    assert!(feedback.error.is_none(), "{:?}", feedback.error);
    let warning = feedback.warning.expect("draft diagnostics");
    assert!(warning.contains("unknown variable: al"), "{warning}");
    assert!(warning.contains("unclosed delimiter"), "{warning}");
    let markup = snap.blocks[0].markup_text();
    let cells = &feedback.geometry[0].cells;
    assert!(
        cells
            .iter()
            .any(|cell| markup.get(cell.input.clone()) == Some("beta")),
        "healthy Greek glyph retains its complete token"
    );
    assert!(
        cells
            .iter()
            .any(|cell| markup.get(cell.input.clone()) == Some("l")),
        "unfinished formula exposes each typed character for editing"
    );
    assert_eq!(
        generate_typst(&snap),
        source,
        "feedback never rewrites authority"
    );
    compiler.submit(snap.document, 2, source, 1);
    let strict = wait_for_compile(&mut compiler, snap.document, 2);
    assert!(
        strict.error.is_some(),
        "strict source compilation cannot silently repair formulas"
    );
    assert!(strict.warning.is_none());
}

#[test]
fn preview_worker_wakes_the_host_for_compile_and_page_delivery() {
    let (wake, events) = std::sync::mpsc::channel();
    let mut compiler = PreviewCompiler::spawn_with_wake(move || {
        let _ = wake.send(());
    });
    let mut snap = snapshot(&[(BlockKind::Paragraph, "正文")]);
    compiler.submit_snapshot(&snap);
    events
        .recv_timeout(std::time::Duration::from_secs(15))
        .expect("cold font setup + compile");
    let cold = compiler.poll().expect("metadata sent before notification");
    assert!(matches!(
        cold,
        PreviewEvent::Compiled(CompileOutcome { error: None, .. })
    ));
    for (revision, formula) in ["al", "alpha/", "alpha/2"].iter().enumerate() {
        snap.revision = Revision(revision as u64 + 2);
        snap.blocks[0].content = vec![Inline::Math((*formula).into())];
        let started = std::time::Instant::now();
        compiler.submit_snapshot(&snap);
        events
            .recv_timeout(std::time::Duration::from_secs(15))
            .expect("compile notification");
        let Some(PreviewEvent::Compiled(result)) = compiler.poll() else {
            panic!("compile event");
        };
        assert!(result.error.is_none());
        compiler.request_page(snap.document, snap.revision.0, 0, 2.0);
        events
            .recv_timeout(std::time::Duration::from_secs(15))
            .expect("page notification");
        assert!(matches!(compiler.poll(), Some(PreviewEvent::Page(_))));
        eprintln!(
            "warm formula {formula:?}: submit through raster delivery = {}ms",
            started.elapsed().as_millis()
        );
    }
}
