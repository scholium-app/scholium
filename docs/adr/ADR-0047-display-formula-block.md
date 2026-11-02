# ADR 0047: Display formulas are blocks

Status: Accepted

## Context

The structured Typst editor supports formulas as inline content. A display formula has independent paragraph layout and editing boundaries: Enter leaves it for a following paragraph, Backspace at that paragraph returns to the formula, and source uses Typst's `$ … $` block spelling. These are block operations, not an inline presentation option.

## Decision

Add `BlockKind::Equation`, whose validated content is exactly one `InlineBody::Math`. The existing math tree remains the sole equation body, so every math node, caret stop and structural edit keeps the inline contract. The editor constructs `EquationElem` with `block: true`. Source generation emits spaced dollar delimiters as a standalone block and maps the complete block range.

Insert at a body caret splits the body at the caret and places nonempty prefix and suffix paragraphs around the equation. A selection is accepted only when both endpoints form a valid selection within one math row; that selection becomes the equation root. Other selections are rejected atomically. Enter, Backspace and empty-block deletion follow explicit block boundaries. The shortcut is Ctrl+Shift+E; Ctrl+Shift+M already switches page/source mode.

The v1 JSON block tag is additive. Older builds reject the unknown enum tag just as they reject Attach; current storage round-trips it without changing the envelope version.

## Consequences

The block kind determines layout and source spelling, while math editing remains shared. Inline/display conversion is deferred because conversion needs a user-visible block-boundary policy for surrounding text and is not required for initial insertion.
