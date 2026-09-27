//! Unit tests for the structural caret projection (ADR 0031).
use super::caret::*;
use scholium_model::{Block, DocumentSnapshot, Inline, NodeId};

fn math(source: &str) -> Inline {
    Inline::Math(source.into())
}

fn text(value: &str) -> Inline {
    Inline::Text(value.into())
}

fn block(content: Vec<Inline>) -> Block {
    Block {
        node: NodeId::fresh(),
        kind: scholium_model::BlockKind::Paragraph,
        content,
    }
}

#[test]
fn math_caret_offsets_exclude_delimiters() {
    // `$alpha$`: offset 0 is the first source byte, 5 is the node end.
    let b = block(vec![math("alpha")]);
    assert_eq!(
        to_markup_byte(
            &b,
            Caret {
                block: b.node,
                inline: 0,
                offset: 0
            }
        ),
        1
    );
    assert_eq!(
        to_markup_byte(
            &b,
            Caret {
                block: b.node,
                inline: 0,
                offset: 5
            }
        ),
        6
    );
}

#[test]
fn markup_byte_inside_a_delimiter_never_lands_inside_it() {
    // Byte 0 is the opening `$`; it must snap to the node start, and byte 6
    // (the closing `$`) to the node end. A caret inside a delimiter is what
    // used to let Enter and Backspace split the pair apart.
    let b = block(vec![math("alpha")]);
    assert_eq!(from_markup_byte(&b, 0).offset, 0);
    assert_eq!(from_markup_byte(&b, 6).offset, 5);
    assert_eq!(from_markup_byte(&b, 3).offset, 2);
}

#[test]
fn offsets_round_trip_through_the_projection() {
    // Carets that address a real position *inside* a node survive the round
    // trip exactly. A caret at a node boundary shares its byte with the next
    // node's start, and `from_markup_byte` resolves that byte to the later
    // node; those boundaries are covered by
    // `a_node_boundary_resolves_to_the_following_node` instead.
    let b = block(vec![text("ab"), math("alpha/2"), text(" tail")]);
    for (inline, offset) in [(0, 0), (0, 1), (1, 0), (1, 3), (1, 6), (2, 1), (2, 3)] {
        let caret = Caret {
            block: b.node,
            inline,
            offset,
        };
        let byte = to_markup_byte(&b, caret);
        assert_eq!(from_markup_byte(&b, byte), caret, "{caret:?}");
    }
}

#[test]
fn a_node_boundary_resolves_to_a_real_position() {
    // `$alpha/2$ tail`: byte 8 is the closing `$` of the formula and byte 9
    // starts the following text. Neither is allowed to name a position
    // outside its node, and the closing delimiter belongs to the formula.
    let b = block(vec![math("alpha/2"), text(" tail")]);
    let end_of_math = to_markup_byte(
        &b,
        Caret {
            block: b.node,
            inline: 0,
            offset: 7,
        },
    );
    assert_eq!(end_of_math, 8);
    assert_eq!(
        from_markup_byte(&b, end_of_math),
        Caret {
            block: b.node,
            inline: 0,
            offset: 7
        }
    );
    let start_of_text = to_markup_byte(
        &b,
        Caret {
            block: b.node,
            inline: 1,
            offset: 0,
        },
    );
    assert_eq!(start_of_text, 9);
    assert_eq!(
        from_markup_byte(&b, start_of_text),
        Caret {
            block: b.node,
            inline: 1,
            offset: 0
        }
    );
}

#[test]
fn every_projected_byte_yields_a_caret_inside_the_block() {
    // `from_markup_byte` is a lossy hit-test inverse, so the promise is
    // validity rather than bijection: any byte, including one that splits an
    // escape pair, must resolve to a real position in this block.
    for nodes in [
        vec![text("a$b"), math("x")],
        vec![text("中$文")],
        vec![Inline::Strong("x*y".into())],
        vec![math("alpha/2"), text(" tail")],
        vec![text(""), text("$")],
        vec![text("")],
        vec![math("")],
        vec![text("a"), math(""), text("b")],
    ] {
        let b = block(nodes.clone());
        for byte in 0..=projected_len_block(&b) {
            let caret = from_markup_byte(&b, byte);
            assert_eq!(caret.block, b.node);
            assert!(caret.inline <= b.content.len(), "{nodes:?} byte {byte}");
            if let Some(node) = b.content.get(caret.inline) {
                assert!(
                    caret.offset <= content(node).len(),
                    "{nodes:?} byte {byte} -> {caret:?}"
                );
            } else {
                assert_eq!(caret.offset, 0, "{nodes:?} byte {byte}");
            }
        }
    }
}

