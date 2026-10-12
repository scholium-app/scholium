use super::*;
use scholium_model::{
    BlockKind, DocumentId, NodeId, Revision,
    structured::{
        InlineBody, MathBody, MathNode, StructuredBlock, StructuredDocument, StructuredInline,
        TextStyle,
    },
};

fn inline(body: InlineBody) -> StructuredInline {
    StructuredInline {
        node: NodeId::fresh(),
        body,
    }
}

fn text(value: &str, style: TextStyle) -> StructuredInline {
    inline(InlineBody::Text {
        text: value.into(),
        style,
    })
}

fn plain(value: &str) -> StructuredInline {
    text(value, TextStyle::Plain)
}

fn leaf(body: MathBody) -> MathNode {
    MathNode {
        node: NodeId::fresh(),
        body,
    }
}

fn math(root: MathNode) -> StructuredInline {
    inline(InlineBody::Math { root })
}

fn doc(blocks: Vec<(BlockKind, Vec<StructuredInline>)>) -> StructuredDocument {
    StructuredDocument {
        document: DocumentId::fresh(),
        revision: Revision(1),
        blocks: blocks
            .into_iter()
            .map(|(kind, content)| StructuredBlock {
                node: NodeId::fresh(),
                kind,
                content,
            })
            .collect(),
    }
}

fn preview(doc: &StructuredDocument) -> String {
    generate_source(doc, SourceMode::Preview).source
}

#[test]
fn view_source_maps_blocks_styles_and_escapes_markup() {
    let d = doc(vec![
        (BlockKind::Heading1, vec![plain("谱与振动")]),
        (BlockKind::Heading2, vec![plain("1 = 模态")]),
        (
            BlockKind::Paragraph,
            vec![
                plain("a*b #x $y "),
                text("粗", TextStyle::Strong),
                text("强调_", TextStyle::Emphasis),
            ],
        ),
    ]);
    let generated = generate_source(&d, SourceMode::View);
    assert_eq!(
        generated.source,
        "= 谱与振动\n\n== 1 \\= 模态\n\na\\*b \\#x \\$y #strong[粗]#emph[强调\\_]\n"
    );
    assert_eq!(generated.blocks.len(), 3);
    for (block, generated_block) in d.blocks.iter().zip(&generated.blocks) {
        assert_eq!(block.node, generated_block.node);
    }
    assert_eq!(
        &generated.source[generated.blocks[0].range.clone()],
        "= 谱与振动"
    );
    assert!(
        !generated.source.contains("metadata"),
        "the view has no anchors"
    );
}

#[test]
fn math_text_is_literal_fractions_group_and_holes_are_counted() {
    let fraction = leaf(MathBody::Fraction {
        numerator: Box::new(leaf(MathBody::Text {
            text: r#"ab, c""#.into(),
        })),
        denominator: Box::new(MathNode::hole()),
    });
    let row = leaf(MathBody::Row {
        children: vec![
            leaf(MathBody::Text { text: "x".into() }),
            leaf(MathBody::Symbol { symbol: '+' }),
            leaf(MathBody::Text { text: "12".into() }),
            fraction,
        ],
    });
    let generated = generate_source(
        &doc(vec![(BlockKind::Paragraph, vec![math(row)])]),
        SourceMode::View,
    );
    assert_eq!(generated.source, "$x + 12 (a b , c \"\\\"\")/(□)$\n");
    assert_eq!(generated.holes, 1);
}

