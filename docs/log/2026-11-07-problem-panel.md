# Problem panel

2026-11-07 delivered the shared diagnostic panel for native Typst, Typst source preview and LaTeX workspaces. The panel consumes revision-bound `scholium_model::diagnostic::DiagnosticBatch` values, groups rows by resource, exposes bounded build output, and routes Enter/F8 jumps through source selections or Typst NodeId targets. LaTeX source-member selection retains per-file drafts under the existing project lock and keeps the declared build entry unchanged.

Validation passed `cargo fmt`, workspace clippy, workspace tests, rustdoc, size, documentation links, UI strings, layer checks, cargo-deny, app/storage builds, and both stock Typst comparisons. Private-Xvfb window checks passed diagnostics and problem-panel checks in zh-Hans and en; captured light/dark evidence is in `evidence/problem-panel/`. Native XeLaTeX and LuaLaTeX checks were run with the pinned local toolchain; the final parallel attempt had an environment contention timeout during theme-menu navigation, so the earlier serial native evidence is retained separately from the CI gate.

The storage resource-selection API and the small LaTeX supervisor load-before-edit change are scoped exceptions required for multi-file diagnostic jumps. No PR was opened by this executor.
