use super::*;
use scholium_model::{Block, DocumentId, Inline, NodeId, Revision};

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
    compiler.submit(1, source, snap.blocks.len());
    let outcome = wait_for_outcome(&mut compiler, 1);
    assert!(
        outcome.error.is_none(),
        "math must compile: {:?}",
        outcome.error
    );
    assert!(!outcome.pages.is_empty());
}

#[test]
fn compiler_produces_pages_for_a_document() {
    let snap = snapshot(&[
        (BlockKind::Heading1, "谱与振动"),
        (BlockKind::Paragraph, "设弦的长度为 L，张力为 T。"),
        (BlockKind::Paragraph, "固定端点给出边界条件。"),
    ]);
    let mut compiler = PreviewCompiler::spawn();
    compiler.submit(1, generate_typst_anchored(&snap), snap.blocks.len());
    let outcome = wait_for_outcome(&mut compiler, 1);
    assert_eq!(outcome.revision, 1);
    let error = outcome.error.unwrap_or_default();
    assert!(error.is_empty(), "compile should succeed: {error}");
    assert!(!outcome.pages.is_empty(), "at least one rasterized page");
    let page = &outcome.pages[0];
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
    compiler.submit(7, generate_typst_anchored(&snap), snap.blocks.len());
    compiler.submit(9, generate_typst_anchored(&snap), snap.blocks.len());
    let outcome = wait_for_outcome(&mut compiler, 9);
    assert_eq!(
        outcome.revision, 9,
        "queued request is replaced, not queued up"
    );
}

// Compile includes a system font scan on first use; allow seconds, not ms.
fn wait_for_outcome(compiler: &mut PreviewCompiler, revision: u64) -> Outcome {
    for _ in 0..600 {
        if let Some(outcome) = compiler.poll() {
            assert!(
                outcome.revision >= revision,
                "unexpected stale outcome r{}",
                outcome.revision
            );
            if outcome.revision == revision {
                return outcome;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    panic!("no compile outcome within 15s (font scan + compile)");
}

#[test]
fn anchors_locate_every_block_and_map_clicks() {
    let snap = snapshot(&[
        (BlockKind::Heading1, "谱与振动"),
        (BlockKind::Paragraph, "第一段正文。"),
        (BlockKind::Paragraph, "第二段正文。"),
    ]);
    let mut compiler = PreviewCompiler::spawn();
    compiler.submit(1, generate_typst_anchored(&snap), snap.blocks.len());
    let outcome = wait_for_outcome(&mut compiler, 1);
    assert!(outcome.error.is_none(), "{:?}", outcome.error);
    assert_eq!(outcome.anchors.len(), 3, "one anchor per block");
    assert!(outcome.anchors.iter().all(|a| a.page == 1));
    assert!(outcome.anchors[0].y < outcome.anchors[1].y);
    assert!(outcome.anchors[1].y < outcome.anchors[2].y);
    // Click mapping: between anchors 1 and 2 selects block 1; above all selects 0.
    let mid = (outcome.anchors[1].y + outcome.anchors[2].y) / 2.0;
    assert_eq!(block_at_click(&outcome.anchors, 1, mid), Some(1));
    assert_eq!(block_at_click(&outcome.anchors, 1, 0.0), Some(0));
    assert_eq!(
        block_at_click(&outcome.anchors, 9, 10.0),
        None,
        "unknown page"
    );
}
