//! The editing surface and the A4 preview must break lines identically.

use super::content::ContentCache;
use super::kernel::{EditWorld, layout};
use crate::{PreviewWorld, SourceMode, generate_source};
use scholium_model::{
    BlockKind, DocumentId, NodeId, Revision,
    structured::{
        InlineBody, MathBody, MathNode, StructuredBlock, StructuredDocument, StructuredInline,
        TextStyle,
    },
};
use typst::layout::{Frame, FrameItem, Point, Transform};
use typst_layout::PagedDocument;

const HEADING_ONE: &str = "第一章 排版与输出";
const HEADING_TWO: &str = "1.1 Line breaking 与断行";
const PARAGRAPH_ONE: &str = "所见即所得的排版编辑器必须让编辑面与输出完全一致：字体、字号、行距、缩进与断行都不能因为预览而改变。This sentence mixes English words with 中文字符 so that the line breaker has to choose between Latin and CJK break opportunities, 并且足够长以便换行三次以上。";
const PARAGRAPH_TWO: &str = "第二段同样混排。Typesetting parity means the same glyphs land on the same lines in both the quick preview and the continuous editing surface, 无论段落多长，每一行的首个字符都应当相同。Another English clause keeps going to force a wrap.";
const FORMULA: &str = "ab";

fn plain(text: &str) -> StructuredInline {
    StructuredInline {
        node: NodeId::fresh(),
        body: InlineBody::Text {
            text: text.into(),
            style: TextStyle::Plain,
        },
    }
}

fn formula() -> StructuredInline {
    StructuredInline {
        node: NodeId::fresh(),
        body: InlineBody::Math {
            root: MathNode {
                node: NodeId::fresh(),
                body: MathBody::Text {
                    text: FORMULA.into(),
                },
            },
        },
    }
}

