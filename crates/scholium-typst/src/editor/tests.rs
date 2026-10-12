//! Editor scene geometry for math atoms and script slots, from the same Frame as the raster.
use super::content::ContentCache;
use super::{Affinity, EditGeometry, Position, kernel};
use scholium_model::{
    BlockKind, DocumentId, NodeId, Revision,
    structured::{InlineBody, MathBody, MathNode, StructuredBlock, StructuredDocument},
};
use typst::layout::{Frame, FrameItem};
use unicode_segmentation::UnicodeSegmentation;

fn node(body: MathBody) -> MathNode {
    MathNode {
        node: NodeId::fresh(),
        body,
    }
}

fn text(value: &str) -> MathNode {
    node(MathBody::Text { text: value.into() })
}

fn attach(base: MathNode, sub: Option<MathNode>, sup: Option<MathNode>) -> MathNode {
    node(MathBody::Attach {
        base: Box::new(base),
        sub: sub.map(Box::new),
        sup: sup.map(Box::new),
    })
}

struct Laid {
    geometry: EditGeometry,
    runs: Vec<String>,
}

fn lay_out(root: MathNode) -> Laid {
    let doc = StructuredDocument {
        document: DocumentId::fresh(),
        revision: Revision(1),
        blocks: vec![StructuredBlock {
            node: NodeId::fresh(),
            kind: BlockKind::Paragraph,
            content: vec![scholium_model::structured::StructuredInline {
                node: NodeId::fresh(),
                body: InlineBody::Math { root },
            }],
        }],
    };
    let mut cache = ContentCache::default();
    let content = cache.document(&doc).expect("content");
    let frame = kernel::layout(&kernel::EditWorld::new(), &content).expect("layout");
    let page = kernel::page(&frame);
    let geometry = EditGeometry::from_frame(&page.frame, &cache.reverse).expect("geometry");
    let mut runs = Vec::new();
    collect_runs(&page.frame, &mut runs);
    Laid { geometry, runs }
}

fn collect_runs(frame: &Frame, runs: &mut Vec<String>) {
    for (_, item) in frame.items() {
        match item {
            FrameItem::Text(run) => runs.push(run.text.to_string()),
            FrameItem::Group(group) => collect_runs(&group.frame, runs),
            _ => {}
        }
    }
}

fn at(leaf: NodeId, byte: usize, affinity: Affinity) -> Position {
    Position {
        leaf,
        byte,
        affinity,
    }
}

fn x_of(laid: &Laid, position: Position) -> f64 {
    let caret = laid
        .geometry
        .carets
        .iter()
        .find(|c| c.position == position)
        .unwrap_or_else(|| panic!("no caret at {position:?}"));
    caret.top[0]
}

#[test]
fn every_grapheme_boundary_of_math_text_has_a_caret() {
    let value = "ax+ 3.1\u{301}中 y-";
    let leaf = text(value);
    let id = leaf.node;
    let laid = lay_out(leaf);
    let boundaries = value
        .grapheme_indices(true)
        .map(|(at, _)| at)
        .chain([value.len()]);
    for byte in boundaries {
        assert!(
            laid.geometry
                .caret(at(id, byte, Affinity::Downstream))
                .is_some(),
            "no caret at byte {byte}"
        );
    }
}

#[test]
fn every_ascii_letter_is_an_italic_variable_with_carets() {
    let letters: String = ('a'..='z').chain('A'..='Z').collect();
    let leaf = text(&letters);
    let id = leaf.node;
    let laid = lay_out(leaf);
    for byte in 0..=letters.len() {
        assert!(
            laid.geometry
                .caret(at(id, byte, Affinity::Downstream))
                .is_some(),
            "no caret at byte {byte}"
        );
    }
    assert!(laid.runs.iter().all(|run| run.chars().count() == 1));
    assert!(
        laid.runs.iter().all(|run| !run.is_ascii()),
        "{:?}",
        laid.runs
    );
}