#[test]
fn raw_formulas_are_shown_verbatim_but_never_compiled() {
    let raw = inline(InlineBody::RawMath {
        source: r#"#sys.inputs.at("x")"#.into(),
    });
    let id = raw.node;
    let d = doc(vec![(BlockKind::Paragraph, vec![plain("前"), raw])]);
    let view = generate_source(&d, SourceMode::View);
    assert!(view.source.contains(r#"$#sys.inputs.at("x")$"#));
    let compiled = generate_source(&d, SourceMode::Preview);
    assert_eq!(compiled.raw, vec![id]);
    assert!(
        !compiled.source.contains("sys.inputs"),
        "raw source never reaches Typst"
    );
    assert!(compiled.source.contains("原串公式未执行"));
}

#[test]
fn compiler_produces_pages_for_a_structured_document() {
    let d = doc(vec![
        (BlockKind::Heading1, vec![plain("谱与振动")]),
        (
            BlockKind::Paragraph,
            vec![plain("设弦的长度为 L，张力为 T。")],
        ),
        (
            BlockKind::Paragraph,
            vec![
                plain("比值 "),
                math(leaf(MathBody::Fraction {
                    numerator: Box::new(leaf(MathBody::Text { text: "T".into() })),
                    denominator: Box::new(MathNode::hole()),
                })),
            ],
        ),
    ]);
    let mut compiler = PreviewCompiler::spawn();
    compiler.submit(d.document, 1, preview(&d), d.blocks.len());
    let outcome = wait_for_compile(&mut compiler, d.document, 1);
    let error = outcome.error.unwrap_or_default();
    assert!(error.is_empty(), "compile should succeed: {error}");
    assert!(outcome.page_count > 0);
    assert_eq!(outcome.anchors.len(), 3);
    compiler.request_page(d.document, 1, 0, 2.0);
    let page = wait_for_page(&mut compiler, d.document, 1, 0).pixels;
    assert!(page.width > 0 && page.height > page.width, "A4 portrait");
    assert_eq!(
        page.rgba.len(),
        page.width as usize * page.height as usize * 4
    );
}

#[test]
fn latest_generation_wins_when_requests_overtake_each_other() {
    let d = doc(vec![(BlockKind::Paragraph, vec![plain("短文档")])]);
    let mut compiler = PreviewCompiler::spawn();
    compiler.submit(d.document, 7, preview(&d), d.blocks.len());
    compiler.submit(d.document, 9, preview(&d), d.blocks.len());
    let outcome = wait_for_compile(&mut compiler, d.document, 9);
    assert_eq!(
        outcome.revision, 9,
        "queued request is replaced, not queued up"
    );
}

#[test]
fn anchors_locate_every_block_and_map_clicks() {
    let d = doc(vec![
        (BlockKind::Heading1, vec![plain("谱与振动")]),
        (BlockKind::Paragraph, vec![plain("第一段正文。")]),
        (BlockKind::Paragraph, vec![plain("第二段正文。")]),
    ]);
    let mut compiler = PreviewCompiler::spawn();
    compiler.submit(d.document, 1, preview(&d), d.blocks.len());
    let outcome = wait_for_compile(&mut compiler, d.document, 1);
    assert!(outcome.error.is_none(), "{:?}", outcome.error);
    assert_eq!(outcome.anchors.len(), 3, "one anchor per block");
    assert!(
        outcome
            .anchors
            .iter()
            .all(|a| a.page == 1 && a.start_page == 1)
    );
    assert!(outcome.anchors[0].start_y < outcome.anchors[0].y);
    assert!(outcome.anchors[0].y < outcome.anchors[1].y);
    assert!(outcome.anchors[1].y < outcome.anchors[2].y);
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
fn preview_worker_wakes_the_host_for_compile_and_page_delivery() {
    let (wake, events) = std::sync::mpsc::channel();
    let mut compiler = PreviewCompiler::spawn_with_wake(move || {
        let _ = wake.send(());
    });
    let mut d = doc(vec![(BlockKind::Paragraph, vec![plain("正文")])]);
    for generation in 1..=3u64 {
        d.blocks[0].content = vec![plain(&"正文".repeat(generation as usize))];
        compiler.submit(d.document, generation, preview(&d), d.blocks.len());
        events
            .recv_timeout(std::time::Duration::from_secs(15))
            .expect("compile notification");
        let Some(PreviewEvent::Compiled(result)) = compiler.poll() else {
            panic!("metadata is sent before the notification");
        };
        assert!(result.error.is_none(), "{:?}", result.error);
        compiler.request_page(d.document, generation, 0, 2.0);
        events
            .recv_timeout(std::time::Duration::from_secs(15))
            .expect("page notification");
        assert!(matches!(compiler.poll(), Some(PreviewEvent::Page(_))));
    }
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
fn clicks_in_the_last_line_of_a_multiline_block_stay_in_that_block() {
    let d = doc(vec![
        (
            BlockKind::Paragraph,
            vec![plain(&"第一段很长的正文。".repeat(20))],
        ),
        (BlockKind::Paragraph, vec![plain("第二段正文。")]),
        (BlockKind::Paragraph, vec![plain("第三段正文。")]),
    ]);
    let mut compiler = PreviewCompiler::spawn();
    compiler.submit(d.document, 1, preview(&d), d.blocks.len());
    let outcome = wait_for_compile(&mut compiler, d.document, 1);
    let a = &outcome.anchors;
    // A non-final block's end tag sits at its bottom edge, which is the next block's start.
    assert_eq!(a[0].y, a[1].start_y);
    assert_eq!(
        block_at_click(a, 1, a[0].y - 2.0),
        Some(0),
        "last line of block 0"
    );
    assert_eq!(block_at_click(a, 1, a[1].start_y + 2.0), Some(1));
    assert_eq!(
        block_at_click(a, 1, a[2].y + 10.0),
        Some(2),
        "below the last block"
    );
}

#[test]
fn in_word_styles_parentheses_list_markers_and_unicode_newlines_stay_literal() {
    let d = doc(vec![
        (BlockKind::Heading1, vec![plain("1. 标题\u{2028}续")]),
        (
            BlockKind::Paragraph,
            vec![
                plain("foo"),
                text("bar", TextStyle::Strong),
                text("baz", TextStyle::Emphasis),
                plain("(x)"),
            ],
        ),
    ]);
    let view = generate_source(&d, SourceMode::View).source;
    assert_eq!(
        view,
        "= 1\\. 标题\\u{2028}续\n\nfoo#strong[bar]#emph[baz]\\(x\\)\n"
    );
    let mut compiler = PreviewCompiler::spawn();
    compiler.submit(d.document, 1, preview(&d), d.blocks.len());
    let outcome = wait_for_compile(&mut compiler, d.document, 1);
    assert!(outcome.error.is_none(), "{:?}", outcome.error);
    assert_eq!(
        outcome.anchors.len(),
        2,
        "no extra structure from user text"
    );
}

fn text_node(value: &str) -> MathNode {
    leaf(MathBody::Text { text: value.into() })
}

fn attach(base: MathNode, sub: Option<MathNode>, sup: Option<MathNode>) -> MathNode {
    leaf(MathBody::Attach {
        base: Box::new(base),
        sub: sub.map(Box::new),
        sup: sup.map(Box::new),
    })
}

fn view_of(root: MathNode) -> String {
    generate_source(
        &doc(vec![(BlockKind::Paragraph, vec![math(root)])]),
        SourceMode::View,
    )
    .source
}

/// Text of every shaped run in the first page, as Typst laid it out.
fn rendered_text(source: &str) -> String {
    use typst::layout::{Frame, FrameItem};
    fn walk(frame: &Frame, out: &mut String) {
        for (_, item) in frame.items() {
            match item {
                FrameItem::Text(run) => out.push_str(&run.text),
                FrameItem::Group(group) => walk(&group.frame, out),
                _ => {}
            }
        }
    }
    let world = PreviewWorld::new();
    world.set_source(source.into());
    let compiled = typst::compile::<PagedDocument>(&world);
    let document = compiled.output.expect("generated source compiles");
    let mut out = String::new();
    walk(&document.pages()[0].frame, &mut out);
    out
}

#[test]
fn attach_source_parenthesizes_slots_and_multi_atom_bases() {
    let both = attach(text_node("x"), Some(text_node("i")), Some(text_node("2")));
    assert_eq!(view_of(both), "$x^(2)_(i)$\n");
    let only_sub = attach(text_node("x"), Some(MathNode::hole()), None);
    assert_eq!(view_of(only_sub), "$x_(□)$\n");
    let grouped = attach(text_node("ab"), None, Some(text_node("2")));
    assert_eq!(view_of(grouped), "$(a b)^(2)$\n");
    let nested = attach(
        attach(text_node("x"), None, Some(text_node("2"))),
        None,
        Some(text_node("3")),
    );
    assert_eq!(view_of(nested), "$(x^(2))^(3)$\n");
    let row = leaf(MathBody::Row {
        children: vec![
            attach(text_node("x"), None, Some(text_node("2"))),
            text_node("+y"),
        ],
    });
    assert_eq!(view_of(row), "$x^(2) + y$\n");
}

#[test]
fn math_text_is_emitted_per_atom() {
    assert_eq!(view_of(text_node("x+y = 3.14 <")), "$x + y = 3.14 <$\n");
    assert_eq!(view_of(text_node("12ab")), "$12 a b$\n");
    assert_eq!(view_of(text_node("中é")), "$\"中\" \"é\"$\n");
    assert_eq!(view_of(text_node("e\u{301}")), "$\"e\u{301}\"$\n");
}

#[test]
fn attach_preview_compiles_with_real_scripts() {
    let both = attach(text_node("x"), Some(text_node("i")), Some(text_node("2")));
    let d = doc(vec![(BlockKind::Paragraph, vec![math(both)])]);
    let text = rendered_text(&preview(&d));
    assert!(text.contains('2') && !text.contains('^'), "{text:?}");
}

#[test]
fn syntax_characters_in_math_text_stay_literal_in_the_compiled_output() {
    const HOSTILE: &str = "#$\\\"/^_&@*'`[]()~=";
    let chars: Vec<char> = HOSTILE.chars().collect();
    for plain_syntax in ['#', '$', '\\', '"', '/', '^', '_', '&', '@'] {
        let d = doc(vec![(
            BlockKind::Paragraph,
            vec![math(attach(
                text_node(&format!("a{plain_syntax}b")),
                Some(text_node(&plain_syntax.to_string())),
                Some(text_node(&HOSTILE.replace(' ', ""))),
            ))],
        )]);
        let view = generate_source(&d, SourceMode::View).source;
        let outside = outside_strings(&view);
        for forbidden in ['#', '\\', '"', '/', '&', '@'] {
            assert!(!outside.contains(forbidden), "{forbidden:?} leaked: {view}");
        }
        assert_eq!(outside.matches('^').count(), 1, "{view}");
        assert_eq!(outside.matches('_').count(), 1, "{view}");
        let rendered = rendered_text(&preview(&d));
        for c in chars.iter().chain(&[plain_syntax]) {
            assert!(rendered.contains(*c), "{c:?} missing from {rendered:?}");
        }
    }
}

/// Generated source with quoted string contents removed, minus the formula delimiters.
fn outside_strings(source: &str) -> String {
    let mut out = String::new();
    let mut quoted = false;
    let mut escaped = false;
    for c in source.trim().trim_matches('$').chars() {
        match (quoted, escaped, c) {
            (true, true, _) => escaped = false,
            (true, false, '\\') => escaped = true,
            (true, false, '"') => quoted = false,
            (true, false, _) => {}
            (false, _, '"') => quoted = true,
            (false, _, _) => out.push(c),
        }
    }
    out
}