#[test]
fn byte_offsets_grow_with_caret_order() {
    // The direction that must be exact: projected bytes never decrease as
    // the caret advances through the block, so a position can be ordered and
    // an edit range can be built from two carets without re-deriving bytes.
    for nodes in [
        vec![text("a$b"), math("x")],
        vec![text("中$文")],
        vec![math("alpha/2"), text(" tail")],
        vec![text("a"), math(""), text("b")],
    ] {
        let b = block(nodes.clone());
        let mut previous = 0;
        for (inline, node) in b.content.iter().enumerate() {
            for offset in 0..=content(node).len() {
                let byte = to_markup_byte(
                    &b,
                    Caret {
                        block: b.node,
                        inline,
                        offset,
                    },
                );
                assert!(byte >= previous, "{nodes:?} {inline}:{offset} -> {byte}");
                previous = byte;
            }
        }
    }
}

#[test]
fn zero_width_nodes_do_not_shadow_the_following_node() {
    // An empty inline node projects to nothing, so its single caret shares a
    // byte with the next node's start. A hit on that byte belongs to the
    // following node, which is the one that paints a glyph there.
    let b = block(vec![text(""), math("x")]);
    let hit = from_markup_byte(&b, 1);
    assert_eq!(hit.inline, 1);
    assert_eq!(hit.offset, 0);
}

#[test]
fn strong_delimiters_are_projection_only() {
    // `*abcd*` with the caret after "ab" is inline 0 offset 2, and it maps
    // past the opening marker.
    let b = block(vec![Inline::Strong("abcd".into())]);
    let caret = Caret {
        block: b.node,
        inline: 0,
        offset: 2,
    };
    assert_eq!(to_markup_byte(&b, caret), 3);
    assert_eq!(from_markup_byte(&b, 3), caret);
}

#[test]
fn escaped_literal_text_round_trips_through_the_projection() {
    // Projection escapes `$ * _ \` inside Text, so a stored byte and its
    // projected byte differ. Before this was handled, every offset after a
    // literal `$` drifted by one byte per escape.
    let b = block(vec![text("a$b"), math("x")]);
    assert_eq!(projected_len(&b.content[0]), 4);
    for (inline, offset) in [(0, 0), (0, 1), (0, 2), (0, 3), (1, 0), (1, 1)] {
        let caret = Caret {
            block: b.node,
            inline,
            offset,
        };
        let byte = to_markup_byte(&b, caret);
        assert_eq!(from_markup_byte(&b, byte), caret, "{caret:?}");
    }
    // The `$` sits at stored offset 1, which is projected byte 1; the
    // escape pair spans projected 1..3, and offset 2 is "b".
    assert_eq!(
        to_markup_byte(
            &b,
            Caret {
                block: b.node,
                inline: 0,
                offset: 2
            }
        ),
        3
    );
}

#[test]
fn markup_offsets_match_the_real_projection() {
    // Guard against the helper drifting from `scholium_model::markup`.
    let content = vec![text("a$b"), math("alpha"), Inline::Strong("x*y".into())];
    let b = block(content.clone());
    assert_eq!(
        projected_len_block(&b),
        scholium_model::markup(&content).len()
    );
}

#[test]
fn block_end_is_addressable_after_the_last_node() {
    let b = block(vec![math("x")]);
    let caret = Caret {
        block: b.node,
        inline: 1,
        offset: 0,
    };
    assert_eq!(to_markup_byte(&b, caret), 3);
    assert_eq!(from_markup_byte(&b, 3), caret);
}

#[test]
fn clamp_snaps_an_out_of_range_caret_into_the_same_block() {
    let b = block(vec![text("ab")]);
    let clamped = clamp(
        &b,
        Caret {
            block: b.node,
            inline: 9,
            offset: 0,
        },
    );
    assert_eq!(clamped.inline, 1);
    assert_eq!(clamped.block, b.node);
}

#[test]
fn clamp_snaps_an_interior_multibyte_offset_down_to_a_boundary() {
    // "中" is 3 bytes; offset 1 is interior and must not slice mid-char.
    let b = block(vec![text("中")]);
    assert_eq!(
        clamp(
            &b,
            Caret {
                block: b.node,
                inline: 0,
                offset: 1
            }
        )
        .offset,
        0
    );
}

#[test]
fn global_and_local_offsets_agree_across_blocks() {
    let first = block(vec![math("alpha")]);
    let second = block(vec![text("tail")]);
    let snapshot = DocumentSnapshot {
        document: scholium_model::DocumentId::fresh(),
        revision: scholium_model::Revision::default(),
        blocks: vec![first.clone(), second.clone()],
    };
    // `$alpha$` is 7 bytes, then the joining `\n`, so "tail" starts at 8.
    assert_eq!(
        global_byte(
            &snapshot,
            position_of(
                &second,
                Caret {
                    block: second.node,
                    inline: 0,
                    offset: 0
                }
            )
        ),
        Some(8)
    );
    assert_eq!(
        from_global_byte(&snapshot, 8),
        Some(Caret {
            block: second.node,
            inline: 0,
            offset: 0
        })
    );
}