#[test]
fn letters_and_operators_are_separate_atoms_with_math_spacing() {
    let leaf = text("x+y");
    let id = leaf.node;
    let laid = lay_out(leaf);
    assert_eq!(laid.runs, ["\u{1D465}", "+", "\u{1D466}"]);
    // Binary operators get medium spaces on both sides; adjacent variables get none.
    let gap = |byte, other: &Laid| {
        x_of(other, at(id, byte, Affinity::Downstream))
            - x_of(other, at(id, byte, Affinity::Upstream))
    };
    assert!(gap(1, &laid) > 1.0, "no space before +");
    assert!(gap(2, &laid) > 1.0, "no space after +");
    let pair = text("xy");
    let pair_id = pair.node;
    let laid = lay_out(pair);
    assert!(
        gap_of(&laid, pair_id, 1).abs() < 0.01,
        "variables must touch"
    );
}

fn gap_of(laid: &Laid, leaf: NodeId, byte: usize) -> f64 {
    x_of(laid, at(leaf, byte, Affinity::Downstream))
        - x_of(laid, at(leaf, byte, Affinity::Upstream))
}

#[test]
fn attach_slots_have_carets_and_scripts_sit_above_and_below_the_base() {
    let (base, sub, sup) = (text("x"), text("i"), text("2"));
    let ids = (base.node, sub.node, sup.node);
    let laid = lay_out(attach(base, Some(sub), Some(sup)));
    let top = |leaf| {
        laid.geometry
            .caret(at(leaf, 0, Affinity::Downstream))
            .unwrap_or_else(|| panic!("no caret in {leaf:?}"))
            .top[1]
    };
    assert!(
        top(ids.2) < top(ids.0),
        "superscript must rise above the base"
    );
    assert!(
        top(ids.1) > top(ids.2),
        "subscript must sit below the superscript"
    );
    for leaf in [ids.0, ids.1, ids.2] {
        assert!(
            laid.geometry
                .caret(at(leaf, 1, Affinity::Upstream))
                .is_some(),
            "no end caret in {leaf:?}"
        );
    }
}

#[test]
fn empty_script_slots_and_hole_bases_are_addressable() {
    let (sub, sup) = (MathNode::hole(), MathNode::hole());
    let base = MathNode::hole();
    let ids = (base.node, sub.node, sup.node);
    let laid = lay_out(attach(base, Some(sub), Some(sup)));
    for leaf in [ids.0, ids.1, ids.2] {
        assert!(
            laid.geometry
                .caret(at(leaf, 0, Affinity::Downstream))
                .is_some(),
            "no hole caret in {leaf:?}"
        );
    }
}

#[test]
fn a_formula_row_around_an_attach_keeps_every_leaf_addressable() {
    let (left, base, sup, right) = (text("a"), text("b"), text("2"), text("+c"));
    let ids = [left.node, base.node, sup.node, right.node];
    let row = node(MathBody::Row {
        children: vec![left, attach(base, None, Some(sup)), right],
    });
    let laid = lay_out(row);
    for leaf in ids {
        assert!(
            laid.geometry
                .caret(at(leaf, 0, Affinity::Downstream))
                .is_some(),
            "no caret in {leaf:?}"
        );
    }
}

fn paragraph_doc(words: usize) -> StructuredDocument {
    let body = "lorem ipsum dolor ".repeat(words);
    StructuredDocument {
        document: DocumentId::fresh(),
        revision: Revision(1),
        blocks: vec![StructuredBlock {
            node: NodeId::fresh(),
            kind: BlockKind::Paragraph,
            content: vec![scholium_model::structured::StructuredInline {
                node: NodeId::fresh(),
                body: InlineBody::Text {
                    text: body,
                    style: scholium_model::structured::TextStyle::Plain,
                },
            }],
        }],
    }
}

fn stamp_of(doc: &StructuredDocument) -> scholium_model::layout_identity::SceneStamp {
    use scholium_model::layout_identity::*;
    SceneStamp {
        document: doc.document,
        epoch: LayoutEpoch::fresh(),
        request: LayoutRequestId::fresh(),
        revision: doc.revision,
        profile: ProfileGeneration(0),
        resources: ResourceGeneration(0),
    }
}

