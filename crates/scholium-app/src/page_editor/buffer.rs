//! One-frame editing projection. Its minimal diff is the only outgoing write.
use scholium_model::{BlockEdit, BlockPosition, DocumentRequest, DocumentSnapshot, NodeId};

/// One edit between the compiled revision's text and the live buffer: the byte
/// range `[start, old_end)` of the older text became `[start, new_end)` of the
/// newer one. `resulting` is the document revision the edit produced; the
/// placeholder `PENDING_REVISION` is filled in when the session accepts it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Shift {
    /// Byte offset where the change begins, in the text before this edit.
    pub(crate) start: usize,
    /// End of the replaced range in the text before this edit.
    pub(crate) old_end: usize,
    /// End of the replacement in the text after this edit.
    pub(crate) new_end: usize,
    /// Revision produced by the accepted edit; `PENDING_REVISION` until known.
    pub(crate) resulting: u64,
}

/// Marks a shift whose session acceptance is still undecided this frame.
pub(crate) const PENDING_REVISION: u64 = u64::MAX;

/// Which end of a byte range a position anchors: range starts stay before an
/// insertion at that point, range ends move past it. This keeps a line's left
/// edge from drifting right when text is inserted at the line start, while a
/// click right of the last glyph still lands after freshly typed text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Side {
    Start,
    End,
}

/// Map a byte position of the compiled text onto the live buffer by replaying
/// every edit since the compiled revision. Positions inside a replaced range
/// collapse to the replacement start.
pub(crate) fn map_forward(shifts: &[Shift], byte: usize, side: Side) -> usize {
    let mut at = byte;
    for shift in shifts {
        if at < shift.start {
            // Before the edit: unchanged.
        } else if at == shift.start && shift.old_end == shift.start {
            // Pure insertion at this position: the start side keeps pointing
            // before the inserted text, the end side jumps past it.
            if side == Side::End {
                at = shift.new_end;
            }
        } else if at < shift.old_end {
            at = shift.start;
        } else {
            at = at - shift.old_end + shift.new_end;
        }
    }
    at
}

/// Drop shifts that are baked into geometry compiled at `revision`: an edit
/// with resulting revision <= `revision` is part of that compile.
pub(crate) fn retain_after(shifts: &mut Vec<Shift>, revision: u64) {
    shifts.retain(|shift| shift.resulting > revision);
}

/// Byte diff of one frame's editing, as a pending shift on the shift chain.
pub(crate) fn shift_of(before: &str, after: &str) -> Option<Shift> {
    let prefix = before
        .chars()
        .zip(after.chars())
        .take_while(|(a, b)| a == b)
        .map(|(ch, _)| ch.len_utf8())
        .sum::<usize>();
    let suffix = before[prefix..]
        .chars()
        .rev()
        .zip(after[prefix..].chars().rev())
        .take_while(|(a, b)| a == b)
        .map(|(ch, _)| ch.len_utf8())
        .sum::<usize>();
    (before != after).then(|| Shift {
        start: prefix,
        old_end: before.len() - suffix,
        new_end: after.len() - suffix,
        resulting: PENDING_REVISION,
    })
}

pub(super) fn text(snapshot: &DocumentSnapshot) -> String {
    snapshot
        .blocks
        .iter()
        .map(|block| block.markup_text())
        .collect::<Vec<_>>()
        .join("\n")
}

pub(super) fn global(snapshot: &DocumentSnapshot, node: NodeId, byte: usize) -> Option<usize> {
    let mut offset = 0;
    for block in &snapshot.blocks {
        let text = block.markup_text();
        if block.node == node {
            return text.is_char_boundary(byte).then_some(offset + byte);
        }
        offset += text.len() + 1;
    }
    None
}

pub(super) fn position(snapshot: &DocumentSnapshot, offset: usize) -> Option<BlockPosition> {
    let mut base = 0;
    for block in &snapshot.blocks {
        let text = block.markup_text();
        if offset <= base + text.len() {
            return text
                .is_char_boundary(offset - base)
                .then_some(BlockPosition {
                    block: block.node,
                    byte: offset - base,
                });
        }
        base += text.len() + 1;
    }
    None
}

/// The outgoing write plus the shift that keeps stale geometry usable until
/// the recompile lands (dead-zone elimination).
pub(super) fn request_and_shift(
    snapshot: &DocumentSnapshot,
    before: &str,
    after: &str,
) -> Option<(DocumentRequest, Shift)> {
    let shift = shift_of(before, after)?;
    let text = &after[shift.start..shift.new_end];
    Some((
        snapshot.request(BlockEdit::ReplaceRange {
            start: position(snapshot, shift.start)?,
            end: position(snapshot, shift.old_end)?,
            text: text.into(),
        }),
        shift,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn insertion(start: usize, len: usize) -> Shift {
        Shift {
            start,
            old_end: start,
            new_end: start + len,
            resulting: PENDING_REVISION,
        }
    }

    fn replacement(start: usize, old_end: usize, new_end: usize) -> Shift {
        Shift {
            start,
            old_end,
            new_end,
            resulting: PENDING_REVISION,
        }
    }

    #[test]
    fn insertion_at_a_line_start_keeps_the_line_start_left_of_it() {
        // "first line" with "!" typed at byte 0: the compiled line's left edge
        // still addresses the line start, not the inserted character.
        let shifts = [insertion(0, 1)];
        assert_eq!(map_forward(&shifts, 0, Side::Start), 0);
        assert_eq!(map_forward(&shifts, 10, Side::End), 11);
    }

    #[test]
    fn insertion_at_a_run_end_carries_the_run_end_past_it() {
        // "ab" with "XY" appended: a click right of the glyphs lands after
        // the freshly typed text, matching the caret-at-end convention.
        let shifts = [insertion(2, 2)];
        assert_eq!(map_forward(&shifts, 0, Side::Start), 0);
        assert_eq!(map_forward(&shifts, 2, Side::End), 4);
    }

    #[test]
    fn deletion_collapses_positions_inside_the_replaced_range() {
        let shifts = [replacement(2, 4, 2)];
        assert_eq!(map_forward(&shifts, 1, Side::Start), 1);
        assert_eq!(map_forward(&shifts, 3, Side::Start), 2);
        assert_eq!(map_forward(&shifts, 4, Side::End), 2);
        assert_eq!(map_forward(&shifts, 6, Side::End), 4);
    }

    #[test]
    fn successive_edits_replay_in_order() {
        // Type "b" at 1, then "d" at 3: 0..3 of the compiled text is 0..5 now.
        let shifts = [insertion(1, 1), insertion(3, 1)];
        assert_eq!(map_forward(&shifts, 0, Side::Start), 0);
        assert_eq!(map_forward(&shifts, 3, Side::End), 5);
    }

    #[test]
    fn retain_after_drops_shifts_baked_into_a_compile() {
        let mut shifts = vec![
            replacement(0, 1, 1).with_revision(2),
            replacement(4, 5, 6).with_revision(5),
        ];
        retain_after(&mut shifts, 2);
        assert_eq!(shifts, vec![replacement(4, 5, 6).with_revision(5)]);
        retain_after(&mut shifts, 9);
        assert!(shifts.is_empty());
    }

    impl Shift {
        fn with_revision(mut self, revision: u64) -> Self {
            self.resulting = revision;
            self
        }
    }
}
