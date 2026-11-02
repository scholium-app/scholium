# Display formula implementation

Implemented the additive Equation block kind for Typst display formulas. Equation blocks validate as one Math inline, serialize through the existing version-1 structured envelope, lay out as block equations, and generate standalone spaced-dollar source with block ranges. Insertion splits body text or extracts a same-row math selection; Enter, Backspace, undo and empty-equation deletion use block semantics. Inline/display conversion remains deferred.

Tests were added before implementation for the Equation model/validation and codec, insertion and
selection behavior, block-boundary editing, source ranges, Content geometry, and display-aware session
parity. The initial model test failed because Equation did not exist; the initial placeholder/Enter
regression failed because the temporary Attach slot was still unfilled; and the selection-transfer test
initially failed because display roots rejected selected math. Each passed after its corresponding fix.

`cargo fmt --all -- --check`, workspace clippy with `-D warnings`, rustdoc with `-D warnings`,
workspace binary build, Rust-size, doc-link, UI-string, layer, and `cargo deny` checks passed. Both
`cargo test --locked --workspace` and `SCHOLIUM_TEST_FONTS=bundled cargo test --locked --workspace`
reported 517 passed, 0 failed, and 4 ignored. Cargo deny reported the existing unused
`Unicode-DFS-2016` allowance warning; bans, licenses, and sources passed.

The independent stock Typst reference matched all 19 fork fixtures pixel-for-pixel under system fonts
and all 19 under bundled fonts, including display equations alone, at document start/end, and between
paragraphs. No fork/vendor patch was needed and comparison thresholds were unchanged. The full private
Xvfb window suite passed 42/42 checks across `zh-Hans` and `en`, including display insertion, shortcut,
menu, Enter/Backspace, and shared parity replay. Screenshot and run evidence is in the ignored
`target/t33-window/` output.

The shortcut is Ctrl+Shift+E because Ctrl+Shift+M already switches page/source mode. Inline/display
focus-toolbar conversion remains deferred, as permitted by the task spec; the block-boundary policy for
surrounding text needs a product decision before conversion can be made unambiguous.