fn fixture() -> StructuredDocument {
    let blocks = [
        (BlockKind::Heading1, vec![plain(HEADING_ONE)]),
        (BlockKind::Heading2, vec![plain(HEADING_TWO)]),
        (BlockKind::Paragraph, vec![plain(PARAGRAPH_ONE)]),
        (
            BlockKind::Paragraph,
            vec![plain(PARAGRAPH_TWO), formula(), plain("。")],
        ),
    ];
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

/// Text items grouped by baseline, left to right, in reading order.
fn lines(frame: &Frame, max_y: f64) -> Vec<(f64, String)> {
    let mut items = Vec::new();
    collect(frame, Transform::identity(), &mut items);
    items.retain(|(y, _, _)| *y < max_y);
    items.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
    let mut out: Vec<(f64, String)> = Vec::new();
    for (y, _, text) in items {
        match out.last_mut() {
            Some((last, line)) if (y - *last).abs() < 0.5 => line.push_str(&text),
            _ => out.push((y, text)),
        }
    }
    let top = out.first().map_or(0.0, |(y, _)| *y);
    out.into_iter()
        .map(|(y, line)| (y - top, line.trim().to_owned()))
        .collect()
}

fn collect(frame: &Frame, transform: Transform, out: &mut Vec<(f64, f64, String)>) {
    for (position, item) in frame.items() {
        match item {
            FrameItem::Group(group) => {
                let inner = transform
                    .pre_concat(Transform::translate(position.x, position.y))
                    .pre_concat(group.transform);
                collect(&group.frame, inner, out);
            }
            FrameItem::Text(text) => {
                let at = Point::zero()
                    .transform(transform.pre_concat(Transform::translate(position.x, position.y)));
                out.push((at.y.to_pt(), at.x.to_pt(), text.text.to_string()));
            }
            _ => {}
        }
    }
}

fn editor_lines(doc: &StructuredDocument) -> Vec<(f64, String)> {
    let content = ContentCache::default().document(doc).expect("content");
    let frame = layout(&EditWorld::new(), &content).expect("editor layout");
    lines(&frame, f64::MAX)
}

fn preview_lines(doc: &StructuredDocument) -> Vec<(f64, String)> {
    let world = PreviewWorld::new();
    world.set_source(generate_source(doc, SourceMode::Preview).source);
    let compiled = typst::compile::<PagedDocument>(&world);
    let document = compiled.output.expect("preview compile");
    assert_eq!(document.pages().len(), 1, "fixture fits one page");
    let frame = &document.pages()[0].frame;
    // Drop the page-number footer that lives in the bottom margin.
    lines(frame, frame.height().to_pt() - crate::margin_y_pt())
}

fn block_texts(doc: &StructuredDocument) -> Vec<String> {
    doc.blocks
        .iter()
        .map(|block| {
            block
                .content
                .iter()
                .map(|inline| match &inline.body {
                    InlineBody::Text { text, .. } => text.clone(),
                    _ => FORMULA.to_owned(),
                })
                .collect::<String>()
                .split_whitespace()
                .collect()
        })
        .collect()
}

/// Number of lines each block occupies, by consuming each block's characters.
fn lines_per_block(lines: &[String], blocks: &[String]) -> Vec<usize> {
    let mut remaining = lines.iter();
    blocks
        .iter()
        .map(|block| {
            let mut left = block.chars().count();
            let mut count = 0;
            while left > 0 {
                let line = remaining.next().expect("line for block text");
                left = left
                    .saturating_sub(line.split_whitespace().collect::<String>().chars().count());
                count += 1;
            }
            count
        })
        .collect()
}

#[test]
fn editor_and_preview_break_every_block_into_the_same_lines() {
    let doc = fixture();
    let (editor_y, editor) = editor_lines(&doc)
        .into_iter()
        .unzip::<_, _, Vec<_>, Vec<_>>();
    let (preview_y, preview) = preview_lines(&doc)
        .into_iter()
        .unzip::<_, _, Vec<_>, Vec<_>>();
    let blocks = block_texts(&doc);

    let per_block = lines_per_block(&editor, &blocks);
    assert_eq!(
        per_block,
        lines_per_block(&preview, &blocks),
        "lines per block"
    );
    assert!(
        per_block[2] >= 3 && per_block[3] >= 3,
        "paragraphs wrap three or more lines: {per_block:?}"
    );
    let first = |lines: &[String]| -> Vec<Option<char>> {
        lines.iter().map(|line| line.chars().next()).collect()
    };
    assert_eq!(first(&editor), first(&preview), "first character per line");
    assert_eq!(editor, preview, "full line text");
    // Heading size, spacing and leading show up as baseline offsets.
    for (index, (a, b)) in editor_y.iter().zip(&preview_y).enumerate() {
        assert!((a - b).abs() < 0.5, "baseline of line {index}: {a} vs {b}");
    }
}

fn text_node(text: &str) -> MathNode {
    MathNode {
        node: NodeId::fresh(),
        body: MathBody::Text { text: text.into() },
    }
}

fn math_doc(root: MathNode) -> StructuredDocument {
    let formula = StructuredInline {
        node: NodeId::fresh(),
        body: InlineBody::Math { root },
    };
    StructuredDocument {
        document: DocumentId::fresh(),
        revision: Revision(1),
        blocks: vec![StructuredBlock {
            node: NodeId::fresh(),
            kind: BlockKind::Paragraph,
            content: vec![plain("Start "), formula, plain(" end.")],
        }],
    }
}

fn letters(count: usize, separator: &str) -> String {
    (0..count)
        .map(|i| char::from(b'a' + (i % 26) as u8).to_string())
        .collect::<Vec<_>>()
        .join(separator)
}

fn attach_sup(base: &str, sup: &str) -> MathNode {
    MathNode {
        node: NodeId::fresh(),
        body: MathBody::Attach {
            base: Box::new(text_node(base)),
            sub: None,
            sup: Some(Box::new(text_node(sup))),
        },
    }
}

/// Line texts without whitespace: a typed space in a formula has no width in either surface.
fn compact(lines: Vec<(f64, String)>) -> Vec<String> {
    lines
        .into_iter()
        .map(|(_, line)| line.split_whitespace().collect())
        .collect()
}

#[test]
fn long_operator_formulas_break_after_the_same_operators() {
    for (name, root) in [
        ("sum", letters(40, "+")),
        ("sum with typed spaces", letters(40, " + ")),
        ("equation chain", letters(30, "=")),
    ] {
        let doc = math_doc(text_node(&root));
        let editor = compact(editor_lines(&doc));
        assert!(editor.len() >= 2, "{name} wraps: {editor:?}");
        assert!(
            editor[0].ends_with(['+', '=']),
            "{name} breaks after an operator: {editor:?}"
        );
        assert_eq!(editor, compact(preview_lines(&doc)), "{name}");
    }
}

#[test]
fn operator_free_runs_stay_whole_in_both_surfaces() {
    let run = letters(104, "");
    let shown: String = run
        .chars()
        .map(|c| super::content::display(&c.to_string()))
        .collect();
    for (name, root) in [
        ("run", text_node(&run)),
        ("superscript run", attach_sup("x", &run)),
    ] {
        let doc = math_doc(root);
        for (surface, lines) in [
            ("editor", compact(editor_lines(&doc))),
            ("preview", compact(preview_lines(&doc))),
        ] {
            assert!(
                lines.iter().any(|line| line.contains(&shown)),
                "{name} is not broken in the {surface}: {lines:?}"
            );
        }
    }
}