fn wait_for<T>(mut poll: impl FnMut() -> Option<T>) -> T {
    let at = std::time::Instant::now();
    loop {
        if let Some(value) = poll() {
            return value;
        }
        assert!(at.elapsed().as_secs() < 30, "worker timeout");
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

#[test]
fn scene_carries_a_cheap_base_and_only_the_visible_sharp_strips() {
    use super::{EditorWorker, RasterSpec, ViewRange, strip_count, strip_window};
    let spec = RasterSpec {
        px_per_pt: 3.0,
        max_side: 16384,
    };
    let view = ViewRange {
        top_pt: 0.0,
        bottom_pt: 300.0,
    };
    let worker = EditorWorker::spawn(|| {}).expect("worker");
    let doc = std::sync::Arc::new(paragraph_doc(1500));
    let stamp = stamp_of(&doc);
    worker.submit(doc, stamp, spec, view);
    let (_, scene) = wait_for(|| worker.poll());
    let scene = scene.expect("scene");
    assert!(scene.base.px_per_pt <= 1.0);
    let base_rows: u32 = scene.base.strips.iter().map(|s| s.height).sum();
    assert_eq!(base_rows, scene.base.height, "base tiles the whole page");
    assert_eq!(scene.sharp.px_per_pt, 3.0);
    let window = strip_window(view, 3.0, scene.sharp.height);
    let delivered: Vec<u32> = scene
        .sharp
        .strips
        .iter()
        .map(|s| s.y / super::SHARP_STRIP_PX)
        .collect();
    assert_eq!(delivered, window.visible.clone().collect::<Vec<_>>());
    assert!(delivered.len() < strip_count(scene.sharp.height) as usize);

    // The margin follows as separate updates for the same Scene, never repeating a strip.
    let mut margin = Vec::new();
    let want = window.keep.len() - window.visible.len();
    while margin.len() < want {
        margin.extend(wait_for(|| {
            Some(worker.poll_strips()).filter(|v| !v.is_empty())
        }));
    }
    assert!(
        margin
            .iter()
            .all(|u| u.stamp == stamp && u.px_per_pt == 3.0)
    );
    let mut indices: Vec<u32> = margin.iter().map(|u| u.strip.y / 512).collect();
    indices.sort_unstable();
    indices.dedup();
    assert_eq!(indices.len(), want);
    assert!(
        indices
            .iter()
            .all(|i| window.keep.contains(i) && !window.visible.contains(i))
    );
}

#[test]
fn view_and_scale_changes_render_strips_without_a_new_layout() {
    use super::{EditorWorker, RasterSpec, ViewRange, strip_window};
    let spec = |px_per_pt| RasterSpec {
        px_per_pt,
        max_side: 16384,
    };
    let top = ViewRange {
        top_pt: 0.0,
        bottom_pt: 100.0,
    };
    let worker = EditorWorker::spawn(|| {}).expect("worker");
    let doc = std::sync::Arc::new(paragraph_doc(1500));
    let stamp = stamp_of(&doc);
    worker.submit(doc, stamp, spec(3.0), top);
    let (_, scene) = wait_for(|| worker.poll());
    let scene = scene.expect("scene");
    let (height, page_pt) = (scene.sharp.height, scene.size_pt[1]);
    // Let the margin fill finish, then scroll far away.
    std::thread::sleep(std::time::Duration::from_millis(400));
    worker.poll_strips();
    let far = ViewRange {
        top_pt: page_pt * 0.8,
        bottom_pt: page_pt * 0.8 + 100.0,
    };
    worker.set_view(spec(3.0), far);
    let window = strip_window(far, 3.0, height);
    let first = wait_for(|| worker.poll_strips().into_iter().next());
    assert_eq!(first.stamp, stamp);
    assert!(
        window.visible.contains(&(first.strip.y / 512)),
        "visible strip first"
    );
    assert!(worker.poll().is_none(), "no layout ran");

    // A new scale discards delivered strips and renders the window at that scale.
    worker.set_view(spec(2.0), far);
    std::thread::sleep(std::time::Duration::from_millis(400));
    let updates = worker.poll_strips();
    let scaled: Vec<_> = updates.iter().filter(|u| u.px_per_pt == 2.0).collect();
    assert!(!scaled.is_empty());
    assert!(scaled.iter().all(|u| u.page_height < height));
}
