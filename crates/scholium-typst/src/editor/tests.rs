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
